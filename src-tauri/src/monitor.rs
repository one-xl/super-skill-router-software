//! Desktop reconnect monitoring.
//!
//! The reconnect counter and run/idle state come from ChatGPT Desktop's own UI
//! Automation buttons. Logs are tailed only to follow turn and file rotation;
//! delayed log lines never trigger recovery. The monitor never starts a CLI.

use std::collections::HashMap;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::{mpsc, watch, Mutex};

use crate::automation::CodexDesktopSnapshot;
use crate::{automation, settings};

const MAX_FAILURES: u32 = 5;

#[derive(Clone, Debug, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MonitorState {
    Watching,
    Reconnecting,
    Stopped,
    Error,
}

#[derive(Clone, Debug, Serialize)]
pub struct DesktopMonitorStatus {
    pub target_id: String,
    pub target_label: String,
    pub state: MonitorState,
    pub reconnect_attempt: u32,
    pub recovery_sent_count: u32,
    pub recovery_text: String,
    pub last_error: Option<String>,
    pub log_path: Option<String>,
    pub running_seen: bool,
    pub failure_seen: bool,
    pub send_button_visible: bool,
}

struct MonitorEntry {
    id: u64,
    status: DesktopMonitorStatus,
    stop_tx: mpsc::UnboundedSender<()>,
}

struct MonitorTaskContext {
    app: AppHandle,
    target_id: String,
    log_root: PathBuf,
    initial_log_path: PathBuf,
    recovery_text: String,
    stop_rx: mpsc::UnboundedReceiver<()>,
    shutdown_rx: watch::Receiver<bool>,
    monitors: Arc<Mutex<HashMap<String, MonitorEntry>>>,
    monitor_id: u64,
}

pub struct DesktopMonitorSupervisor {
    monitors: Arc<Mutex<HashMap<String, MonitorEntry>>>,
    next_id: AtomicU64,
    shutdown_tx: watch::Sender<bool>,
}

impl Default for DesktopMonitorSupervisor {
    fn default() -> Self {
        let (shutdown_tx, _) = watch::channel(false);
        Self {
            monitors: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
            shutdown_tx,
        }
    }
}

#[derive(Default, Debug)]
struct ReconnectTracker {
    attempt: u32,
    awaiting_recovery_turn: bool,
    recovery_turn_deadline: Option<Instant>,
    running_seen: bool,
    failure_banner_baseline: HashSet<String>,
    terminal_failure_observed: bool,
    terminal_failure_handled: bool,
}

#[derive(Debug, PartialEq)]
enum ReconnectEvent {
    Attempt(u32),
    Connected,
    NewTurn,
    Ignore,
}

impl ReconnectTracker {
    #[cfg(test)]
    fn observe(&mut self, line: &str) -> (ReconnectEvent, bool) {
        self.observe_event(parse_codex_connection_event(line))
    }

    fn observe_event(&mut self, event: ReconnectEvent) -> (ReconnectEvent, bool) {
        self.observe_event_at(event, Instant::now())
    }

    fn observe_event_at(&mut self, event: ReconnectEvent, now: Instant) -> (ReconnectEvent, bool) {
        match event {
            ReconnectEvent::Attempt(attempt) => {
                if self.awaiting_recovery_turn && attempt < MAX_FAILURES {
                    self.rearm_for_next_turn();
                }
                self.attempt = attempt;
                (ReconnectEvent::Attempt(attempt), false)
            }
            ReconnectEvent::Connected => {
                if !self.awaiting_recovery_turn {
                    self.rearm_for_next_turn();
                }
                (ReconnectEvent::Connected, false)
            }
            ReconnectEvent::NewTurn => {
                if self
                    .recovery_turn_deadline
                    .is_some_and(|deadline| now <= deadline)
                {
                    self.recovery_turn_deadline = None;
                    (ReconnectEvent::Ignore, false)
                } else {
                    self.recovery_turn_deadline = None;
                    self.rearm_for_next_turn();
                    (ReconnectEvent::NewTurn, false)
                }
            }
            ReconnectEvent::Ignore => (ReconnectEvent::Ignore, false),
        }
    }

