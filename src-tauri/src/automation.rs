//! Windows desktop-agent automation.
//!
//! The current Codex desktop product is named ChatGPT Desktop and its visible
//! window is owned by `ChatGPT.exe`; `codex.exe` is the headless app-server.
//! Claude Desktop exposes a visible `claude.exe` window.
//! We locate the editable composer through UI Automation before pasting so the
//! operation does not depend on whichever control happened to have focus.

const DESKTOP_TARGETS: &[(&str, &[&str])] = &[
    ("codex_desktop", &["ChatGPT.exe", "Codex.exe"]),
    ("claude_code_desktop", &["claude.exe"]),
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CodexDesktopActivity {
    Reconnecting(u32),
    Running { terminal_failure_visible: bool },
    Idle { terminal_failure_visible: bool },
    Unknown,
}

fn target_executables(target_id: &str) -> Option<&'static [&'static str]> {
    DESKTOP_TARGETS
        .iter()
        .find_map(|(id, executables)| (*id == target_id).then_some(*executables))
}

#[cfg(target_os = "windows")]
mod win {
    use super::{target_executables, CodexDesktopActivity};
    use std::collections::HashSet;
    use std::thread;
    use std::time::Duration;

    use uiautomation::types::ControlType;
    use uiautomation::{UIAutomation, UIElement};
    use windows::Win32::Foundation::{CloseHandle, BOOL, HWND, LPARAM};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GetWindowThreadProcessId, IsWindowVisible, SetForegroundWindow, ShowWindow,
        SW_RESTORE,
    };

    struct WindowSearch {
        pids: HashSet<u32>,
        result: Option<HWND>,
    }

    unsafe extern "system" fn enum_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let search = &mut *(lparam.0 as *mut WindowSearch);
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if search.pids.contains(&pid) && IsWindowVisible(hwnd).as_bool() {
            search.result = Some(hwnd);
            return BOOL(0);
        }
        BOOL(1)
    }

    fn matching_pids(executables: &[&str]) -> Result<HashSet<u32>, String> {
        let names = executables
            .iter()
            .map(|name| name.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        let mut pids = HashSet::new();

        unsafe {
            let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0)
                .map_err(|error| format!("无法枚举桌面应用进程：{error}"))?;
            let mut entry = PROCESSENTRY32W {
                dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
                ..Default::default()
            };
            let mut next = Process32FirstW(snapshot, &mut entry);
            while next.is_ok() {
                let end = entry
                    .szExeFile
                    .iter()
                    .position(|value| *value == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..end]).to_ascii_lowercase();
                if names.contains(&name) {
                    pids.insert(entry.th32ProcessID);
                }
                next = Process32NextW(snapshot, &mut entry);
            }
            let _ = CloseHandle(snapshot);
        }

        Ok(pids)
    }

    fn visible_window(executables: &[&str]) -> Result<HWND, String> {
        let pids = matching_pids(executables)?;
        if pids.is_empty() {
            return Err(format!(
                "未检测到 {}，请先启动对应的桌面应用。",
                executables.join(" / ")
            ));
        }
        let mut search = WindowSearch { pids, result: None };
        unsafe {
            let _ = EnumWindows(Some(enum_window), LPARAM(&mut search as *mut _ as isize));
        }
        search
            .result
            .ok_or_else(|| "检测到桌面应用进程，但没有找到可见聊天窗口。".to_string())
    }

    fn composer(root: &UIElement) -> Result<UIElement, String> {
        let automation =
            UIAutomation::new().map_err(|error| format!("UI Automation 初始化失败：{error}"))?;
        let candidates = automation
            .create_matcher()
            .from_ref(root)
            .control_type(ControlType::Group)
            .filter_fn(Box::new(|element: &UIElement| {
                Ok(element
                    .get_classname()?
                    .split_whitespace()
                    .any(|name| name == "ProseMirror")
                    && element.is_keyboard_focusable()?)
            }))
            .depth(32)
            .timeout(1500)
            .find_all()
            .map_err(|_| {
                "未找到桌面应用的可编辑对话框，请先打开一个可输入消息的会话。".to_string()
            })?;

        candidates
            .into_iter()
            .filter_map(|element| {
                element
                    .get_bounding_rectangle()
                    .ok()
                    .filter(|rect| rect.get_width() > 100 && rect.get_height() > 16)
                    .map(|rect| (rect.get_bottom(), element))
            })
            .max_by_key(|(bottom, _)| *bottom)
            .map(|(_, element)| element)
            .ok_or_else(|| "找到了对话控件，但它当前不可见或不可输入。".to_string())
    }

    pub(super) fn reconnect_attempt_from_text(text: &str) -> Option<u32> {
        const MARKERS: &[&str] = &["正在重新连接", "reconnecting"];
        let lower = text.to_ascii_lowercase();
        let marker_end = MARKERS.iter().find_map(|marker| {
            if marker.is_ascii() {
                lower.find(marker).map(|index| index + marker.len())
            } else {
                text.find(marker).map(|index| index + marker.len())
            }
        })?;
        let tail = &text[marker_end..];
        let digits = tail
            .chars()
            .skip_while(|character| !character.is_ascii_digit())
            .take_while(|character| character.is_ascii_digit())
            .collect::<String>();
        let attempt = digits.parse::<u32>().ok()?;
        let remainder = &tail[digits.len()..];
        remainder
            .contains('/')
            .then_some(attempt)
            .filter(|attempt| *attempt > 0)
    }

    pub fn codex_desktop_activity() -> Result<CodexDesktopActivity, String> {
        let executables = target_executables("codex_desktop")
            .ok_or_else(|| "Codex Desktop 目标未配置。".to_string())?;
        let hwnd = visible_window(executables)?;
        let automation =
            UIAutomation::new().map_err(|error| format!("UI Automation 初始化失败：{error}"))?;
        let root = automation
            .element_from_handle((hwnd.0 as isize).into())
            .map_err(|error| format!("无法读取 ChatGPT Desktop 控件：{error}"))?;
        // ChatGPT Desktop renders the transient reconnect state as a button
        // (`正在重新连接 1 /5`). Limiting the scan to buttons prevents prose in
        // a conversation from being mistaken for a live reconnect indicator.
        let buttons = automation
            .create_matcher()
            .from_ref(&root)
            .control_type(ControlType::Button)
            .depth(32)
            .timeout(0)
            .find_all()
            .unwrap_or_default();

        let names = buttons
            .into_iter()
            .filter_map(|element| element.get_name().ok())
            .collect::<Vec<_>>();
        if let Some(attempt) = names
            .iter()
            .find_map(|text| reconnect_attempt_from_text(text))
        {
            return Ok(CodexDesktopActivity::Reconnecting(attempt));
        }
        let terminal_failure_visible = terminal_failure_near_composer(&automation, &root);
        if names.iter().any(|name| is_stop_button(name)) {
            return Ok(CodexDesktopActivity::Running {
                terminal_failure_visible,
            });
        }
        if names.iter().any(|name| is_send_button(name)) {
            return Ok(CodexDesktopActivity::Idle {
                terminal_failure_visible,
            });
        }
        Ok(CodexDesktopActivity::Unknown)
    }

    fn terminal_failure_near_composer(automation: &UIAutomation, root: &UIElement) -> bool {
        let Ok(composer_rect) = composer(root).and_then(|element| {
            element
                .get_bounding_rectangle()
                .map_err(|error| format!("无法读取输入框位置：{error}"))
        }) else {
            return false;
        };
        let candidates = automation
            .create_matcher()
            .from_ref(root)
            .control_type(ControlType::Text)
            .depth(32)
            .timeout(0)
            .find_all()
            .unwrap_or_default();

        candidates.into_iter().any(|element| {
            let Ok(name) = element.get_name() else {
                return false;
            };
            if !is_terminal_failure_text(&name) {
                return false;
            }
            let Ok(rect) = element.get_bounding_rectangle() else {
                return false;
            };
            let vertical_gap = composer_rect.get_top() - rect.get_bottom();
            let center_x = (rect.get_left() + rect.get_right()) / 2;
            (0..=180).contains(&vertical_gap)
                && center_x >= composer_rect.get_left()
                && center_x <= composer_rect.get_right()
        })
    }

    pub(super) fn is_terminal_failure_text(text: &str) -> bool {
        const MARKERS: &[&str] = &[
            "exceeded retry limit",
            "too many requests",
            "rate limit exceeded",
            "retry limit exceeded",
            "已超过重试次数",
            "超过重试限制",
            "请求过多",
        ];
        let lower = text.to_ascii_lowercase();
        MARKERS.iter().any(|marker| lower.contains(marker))
    }

    fn normalized_button_name(name: &str) -> String {
        name.trim().to_ascii_lowercase()
    }

    pub(super) fn is_stop_button(name: &str) -> bool {
        matches!(
            normalized_button_name(name).as_str(),
            "停止" | "停止生成" | "stop" | "stop generating"
        )
    }

    pub(super) fn is_send_button(name: &str) -> bool {
        matches!(
            normalized_button_name(name).as_str(),
            "发送" | "send" | "发送消息" | "send message"
        )
    }

    pub fn send_to_desktop(executables: &[&str], text: &str, submit: bool) -> Result<(), String> {
        if text.trim().is_empty() {
            return Err("不能注入空 Prompt。".to_string());
        }

        let hwnd = visible_window(executables)?;
        unsafe {
            let _ = ShowWindow(hwnd, SW_RESTORE);
            if !SetForegroundWindow(hwnd).as_bool() {
                return Err("无法将桌面应用窗口置前，请关闭系统的焦点限制后重试。".to_string());
            }
        }
        thread::sleep(Duration::from_millis(180));

        let automation =
            UIAutomation::new().map_err(|error| format!("UI Automation 初始化失败：{error}"))?;
        let root = automation
            .element_from_handle((hwnd.0 as isize).into())
            .map_err(|error| format!("无法读取桌面应用控件：{error}"))?;
        let editor = composer(&root)?;
        editor
            .send_text_by_clipboard(text)
            .map_err(|error| format!("无法将文本填入桌面对话框：{error}"))?;
        if submit {
            thread::sleep(Duration::from_millis(80));
            editor
                .send_keys("{enter}", 0)
                .map_err(|error| format!("文本已填入，但自动发送失败：{error}"))?;
        }
        Ok(())
    }
}

