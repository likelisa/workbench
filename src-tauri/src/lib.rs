mod management;
use chrono::Local;
use rusqlite::{backup::Backup, Connection};
use std::{fs, path::{Path, PathBuf}, time::Duration};
use tauri::{AppHandle, Manager};

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn online_backup(source: &Path, destination: &Path) -> Result<(), String> {
    if !source.exists() { return Err("数据库尚未创建".into()); }
    if let Some(parent) = destination.parent() { fs::create_dir_all(parent).map_err(|e| e.to_string())?; }
    let temporary = destination.with_extension("db.tmp");
    if temporary.exists() { fs::remove_file(&temporary).map_err(|e| e.to_string())?; }
    let src = Connection::open(source).map_err(|e| e.to_string())?;
    let mut dst = Connection::open(&temporary).map_err(|e| e.to_string())?;
    let backup = Backup::new(&src, &mut dst).map_err(|e| e.to_string())?;
    backup.run_to_completion(100, Duration::from_millis(10), None).map_err(|e| e.to_string())?;
    drop(backup);
    let check: String = dst.query_row("PRAGMA integrity_check", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    if check != "ok" { return Err(format!("备份校验失败：{check}")); }
    drop(dst);
    fs::rename(&temporary, destination).map_err(|e| e.to_string())?;
    Ok(())
}

fn migrate_week_assignments(db_path: &Path, dir: &Path) -> Result<(), String> {
    if !db_path.exists() { return Ok(()); }
    let mut conn = Connection::open(db_path).map_err(|e| e.to_string())?;
    let has_old_column: i64 = conn.query_row(
        "SELECT COUNT(*) FROM pragma_table_info('week_assignments') WHERE name='week_no'", [], |row| row.get(0)
    ).map_err(|e| e.to_string())?;
    if has_old_column == 0 { return Ok(()); }
    let old_invalid: i64 = conn.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    let rescue = dir.join(format!("before-week-pool-{}.db", Local::now().format("%Y%m%d-%H%M%S")));
    online_backup(db_path, &rescue)?;
    conn.execute_batch("PRAGMA foreign_keys=OFF;").map_err(|e| e.to_string())?;
    let transaction = conn.transaction().map_err(|e| e.to_string())?;
    transaction.execute_batch("\
        CREATE TABLE week_assignments_new (id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id), cycle_id TEXT NOT NULL REFERENCES cycles(id), commitment TEXT NOT NULL, status TEXT NOT NULL, review TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL);\
        INSERT INTO week_assignments_new (id,task_id,cycle_id,commitment,status,review,created_at) SELECT id,task_id,cycle_id,commitment,status,review,created_at FROM week_assignments;\
        CREATE TABLE assignment_weeks (assignment_id TEXT NOT NULL REFERENCES week_assignments(id) ON DELETE CASCADE, week_no INTEGER NOT NULL CHECK(week_no BETWEEN 1 AND 12), PRIMARY KEY(assignment_id,week_no));\
        INSERT INTO assignment_weeks (assignment_id,week_no) SELECT id,week_no FROM week_assignments;\
        DROP TABLE week_assignments;\
        ALTER TABLE week_assignments_new RENAME TO week_assignments;\
        CREATE INDEX idx_assignment_weeks_week ON assignment_weeks(week_no);")
        .map_err(|e| e.to_string())?;
    let invalid: i64 = transaction.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| row.get(0)).map_err(|e| e.to_string())?;
    if invalid > old_invalid { return Err(format!("周任务迁移后发现新的无效关联；迁移前备份：{}", rescue.display())); }
    transaction.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn backup_database(app: AppHandle, manual: bool, destination: Option<String>) -> Result<String, String> {
    let dir = config_dir(&app)?;
    let source = dir.join("workbench.db");
    let target = if manual {
        let chosen = destination.ok_or("请选择备份位置")?;
        let path = PathBuf::from(chosen);
        if path == source { return Err("不能覆盖当前数据库".into()); }
        path
    } else {
        let backup_dir = dir.join("backups");
        fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
        let date = Local::now().format("%Y-%m-%d").to_string();
        let path = backup_dir.join(format!("workbench-{date}.db"));
        path
    };
    online_backup(&source, &target)?;
    if !manual {
        let mut files: Vec<PathBuf> = fs::read_dir(dir.join("backups")).map_err(|e| e.to_string())?
            .filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "db")).collect();
        files.sort();
        for old in files.iter().take(files.len().saturating_sub(30)) { let _ = fs::remove_file(old); }
    }
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
fn list_backups(app: AppHandle) -> Result<Vec<String>, String> {
    let folder = config_dir(&app)?.join("backups");
    if !folder.exists() { return Ok(vec![]); }
    let mut files: Vec<String> = fs::read_dir(folder).map_err(|e| e.to_string())?
        .filter_map(Result::ok).map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "db"))
        .map(|p| p.to_string_lossy().into_owned()).collect();
    files.sort_by(|a,b| b.cmp(a)); Ok(files)
}

#[tauri::command]
fn stage_restore(app: AppHandle, path: String) -> Result<(), String> {
    let source = PathBuf::from(path);
    if !source.is_file() { return Err("找不到备份文件".into()); }
    let dir = config_dir(&app)?;
    let staged = dir.join("restore-pending.db");
    online_backup(&source, &staged)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_sql::Builder::default().build())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let handle = app.handle();
            let dir = config_dir(handle).map_err(std::io::Error::other)?;
            let staged = dir.join("restore-pending.db");
            if staged.exists() {
                let db = dir.join("workbench.db");
                if db.exists() {
                    let rescue = dir.join(format!("before-restore-{}.db", Local::now().format("%Y%m%d-%H%M%S")));
                    online_backup(&db, &rescue).map_err(std::io::Error::other)?;
                }
                // Restore through the backup API before the SQL plugin opens the database.
                if db.exists() { fs::remove_file(&db)?; }
                online_backup(&staged, &db).map_err(std::io::Error::other)?;
                fs::remove_file(staged)?;
            }
            migrate_week_assignments(&dir.join("workbench.db"), &dir).map_err(std::io::Error::other)?;
            management::migrate(&dir.join("workbench.db"), &dir).map_err(std::io::Error::other)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![backup_database, list_backups, stage_restore, management::delete_direction, management::delete_cycle, management::batch_create])
        .run(tauri::generate_context!())
        .expect("启动个人工作台失败");
}
