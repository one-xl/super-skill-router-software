//! Windows desktop-agent automation.
//!
//! The visible Codex window is owned by `ChatGPT.exe`; `codex.exe` is its
//! headless app-server. Claude Desktop exposes a visible `claude.exe` window.
//! We locate the editable composer through UI Automation before pasting so the
//! operation does not depend on whichever control happened to have focus.

const DESKTOP_TARGETS: &[(&str, &[&str])] = &[
    ("codex_desktop", &["ChatGPT.exe", "Codex.exe"]),
    ("claude_code_desktop", &["claude.exe"]),
];

fn target_executables(target_id: &str) -> Option<&'static [&'static str]> {
    DESKTOP_TARGETS
        .iter()
        .find_map(|(id, executables)| (*id == target_id).then_some(*executables))
}

#[cfg(target_os = "windows")]
mod win {
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
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_FORMAT,
        PROCESS_QUERY_LIMITED_INFORMATION,
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

    fn process_path(pid: u32) -> Option<String> {
        unsafe {
            let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
            let mut buffer = vec![0_u16; 32_768];
            let mut length = buffer.len() as u32;
            let result = QueryFullProcessImageNameW(
                process,
                PROCESS_NAME_FORMAT(0),
                windows::core::PWSTR(buffer.as_mut_ptr()),
                &mut length,
            );
            let _ = CloseHandle(process);
            result.ok()?;
            Some(String::from_utf16_lossy(&buffer[..length as usize]))
        }
    }

    pub(super) fn is_desktop_process(name: &str, path: Option<&str>) -> bool {
        if name.eq_ignore_ascii_case("ChatGPT.exe") {
            return path.is_some_and(|value| value.to_ascii_lowercase().contains("codex"));
        }
        true
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
                if names.contains(&name)
                    && is_desktop_process(&name, process_path(entry.th32ProcessID).as_deref())
                {
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

    #[cfg(target_os = "windows")]
    use super::win::is_desktop_process;

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
    fn does_not_treat_the_regular_chatgpt_app_as_codex() {
        assert!(is_desktop_process(
            "ChatGPT.exe",
            Some(r"C:\Program Files\WindowsApps\OpenAI.Codex_1.0\app\ChatGPT.exe")
        ));
        assert!(!is_desktop_process(
            "ChatGPT.exe",
            Some(r"C:\Program Files\WindowsApps\OpenAI.ChatGPT_1.0\app\ChatGPT.exe")
        ));
        assert!(is_desktop_process("Codex.exe", None));
    }
}
