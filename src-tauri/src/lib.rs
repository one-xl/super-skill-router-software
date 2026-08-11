mod fetcher;
mod install;
mod scanner;
mod targets;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> tauri::Result<()> {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(install::PendingInstallStore::default())
        .invoke_handler(tauri::generate_handler![
            scanner::scan_skill,
            install::prepare_claude_code_install,
            install::install_prepared_claude_code,
            targets::detect_skill_targets
        ])
        .run(tauri::generate_context!())
}