    fn mark_recovery_sent(&mut self) {
        self.awaiting_recovery_turn = true;
        self.recovery_turn_deadline = Some(Instant::now() + Duration::from_secs(5));
    }

    fn observe_snapshot(&mut self, snapshot: CodexDesktopSnapshot) -> (ReconnectEvent, bool) {
        let mut event = snapshot
            .reconnect_attempt
            .map(|attempt| self.observe_event(ReconnectEvent::Attempt(attempt)).0)
            .unwrap_or(ReconnectEvent::Ignore);

        if snapshot.running {
            if self.awaiting_recovery_turn {
                self.rearm_for_next_turn();
            }
            if !self.running_seen {
                self.failure_banner_baseline = snapshot.failure_banners.iter().cloned().collect();
            }
            self.running_seen = true;
            if event == ReconnectEvent::Ignore {
                event = ReconnectEvent::NewTurn;
            }
        }

        if self.running_seen
            && snapshot
                .failure_banners
                .iter()
                .any(|id| !self.failure_banner_baseline.contains(id))
        {
            self.terminal_failure_observed = true;
        }

        let should_recover = snapshot.idle
            && self.running_seen
            && self.terminal_failure_observed
            && !self.terminal_failure_handled
            && !self.awaiting_recovery_turn;
        if should_recover {
            self.terminal_failure_handled = true;
        }
        if snapshot.idle && event == ReconnectEvent::Ignore {
            event = ReconnectEvent::Connected;
        }
        (event, should_recover)
    }

    fn rearm_for_next_turn(&mut self) {
        self.attempt = 0;
        self.awaiting_recovery_turn = false;
        self.recovery_turn_deadline = None;
        self.running_seen = false;
        self.failure_banner_baseline.clear();
        self.terminal_failure_observed = false;
        self.terminal_failure_handled = false;
    }
}

fn field_u32(line: &str, key: &str) -> Option<u32> {
    let marker = format!("{key}=");
    let start = line.find(&marker)? + marker.len();
    let value = line[start..]
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_matches(|character: char| !character.is_ascii_digit());
    value.parse().ok()
}

fn parse_codex_connection_event(line: &str) -> ReconnectEvent {
    if !line.contains("[AppServerConnection]") {
        return ReconnectEvent::Ignore;
    }
    if line.contains("response_routed") && line.contains("method=turn/start") {
        return ReconnectEvent::NewTurn;
    }
    if !line.contains("state_changed") {
        return ReconnectEvent::Ignore;
    }
    if line.contains("next=connected") {
        return ReconnectEvent::Connected;
    }
    match field_u32(line, "reconnectAttempt") {
        Some(attempt) if attempt > 0 => ReconnectEvent::Attempt(attempt),
        _ => ReconnectEvent::Ignore,
    }
}

fn codex_log_root() -> Result<PathBuf, String> {
    let local = std::env::var_os("LOCALAPPDATA")
        .ok_or_else(|| "无法读取 LOCALAPPDATA，不能定位 Codex Desktop 日志。".to_string())?;
    Ok(PathBuf::from(local).join("Codex").join("Logs"))
}

fn newest_log(root: &Path) -> Result<PathBuf, String> {
    fn visit(directory: &Path, newest: &mut Option<(std::time::SystemTime, PathBuf)>) {
        let Ok(entries) = std::fs::read_dir(directory) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                visit(&path, newest);
            } else if path.extension().and_then(|value| value.to_str()) == Some("log") {
                if let Ok(modified) = entry.metadata().and_then(|metadata| metadata.modified()) {
                    if newest.as_ref().is_none_or(|(time, _)| modified > *time) {
                        *newest = Some((modified, path));
                    }
                }
            }
        }
    }

    let mut newest = None;
    visit(root, &mut newest);
    newest
        .map(|(_, path)| path)
        .ok_or_else(|| format!("没有在 {} 中找到 Codex Desktop 日志。", root.display()))
}