pub fn send_text_to_desktop(target_id: &str, text: &str, submit: bool) -> Result<(), String> {
    let executables =
        target_executables(target_id).ok_or_else(|| format!("不支持的桌面 Agent：{target_id}"))?;

    #[cfg(target_os = "windows")]
    {
        win::send_to_desktop(executables, text, submit)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (executables, text, submit);
        Err("桌面对话框自动化仅支持 Windows。".to_string())
    }
}

pub fn codex_desktop_activity() -> Result<CodexDesktopActivity, String> {
    #[cfg(target_os = "windows")]
    {
        win::codex_desktop_activity()
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("桌面对话框自动化仅支持 Windows。".to_string())
    }
}

#[tauri::command]
pub async fn inject_text_to_agent(
    target_id: String,
    text: String,
    submit: bool,
) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || send_text_to_desktop(&target_id, &text, submit))
        .await
        .map_err(|error| format!("桌面自动化任务异常结束：{error}"))?
}

#[cfg(test)]
mod tests {
    use super::target_executables;

    #[test]
    fn accepts_only_desktop_targets() {
        assert_eq!(
            target_executables("codex_desktop").unwrap()[0],
            "ChatGPT.exe"
        );
        assert_eq!(
            target_executables("claude_code_desktop").unwrap()[0],
            "claude.exe"
        );
        assert!(target_executables("codex_cli").is_none());
        assert!(target_executables("claude_code").is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn parses_reconnect_status_from_desktop_text() {
        assert_eq!(
            super::win::reconnect_attempt_from_text("正在重新连接 5/5"),
            Some(5)
        );
        assert_eq!(
            super::win::reconnect_attempt_from_text("Reconnecting 3/5"),
            Some(3)
        );
        assert_eq!(
            super::win::reconnect_attempt_from_text("正在重新连接 1 /5"),
            Some(1)
        );
        assert_eq!(
            super::win::reconnect_attempt_from_text("正在重新连接"),
            None
        );
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn recognizes_desktop_run_and_idle_buttons() {
        assert!(super::win::is_stop_button("停止"));
        assert!(super::win::is_stop_button("Stop generating"));
        assert!(super::win::is_send_button("发送"));
        assert!(super::win::is_send_button("Send message"));
        assert!(!super::win::is_send_button("跳转到用户消息 2"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn recognizes_terminal_retry_failures() {
        assert!(super::win::is_terminal_failure_text(
            "exceeded retry limit, last status: 429 Too Many Requests"
        ));
        assert!(super::win::is_terminal_failure_text(
            "请求过多，已超过重试次数"
        ));
        assert!(!super::win::is_terminal_failure_text("正在重新连接 3/5"));
    }
}
