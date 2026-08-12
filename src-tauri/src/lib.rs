mod automation;
mod converter;
mod fetcher;
mod install;
mod management;
mod monitor;
mod packager;
mod scanner;
mod settings;
mod skillsmp;
mod targets;
mod translator;

fn installation_migrations() -> Vec<tauri_plugin_sql::Migration> {
    vec![tauri_plugin_sql::Migration {
        version: 1,
        description: "create_installation_records",
        sql: "CREATE TABLE IF NOT EXISTS installation_records (id INTEGER PRIMARY KEY AUTOINCREMENT, skill_name TEXT NOT NULL, directory_name TEXT NOT NULL, repository TEXT NOT NULL, source_url TEXT NOT NULL, commit_sha TEXT NOT NULL, target TEXT NOT NULL, status TEXT NOT NULL, installed_path TEXT, installed_at TEXT NOT NULL, updated_at TEXT NOT NULL, UNIQUE(directory_name, target)); CREATE INDEX IF NOT EXISTS idx_installation_records_skill ON installation_records(skill_name);",
        kind: tauri_plugin_sql::MigrationKind::Up,
    }, tauri_plugin_sql::Migration {
        version: 2,
        description: "add_package_path",
        sql: "ALTER TABLE installation_records ADD COLUMN package_path TEXT;",
        kind: tauri_plugin_sql::MigrationKind::Up,
    }]
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() -> tauri::Result<()> {
    use tauri::Manager;

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:super-skill-router.db", installation_migrations())
                .build(),
        )
        .manage(install::PendingInstallStore::default())
        .manage(management::PendingUninstallStore::default())
        .manage(monitor::DesktopMonitorSupervisor::default())
        .manage(scanner::ScanProcessSupervisor::default())
        .invoke_handler(tauri::generate_handler![
            scanner::scan_skill,
            settings::get_settings,
            settings::save_settings,
            settings::refine_prompt,
            skillsmp::search_skillsmp,
            install::prepare_skill_install,
            install::scan_prepared_skill,
            install::install_prepared_skill,
            install::reveal_packaged_skill,
            targets::detect_skill_targets,
            converter::convert_requirement,
            management::list_installed_skills,
            management::read_installed_skill_markdown,
            management::prepare_skill_uninstall,
            management::commit_skill_uninstall,
            management::rollback_skill_uninstall,
            translator::translate_markdown,
            automation::inject_text_to_agent,
            monitor::start_desktop_monitor,
            monitor::stop_desktop_monitor,
            monitor::list_desktop_monitors,
        ])
        .build(tauri::generate_context!())?;

    app.run(|app_handle, event| match event {
        tauri::RunEvent::WindowEvent {
            event: tauri::WindowEvent::CloseRequested { .. },
            ..
        }
        | tauri::RunEvent::ExitRequested { .. } => {
            app_handle
                .state::<monitor::DesktopMonitorSupervisor>()
                .request_shutdown();
            app_handle
                .state::<scanner::ScanProcessSupervisor>()
                .kill_all();
        }
        _ => {}
    });
    Ok(())
}