impl DesktopMonitorSupervisor {
    pub async fn start(&self, app: AppHandle, target_id: String) -> Result<(), String> {
        if target_id != "codex_desktop" {
            return Err("当前仅 Codex Desktop 暴露了可核验的重连次数日志；Claude Code Desktop 暂不启用自动恢复监控。".to_string());
        }

        let mut monitors = self.monitors.lock().await;
        if monitors.contains_key(&target_id) {
            return Err("Codex Desktop 自动恢复监控已经在运行。".to_string());
        }
        let log_root = codex_log_root()?;
        let log_path = newest_log(&log_root)?;
        let recovery_text = settings::load(&app)?.automation.recovery_text;
        let monitor_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (stop_tx, stop_rx) = mpsc::unbounded_channel();
        let status = DesktopMonitorStatus {
            target_id: target_id.clone(),
            target_label: "ChatGPT Desktop (Codex)".to_string(),
            state: MonitorState::Watching,
            reconnect_attempt: 0,
            recovery_sent_count: 0,
            recovery_text: recovery_text.clone(),
            last_error: None,
            log_path: Some(log_path.display().to_string()),
            running_seen: false,
            failure_seen: false,
            send_button_visible: false,
        };
        monitors.insert(
            target_id.clone(),
            MonitorEntry {
                id: monitor_id,
                status: status.clone(),
                stop_tx,
            },
        );
        drop(monitors);
        let _ = app.emit("desktop-monitor-status", &status);

        let shared = Arc::clone(&self.monitors);
        tokio::spawn(tail_codex_log(MonitorTaskContext {
            app,
            target_id,
            log_root,
            initial_log_path: log_path,
            recovery_text,
            stop_rx,
            shutdown_rx: self.shutdown_tx.subscribe(),
            monitors: shared,
            monitor_id,
        }));
        Ok(())
    }

    pub async fn stop(&self, app: &AppHandle, target_id: &str) -> Result<(), String> {
        let entry = self
            .monitors
            .lock()
            .await
            .remove(target_id)
            .ok_or_else(|| "该桌面 Agent 没有运行中的监控。".to_string())?;
        let mut status = entry.status;
        status.state = MonitorState::Stopped;
        status.reconnect_attempt = 0;
        status.running_seen = false;
        status.failure_seen = false;
        status.send_button_visible = false;
        let _ = entry.stop_tx.send(());
        let _ = app.emit("desktop-monitor-status", &status);
        Ok(())
    }

    pub fn request_shutdown(&self) {
        let _ = self.shutdown_tx.send(true);
    }

    pub async fn list(&self) -> Vec<DesktopMonitorStatus> {
        self.monitors
            .lock()
            .await
            .values()
            .map(|entry| entry.status.clone())
            .collect()
    }
}

async fn update_status(
    app: &AppHandle,
    monitors: &Arc<Mutex<HashMap<String, MonitorEntry>>>,
    target_id: &str,
    update: impl FnOnce(&mut DesktopMonitorStatus),
) {
    let mut lock = monitors.lock().await;
    if let Some(entry) = lock.get_mut(target_id) {
        update(&mut entry.status);
        let _ = app.emit("desktop-monitor-status", &entry.status);
    }
}

