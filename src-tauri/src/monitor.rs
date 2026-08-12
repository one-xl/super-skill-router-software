//! Desktop reconnect monitoring.
//!
//! Codex Desktop exposes reconnect attempts in its own log as
//! `reconnectAttempt=<n>`. The monitor tails the current desktop log and only
//! triggers recovery when the application itself reaches attempt 5. It never
//! starts a CLI process and never treats unrelated network warnings as a
//! reconnect attempt.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::{mpsc, Mutex};

use crate::automation;

const MAX_FAILURES: u32 = 5;
const RECOVERY_TEXT: &str = "继续并恢复todo-list";

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
}

#[derive(Debug, PartialEq)]
enum ReconnectEvent {
    Attempt(u32),
    Connected,
    Ignore,
}

impl ReconnectTracker {
    fn observe(&mut self, line: &str) -> (ReconnectEvent, bool) {
        let event = parse_codex_connection_event(line);
        match event {
            ReconnectEvent::Attempt(attempt) => {
                self.attempt = attempt;
                if attempt < MAX_FAILURES {
                    self.fired_for_attempt_five = false;
                }
                let should_recover = attempt >= MAX_FAILURES && !self.fired_for_attempt_five;
                if should_recover {
                    self.fired_for_attempt_five = true;
                }
                (ReconnectEvent::Attempt(attempt), should_recover)
            }
            ReconnectEvent::Connected => {
                self.attempt = 0;
                self.fired_for_attempt_five = false;
                (ReconnectEvent::Connected, false)
            }
            ReconnectEvent::Ignore => (ReconnectEvent::Ignore, false),
        }
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
    if !line.contains("[AppServerConnection]") || !line.contains("state_changed") {
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
        let (stop_tx, stop_rx) = mpsc::channel(1);
        let status = DesktopMonitorStatus {
            target_id: target_id.clone(),
            target_label: "ChatGPT Desktop (Codex)".to_string(),
            state: MonitorState::Watching,
            reconnect_attempt: 0,
            recovery_sent_count: 0,
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
            app, target_id, log_root, log_path, stop_rx, shared,
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
                    loop {
                        line.clear();
                        let read = reader.read_line(&mut line).await.map_err(|error| format!("读取 Codex Desktop 日志失败：{error}"))?;
                        if read == 0 { break; }
                        let (event, should_recover) = tracker.observe(&line);
                        match event {
                            ReconnectEvent::Attempt(attempt) => {
                                update_status(&app, &monitors, &target_id, |status| {
                                    status.state = MonitorState::Reconnecting;
                                    status.reconnect_attempt = attempt;
                                    status.last_error = None;
                                }).await;
                            }
                            ReconnectEvent::Connected => {
                                update_status(&app, &monitors, &target_id, |status| {
                                    status.state = MonitorState::Watching;
                                    status.reconnect_attempt = 0;
                                    status.last_error = None;
                                }).await;
                            }
                            ReconnectEvent::Ignore => {}
                        }

                        if should_recover {
                            let target = target_id.clone();
                            let recovery = tauri::async_runtime::spawn_blocking(move || {
                                automation::send_text_to_desktop(&target, RECOVERY_TEXT, true)
                            }).await.map_err(|error| format!("自动恢复任务异常结束：{error}"))?;
                            update_status(&app, &monitors, &target_id, |status| match recovery {
                                Ok(()) => {
                                    status.recovery_sent_count += 1;
                                    status.last_error = None;
                                }
                                Err(error) => {
                                    status.state = MonitorState::Error;
                                    status.last_error = Some(error);
                                }
                            }).await;
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
    use super::{parse_codex_connection_event, ReconnectEvent, ReconnectTracker};

    fn line(attempt: u32) -> String {
        format!("info [AppServerConnection] app_server_connection.state_changed next=connecting reconnectAttempt={attempt}")
    }

    #[test]
    fn ignores_unrelated_retry_text() {
        assert_eq!(
            parse_codex_connection_event("tool retrying after network error"),
            ReconnectEvent::Ignore
        );
    }

    #[test]
    fn fires_once_when_attempt_five_is_reached() {
        let mut tracker = ReconnectTracker::default();
        for attempt in 1..5 {
            assert!(!tracker.observe(&line(attempt)).1);
        }
        assert!(tracker.observe(&line(5)).1);
        assert!(!tracker.observe(&line(5)).1);
    }

    #[test]
    fn connected_state_resets_the_recovery_guard() {
        let mut tracker = ReconnectTracker::default();
        assert!(tracker.observe(&line(5)).1);
        tracker.observe("info [AppServerConnection] app_server_connection.state_changed next=connected reconnectAttempt=0");
        assert!(tracker.observe(&line(5)).1);
    }
}
