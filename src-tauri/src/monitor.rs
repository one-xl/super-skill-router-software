//! Desktop reconnect monitoring.
//!
//! The reconnect counter and run/idle state come from ChatGPT Desktop's own UI
//! Automation buttons. Logs are tailed only to follow turn and file rotation;
//! delayed log lines never trigger recovery. The monitor never starts a CLI.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::{mpsc, Mutex};

use crate::automation::CodexDesktopActivity;
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
}

struct MonitorEntry {
    status: DesktopMonitorStatus,
    stop_tx: mpsc::Sender<()>,
}

#[derive(Default)]
pub struct DesktopMonitorSupervisor {
    monitors: Arc<Mutex<HashMap<String, MonitorEntry>>>,
}

#[derive(Default, Debug)]
struct ReconnectTracker {
    attempt: u32,
    fired_for_attempt_five: bool,
    pending_recovery: bool,
    awaiting_recovery_turn: bool,
    recovery_turn_deadline: Option<Instant>,
    running_seen: bool,
    terminal_failure_baseline: Option<bool>,
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
                if attempt >= MAX_FAILURES && !self.fired_for_attempt_five {
                    self.fired_for_attempt_five = true;
                    self.pending_recovery = true;
                }
                (ReconnectEvent::Attempt(attempt), false)
            }
            ReconnectEvent::Connected => {
                if !self.pending_recovery && !self.awaiting_recovery_turn {
                    self.rearm_for_next_turn();
                }
                (ReconnectEvent::Connected, false)
            }
            ReconnectEvent::NewTurn => {
                if self.pending_recovery
                    || self
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
        self.pending_recovery = false;
        self.awaiting_recovery_turn = true;
        self.recovery_turn_deadline = Some(Instant::now() + Duration::from_secs(5));
    }

    fn observe_activity(&mut self, activity: CodexDesktopActivity) -> (ReconnectEvent, bool) {
        match activity {
            CodexDesktopActivity::Reconnecting(attempt) => {
                self.observe_event(ReconnectEvent::Attempt(attempt))
            }
            CodexDesktopActivity::Running {
                terminal_failure_visible,
            } => {
                if self.awaiting_recovery_turn {
                    self.rearm_for_next_turn();
                }
                if self.terminal_failure_baseline.is_none() {
                    self.terminal_failure_baseline = Some(terminal_failure_visible);
                } else if terminal_failure_visible && self.terminal_failure_baseline == Some(false)
                {
                    self.terminal_failure_observed = true;
                }
                self.running_seen = true;
                (ReconnectEvent::NewTurn, false)
            }
            CodexDesktopActivity::Idle {
                terminal_failure_visible,
            } => {
                let new_terminal_failure = self.running_seen
                    && terminal_failure_visible
                    && (self.terminal_failure_observed
                        || self.terminal_failure_baseline == Some(false))
                    && !self.terminal_failure_handled;
                let should_recover =
                    (self.pending_recovery || new_terminal_failure) && !self.awaiting_recovery_turn;
                if should_recover {
                    self.pending_recovery = false;
                    self.terminal_failure_handled = true;
                }
                (ReconnectEvent::Connected, should_recover)
            }
            CodexDesktopActivity::Unknown => (ReconnectEvent::Ignore, false),
        }
    }

    fn rearm_for_next_turn(&mut self) {
        self.attempt = 0;
        self.fired_for_attempt_five = false;
        self.pending_recovery = false;
        self.awaiting_recovery_turn = false;
        self.recovery_turn_deadline = None;
        self.running_seen = false;
        self.terminal_failure_baseline = None;
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
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let status = DesktopMonitorStatus {
            target_id: target_id.clone(),
            target_label: "ChatGPT Desktop (Codex)".to_string(),
            state: MonitorState::Watching,
            reconnect_attempt: 0,
            recovery_sent_count: 0,
            recovery_text: recovery_text.clone(),
            last_error: None,
            log_path: Some(log_path.display().to_string()),
        };
        monitors.insert(
            target_id.clone(),
            MonitorEntry {
                status: status.clone(),
                stop_tx,
            },
        );
        drop(monitors);
        let _ = app.emit("desktop-monitor-status", &status);

        let shared = Arc::clone(&self.monitors);
        tokio::spawn(tail_codex_log(
            app,
            target_id,
            log_root,
            log_path,
            recovery_text,
            stop_rx,
            shared,
        ));
        Ok(())
    }

    pub async fn stop(&self, target_id: &str) -> Result<(), String> {
        let monitors = self.monitors.lock().await;
        let entry = monitors
            .get(target_id)
            .ok_or_else(|| "该桌面 Agent 没有运行中的监控。".to_string())?;
        entry
            .stop_tx
            .send(())
            .await
            .map_err(|_| "监控任务已经结束。".to_string())
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

async fn tail_codex_log(
    app: AppHandle,
    target_id: String,
    log_root: PathBuf,
    initial_log_path: PathBuf,
    recovery_text: String,
    mut stop_rx: mpsc::Receiver<()>,
    monitors: Arc<Mutex<HashMap<String, MonitorEntry>>>,
) {
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
        let mut ui_poll_ticks = 0_u8;

        loop {
            tokio::select! {
                _ = stop_rx.recv() => break,
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
                    ui_poll_ticks = ui_poll_ticks.saturating_add(1);
                    if ui_poll_ticks >= 3 {
                        ui_poll_ticks = 0;
                        let desktop_activity = tauri::async_runtime::spawn_blocking(
                            automation::codex_desktop_activity,
                        )
                        .await
                        .map_err(|error| format!("读取 ChatGPT Desktop 重连状态失败：{error}"))?;
                        match desktop_activity {
                            Ok(activity) => {
                                let (event, should_recover) = tracker.observe_activity(activity);
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

    update_status(&app, &monitors, &target_id, |status| match result {
        Ok(()) => status.state = MonitorState::Stopped,
        Err(error) => {
            status.state = MonitorState::Error;
            status.last_error = Some(error);
        }
    })
    .await;
    monitors.lock().await.remove(&target_id);
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
    supervisor: tauri::State<'_, DesktopMonitorSupervisor>,
    target_id: String,
) -> Result<(), String> {
    supervisor.stop(&target_id).await
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

    use crate::automation::CodexDesktopActivity;

    use super::{parse_codex_connection_event, ReconnectEvent, ReconnectTracker};

    fn line(attempt: u32) -> String {
        format!("info [AppServerConnection] app_server_connection.state_changed next=connecting reconnectAttempt={attempt}")
    }

    fn running(terminal_failure_visible: bool) -> CodexDesktopActivity {
        CodexDesktopActivity::Running {
            terminal_failure_visible,
        }
    }

    fn idle(terminal_failure_visible: bool) -> CodexDesktopActivity {
        CodexDesktopActivity::Idle {
            terminal_failure_visible,
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
    fn waits_for_idle_after_attempt_five() {
        let mut tracker = ReconnectTracker::default();
        for attempt in 1..5 {
            assert!(!tracker.observe(&line(attempt)).1);
        }
        assert!(!tracker.observe(&line(5)).1);
        assert!(!tracker.observe(&line(5)).1);
        tracker.observe_activity(CodexDesktopActivity::Running {
            terminal_failure_visible: false,
        });
        assert!(
            tracker
                .observe_activity(CodexDesktopActivity::Idle {
                    terminal_failure_visible: false
                })
                .1
        );
        assert!(
            !tracker
                .observe_activity(CodexDesktopActivity::Idle {
                    terminal_failure_visible: false
                })
                .1
        );
    }

    #[test]
    fn connected_event_preserves_pending_recovery_until_idle_ui() {
        let mut tracker = ReconnectTracker::default();
        tracker.observe(&line(5));
        tracker.observe_event(ReconnectEvent::Connected);
        tracker.observe(&line(5));
        tracker.observe_activity(CodexDesktopActivity::Running {
            terminal_failure_visible: false,
        });
        assert!(
            tracker
                .observe_activity(CodexDesktopActivity::Idle {
                    terminal_failure_visible: false
                })
                .1
        );
    }

    #[test]
    fn connected_log_does_not_clear_pending_recovery_before_idle_ui() {
        let mut tracker = ReconnectTracker::default();
        tracker.observe_event(ReconnectEvent::Attempt(5));
        tracker.observe_event(ReconnectEvent::Connected);
        tracker.observe_activity(CodexDesktopActivity::Running {
            terminal_failure_visible: false,
        });
        assert!(
            tracker
                .observe_activity(CodexDesktopActivity::Idle {
                    terminal_failure_visible: false
                })
                .1
        );
    }

    #[test]
    fn desktop_ui_attempt_uses_the_same_single_recovery_guard() {
        let mut tracker = ReconnectTracker::default();
        assert!(!tracker.observe_event(ReconnectEvent::Attempt(5)).1);
        assert!(!tracker.observe_event(ReconnectEvent::Attempt(5)).1);
        tracker.observe_activity(CodexDesktopActivity::Running {
            terminal_failure_visible: false,
        });
        assert!(
            tracker
                .observe_activity(CodexDesktopActivity::Idle {
                    terminal_failure_visible: false
                })
                .1
        );
    }

    #[test]
    fn a_user_turn_rearms_when_the_recovery_turn_is_not_observed() {
        let mut tracker = ReconnectTracker::default();
        let start = Instant::now();
        tracker.observe_event(ReconnectEvent::Attempt(5));
        tracker.observe_activity(CodexDesktopActivity::Running {
            terminal_failure_visible: false,
        });
        assert!(
            tracker
                .observe_activity(CodexDesktopActivity::Idle {
                    terminal_failure_visible: false
                })
                .1
        );
        tracker.mark_recovery_sent();
        assert_eq!(
            tracker
                .observe_event_at(ReconnectEvent::NewTurn, start + Duration::from_secs(6))
                .0,
            ReconnectEvent::NewTurn
        );
        tracker.observe_event(ReconnectEvent::Attempt(5));
        tracker.observe_activity(CodexDesktopActivity::Running {
            terminal_failure_visible: false,
        });
        assert!(
            tracker
                .observe_activity(CodexDesktopActivity::Idle {
                    terminal_failure_visible: false
                })
                .1
        );
    }

    #[test]
    fn immediate_recovery_turn_is_not_mistaken_for_a_new_user_turn() {
        let mut tracker = ReconnectTracker::default();
        let start = Instant::now();
        tracker.observe_event(ReconnectEvent::Attempt(5));
        tracker.observe_activity(CodexDesktopActivity::Running {
            terminal_failure_visible: false,
        });
        assert!(
            tracker
                .observe_activity(CodexDesktopActivity::Idle {
                    terminal_failure_visible: false
                })
                .1
        );
        tracker.mark_recovery_sent();
        assert_eq!(
            tracker.observe_event_at(ReconnectEvent::NewTurn, start).0,
            ReconnectEvent::Ignore
        );
        assert!(!tracker.observe_event(ReconnectEvent::Attempt(5)).1);
    }

    #[test]
    fn delayed_original_turn_log_cannot_cancel_pending_recovery() {
        let mut tracker = ReconnectTracker::default();
        tracker.observe_event(ReconnectEvent::Attempt(5));
        assert_eq!(
            tracker.observe_event(ReconnectEvent::NewTurn).0,
            ReconnectEvent::Ignore
        );
        tracker.observe_activity(CodexDesktopActivity::Running {
            terminal_failure_visible: false,
        });
        assert!(
            tracker
                .observe_activity(CodexDesktopActivity::Idle {
                    terminal_failure_visible: false
                })
                .1
        );
    }

    #[test]
    fn running_then_idle_rearms_same_conversation_for_second_recovery() {
        let mut tracker = ReconnectTracker::default();
        tracker.observe_event(ReconnectEvent::Attempt(5));
        tracker.observe_activity(CodexDesktopActivity::Running {
            terminal_failure_visible: false,
        });
        assert!(
            tracker
                .observe_activity(CodexDesktopActivity::Idle {
                    terminal_failure_visible: false
                })
                .1
        );
        tracker.mark_recovery_sent();

        assert!(!tracker.observe_activity(running(false)).1);
        tracker.observe_event(ReconnectEvent::Attempt(5));
        assert!(tracker.observe_activity(idle(false)).1);
    }

    #[test]
    fn lower_reconnect_attempt_rearms_when_running_state_was_missed() {
        let mut tracker = ReconnectTracker::default();
        tracker.observe_event(ReconnectEvent::Attempt(5));
        tracker.observe_activity(running(false));
        assert!(tracker.observe_activity(idle(false)).1);
        tracker.mark_recovery_sent();

        tracker.observe_event(ReconnectEvent::Attempt(1));
        tracker.observe_event(ReconnectEvent::Attempt(5));
        assert!(tracker.observe_activity(idle(false)).1);
    }

    #[test]
    fn new_terminal_failure_after_running_triggers_recovery_once() {
        let mut tracker = ReconnectTracker::default();
        assert!(!tracker.observe_activity(running(false)).1);
        assert!(tracker.observe_activity(idle(true)).1);
        assert!(!tracker.observe_activity(idle(true)).1);
    }

    #[test]
    fn terminal_failure_that_appears_while_still_running_triggers_on_idle() {
        let mut tracker = ReconnectTracker::default();
        assert!(!tracker.observe_activity(running(false)).1);
        assert!(!tracker.observe_activity(running(true)).1);
        assert!(tracker.observe_activity(idle(true)).1);
    }

    #[test]
    fn historical_terminal_failure_does_not_trigger_recovery() {
        let mut tracker = ReconnectTracker::default();
        assert!(!tracker.observe_activity(running(true)).1);
        assert!(!tracker.observe_activity(idle(true)).1);
    }

    #[test]
    fn terminal_failure_rearms_for_the_next_same_conversation_turn() {
        let mut tracker = ReconnectTracker::default();
        tracker.observe_activity(running(false));
        assert!(tracker.observe_activity(idle(true)).1);
        tracker.mark_recovery_sent();

        assert!(!tracker.observe_activity(running(false)).1);
        assert!(tracker.observe_activity(idle(true)).1);
    }
}