async fn tail_codex_log(context: MonitorTaskContext) {
    let MonitorTaskContext {
        app,
        target_id,
        log_root,
        initial_log_path,
        recovery_text,
        mut stop_rx,
        mut shutdown_rx,
        monitors,
        monitor_id,
    } = context;
    let result = async {
        let file = tokio::fs::File::open(&initial_log_path)
            .await
            .map_err(|error| format!("无法打开 Codex Desktop 日志：{error}"))?;
        let mut reader = BufReader::new(file);
        reader
            .seek(std::io::SeekFrom::End(0))
            .await
            .map_err(|error| format!("无法定位 Codex Desktop 日志末尾：{error}"))?;
        let mut log_path = initial_log_path;
        let mut tracker = ReconnectTracker::default();
        let mut line = String::new();
        let mut rotation_ticks = 0_u8;

        loop {
            tokio::select! {
                _ = stop_rx.recv() => break,
                changed = shutdown_rx.changed() => {
                    if changed.is_err() || *shutdown_rx.borrow() {
                        break;
                    }
                }
                _ = tokio::time::sleep(std::time::Duration::from_millis(400)) => {
                    rotation_ticks = rotation_ticks.saturating_add(1);
                    if rotation_ticks >= 5 {
                        rotation_ticks = 0;
                        let latest = newest_log(&log_root)?;
                        if latest != log_path {
                            let file = tokio::fs::File::open(&latest)
                                .await
                                .map_err(|error| format!("无法切换到新的 Codex Desktop 日志：{error}"))?;
                            reader = BufReader::new(file);
                            log_path = latest;
                            tracker = ReconnectTracker::default();
                            update_status(&app, &monitors, &target_id, |status| {
                                status.state = MonitorState::Watching;
                                status.reconnect_attempt = 0;
                                status.last_error = None;
                                status.log_path = Some(log_path.display().to_string());
                            }).await;
                        }
                    }
                    let desktop_activity = tauri::async_runtime::spawn_blocking(
                        automation::codex_desktop_snapshot,
                    )
                    .await
                    .map_err(|error| format!("读取 ChatGPT Desktop 重连状态失败：{error}"))?;
                    match desktop_activity {
                        Ok(activity) => {
                            let send_button_visible = activity.idle;
                            let (event, should_recover) = tracker.observe_snapshot(activity);
                            update_status(&app, &monitors, &target_id, |status| {
                                status.running_seen = tracker.running_seen;
                                status.failure_seen = tracker.terminal_failure_observed;
                                status.send_button_visible = send_button_visible;
                            }).await;
                            apply_reconnect_event(&app, &monitors, &target_id, event).await;
                            if should_recover
                                && send_recovery(&app, &monitors, &target_id, &recovery_text)
                                    .await?
                            {
                                tracker.mark_recovery_sent();
                            }
                        }
                        Err(_) => {
                            // ChatGPT Desktop may be minimized or closing. The log monitor remains active.
                        }
                    }
                    loop {
                        line.clear();
                        let read = reader.read_line(&mut line).await.map_err(|error| format!("读取 Codex Desktop 日志失败：{error}"))?;
                        if read == 0 { break; }
                        let event = parse_codex_connection_event(&line);
                        if event == ReconnectEvent::NewTurn {
                            let (event, _) = tracker.observe_event(ReconnectEvent::NewTurn);
                            apply_reconnect_event(&app, &monitors, &target_id, event).await;
                        }
                    }
                }
            }
        }
        Ok::<(), String>(())
    }
    .await;

    finish_monitor(&app, &monitors, &target_id, monitor_id, result).await;
}

async fn finish_monitor(
    app: &AppHandle,
    monitors: &Arc<Mutex<HashMap<String, MonitorEntry>>>,
    target_id: &str,
    monitor_id: u64,
    result: Result<(), String>,
) {
    let status = {
        let mut lock = monitors.lock().await;
        let is_current = lock
            .get(target_id)
            .is_some_and(|entry| entry.id == monitor_id);
        if !is_current {
            return;
        }
        let mut entry = match lock.remove(target_id) {
            Some(entry) => entry,
            None => return,
        };
        match result {
            Ok(()) => entry.status.state = MonitorState::Stopped,
            Err(error) => {
                entry.status.state = MonitorState::Error;
                entry.status.last_error = Some(error);
            }
        }
        entry.status
    };
    let _ = app.emit("desktop-monitor-status", &status);
}

async fn apply_reconnect_event(
    app: &AppHandle,
    monitors: &Arc<Mutex<HashMap<String, MonitorEntry>>>,
    target_id: &str,
    event: ReconnectEvent,
) {
    match event {
        ReconnectEvent::Attempt(attempt) => {
            update_status(app, monitors, target_id, |status| {
                status.state = MonitorState::Reconnecting;
                status.reconnect_attempt = attempt;
                status.last_error = None;
            })
            .await;
        }
        ReconnectEvent::Connected => {
            update_status(app, monitors, target_id, |status| {
                status.state = MonitorState::Watching;
                status.reconnect_attempt = 0;
                status.last_error = None;
            })
            .await;
        }
        ReconnectEvent::NewTurn => {
            update_status(app, monitors, target_id, |status| {
                status.state = MonitorState::Watching;
                status.reconnect_attempt = 0;
                status.last_error = None;
            })
            .await;
        }
        ReconnectEvent::Ignore => {}
    }
}

