mod converter;
mod fetcher;
mod install;
mod packager;
mod scanner;
mod targets;

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
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_sql::Builder::default()
                .add_migrations("sqlite:super-skill-router.db", installation_migrations())
                .build(),
        )
        .manage(install::PendingInstallStore::default())
        .invoke_handler(tauri::generate_handler![
            scanner::scan_skill,
            install::prepare_skill_install,
            install::install_prepared_skill,
            install::reveal_packaged_skill,
            targets::detect_skill_targets,
            converter::convert_requirement
        ])
        .run(tauri::generate_context!())
}