async fn send_recovery(
    app: &AppHandle,
    monitors: &Arc<Mutex<HashMap<String, MonitorEntry>>>,
    target_id: &str,
    recovery_text: &str,
) -> Result<bool, String> {
    let target = target_id.to_string();
    let recovery_text = recovery_text.to_string();
    let recovery = tauri::async_runtime::spawn_blocking(move || {
        automation::send_text_to_desktop(&target, &recovery_text, true)
    })
    .await
    .map_err(|error| format!("自动恢复任务异常结束：{error}"))?;
    let sent = recovery.is_ok();
    update_status(app, monitors, target_id, |status| match recovery {
        Ok(()) => {
            status.recovery_sent_count += 1;
            status.state = MonitorState::Watching;
            status.reconnect_attempt = 0;
            status.last_error = None;
        }
        Err(error) => {
            status.state = MonitorState::Error;
            status.last_error = Some(error);
        }
    })
    .await;
    Ok(sent)
}

#[tauri::command]
pub async fn start_desktop_monitor(
    app: AppHandle,
    supervisor: tauri::State<'_, DesktopMonitorSupervisor>,
    target_id: String,
) -> Result<(), String> {
    supervisor.start(app, target_id).await
}

#[tauri::command]
pub async fn stop_desktop_monitor(
    app: AppHandle,
    supervisor: tauri::State<'_, DesktopMonitorSupervisor>,
    target_id: String,
) -> Result<(), String> {
    supervisor.stop(&app, &target_id).await
}

#[tauri::command]
pub async fn list_desktop_monitors(
    supervisor: tauri::State<'_, DesktopMonitorSupervisor>,
) -> Result<Vec<DesktopMonitorStatus>, String> {
    Ok(supervisor.list().await)
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::automation::CodexDesktopSnapshot;

    use super::{parse_codex_connection_event, ReconnectEvent, ReconnectTracker};

    fn line(attempt: u32) -> String {
        format!("info [AppServerConnection] app_server_connection.state_changed next=connecting reconnectAttempt={attempt}")
    }

    fn running(error: Option<&str>) -> CodexDesktopSnapshot {
        CodexDesktopSnapshot {
            running: true,
            failure_banners: error.into_iter().map(str::to_string).collect(),
            ..Default::default()
        }
    }

    fn idle(error: Option<&str>) -> CodexDesktopSnapshot {
        CodexDesktopSnapshot {
            idle: true,
            failure_banners: error.into_iter().map(str::to_string).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn ignores_unrelated_retry_text() {
        assert_eq!(
            parse_codex_connection_event("tool retrying after network error"),
            ReconnectEvent::Ignore
        );
    }

    #[test]
    fn reconnect_attempt_without_error_never_recovers() {
        let mut tracker = ReconnectTracker::default();
        for attempt in 1..5 {
            assert!(!tracker.observe(&line(attempt)).1);
        }
        assert!(!tracker.observe(&line(5)).1);
        assert!(!tracker.observe_snapshot(idle(None)).1);
    }

    #[test]
    fn attempt_five_and_error_without_a_seen_stop_button_does_not_recover() {
        let mut tracker = ReconnectTracker::default();
        assert!(
            !tracker
                .observe_snapshot(CodexDesktopSnapshot {
                    reconnect_attempt: Some(5),
                    idle: true,
                    failure_banners: vec!["unexpected status 503 service unavailable".to_string()],
                    ..Default::default()
                })
                .1
        );
    }

    #[test]
    fn arbitrary_error_after_running_recovers_on_idle() {
        let mut tracker = ReconnectTracker::default();
        assert!(!tracker.observe_snapshot(running(None)).1);
        assert!(
            tracker
                .observe_snapshot(idle(Some("unexpected status 503 service unavailable")))
                .1
        );
    }

    #[test]
    fn turn_start_log_cannot_replace_the_stop_button_transition() {
        let mut tracker = ReconnectTracker::default();
        tracker.observe_event(ReconnectEvent::NewTurn);
        assert!(
            !tracker
                .observe_snapshot(idle(Some("connection reset by peer error")))
                .1
        );
    }

    #[test]
    fn error_visible_while_running_waits_until_idle() {
        let mut tracker = ReconnectTracker::default();
        assert!(!tracker.observe_snapshot(running(None)).1);
        assert!(
            !tracker
                .observe_snapshot(running(Some("request failed with status 500")))
                .1
        );
        assert!(
            tracker
                .observe_snapshot(idle(Some("request failed with status 500")))
                .1
        );
    }

    #[test]
    fn normal_completion_does_not_recover() {
        let mut tracker = ReconnectTracker::default();
        assert!(!tracker.observe_snapshot(running(None)).1);
        assert!(!tracker.observe_snapshot(idle(None)).1);
    }

    #[test]
    fn historical_error_visible_only_after_monitor_start_does_not_recover() {
        let mut tracker = ReconnectTracker::default();
        assert!(!tracker.observe_snapshot(idle(Some("old request failed"))).1);
    }

    #[test]
    fn existing_failure_banner_is_the_running_turn_baseline() {
        let mut tracker = ReconnectTracker::default();
        assert!(!tracker.observe_snapshot(running(Some("banner-1"))).1);
        assert!(!tracker.observe_snapshot(idle(Some("banner-1"))).1);
    }

    #[test]
    fn a_new_banner_id_triggers_even_when_the_error_text_would_match() {
        let mut tracker = ReconnectTracker::default();
        assert!(!tracker.observe_snapshot(running(Some("banner-1"))).1);
        assert!(
            !tracker
                .observe_snapshot(CodexDesktopSnapshot {
                    running: true,
                    failure_banners: vec!["banner-1".to_string(), "banner-2".to_string()],
                    ..Default::default()
                })
                .1
        );
        assert!(
            tracker
                .observe_snapshot(CodexDesktopSnapshot {
                    idle: true,
                    failure_banners: vec!["banner-1".to_string(), "banner-2".to_string()],
                    ..Default::default()
                })
                .1
        );
    }

    #[test]
    fn recovery_rearms_for_the_next_failed_turn() {
        let mut tracker = ReconnectTracker::default();
        tracker.observe_snapshot(running(None));
        assert!(tracker.observe_snapshot(idle(Some("request failed"))).1);
        tracker.mark_recovery_sent();

        assert!(!tracker.observe_snapshot(running(None)).1);
        assert!(tracker.observe_snapshot(idle(Some("request failed"))).1);
    }

    #[test]
    fn delayed_user_turn_log_rearms_after_recovery_window() {
        let mut tracker = ReconnectTracker::default();
        let start = Instant::now();
        tracker.observe_snapshot(running(None));
        assert!(tracker.observe_snapshot(idle(Some("request failed"))).1);
        tracker.mark_recovery_sent();
        assert_eq!(
            tracker
                .observe_event_at(ReconnectEvent::NewTurn, start + Duration::from_secs(6))
                .0,
            ReconnectEvent::NewTurn
        );
    }

    #[test]
    fn immediate_recovery_turn_is_not_mistaken_for_a_new_user_turn() {
        let mut tracker = ReconnectTracker::default();
        let start = Instant::now();
        tracker.observe_snapshot(running(None));
        assert!(tracker.observe_snapshot(idle(Some("request failed"))).1);
        tracker.mark_recovery_sent();
        assert_eq!(
            tracker.observe_event_at(ReconnectEvent::NewTurn, start).0,
            ReconnectEvent::Ignore
        );
        assert!(!tracker.observe_event(ReconnectEvent::Attempt(5)).1);
    }
}
