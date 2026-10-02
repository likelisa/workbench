use rusqlite::Connection;
use tauri::AppHandle;
fn has(c:&Connection, table:&str, field:&str)->Result<bool,String>{
 c.query_row(&format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name=?"),[field],|r|r.get::<_,i64>(0)).map(|n|n>0).map_err(|e|e.to_string())
}
fn upgrade(c:&mut Connection)->Result<(),String>{
 let tx=c.transaction().map_err(|e|e.to_string())?;
 if !has(&tx,"annual_goals","status")? {
  tx.execute_batch("ALTER TABLE annual_goals ADD COLUMN status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN ('todo','done','missed')); UPDATE annual_goals SET status=CASE WHEN completed=1 THEN 'done' ELSE 'missed' END;").map_err(|e|e.to_string())?;
 }
 for table in ["week_assignments","day_tasks"] {
  if !has(&tx,table,"priority")? {tx.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN priority TEXT CHECK(priority IS NULL OR priority IN ('P0','P1','P2','P3'));" )).map_err(|e|e.to_string())?;}
 }
 tx.execute_batch("CREATE TRIGGER IF NOT EXISTS annual_status_insert AFTER INSERT ON annual_goals WHEN NEW.completed != CASE WHEN NEW.status='done' THEN 1 ELSE 0 END BEGIN UPDATE annual_goals SET completed=CASE WHEN NEW.status='done' THEN 1 ELSE 0 END WHERE id=NEW.id; END; CREATE TRIGGER IF NOT EXISTS annual_status_update AFTER UPDATE OF status,completed ON annual_goals WHEN NEW.completed != CASE WHEN NEW.status='done' THEN 1 ELSE 0 END BEGIN UPDATE annual_goals SET completed=CASE WHEN NEW.status='done' THEN 1 ELSE 0 END WHERE id=NEW.id; END;").map_err(|e|e.to_string())?;
 let bad:i64=tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if bad>0{return Err("存在无效外键，升级已回滚".into());}
 tx.commit().map_err(|e|e.to_string())
}
#[tauri::command]
pub fn init_task_management(app:AppHandle)->Result<(),String>{
 let dir=super::config_dir(&app)?; let path=dir.join("workbench.db");
 let mut c=Connection::open(&path).map_err(|e|e.to_string())?;
 c.busy_timeout(std::time::Duration::from_secs(5)).map_err(|e|e.to_string())?;
 if !has(&c,"annual_goals","status")?||!has(&c,"week_assignments","priority")?||!has(&c,"day_tasks","priority")? {
  super::online_backup(&path,&dir.join(format!("before-task-management-{}.db",chrono::Local::now().format("%Y%m%d-%H%M%S"))))?;
 }
 upgrade(&mut c)
}
#[cfg(test)] mod tests {
 use super::*;
 #[test] fn preserves_old_states_ids_and_validates_values(){
  let mut c=Connection::open_in_memory().unwrap();
  c.execute_batch("PRAGMA recursive_triggers=ON; CREATE TABLE annual_goals(id TEXT PRIMARY KEY,completed INTEGER); CREATE TABLE week_assignments(id TEXT PRIMARY KEY); CREATE TABLE day_tasks(id TEXT PRIMARY KEY); INSERT INTO annual_goals VALUES('a',0),('b',1); INSERT INTO week_assignments VALUES('w'); INSERT INTO day_tasks VALUES('d');").unwrap();
  upgrade(&mut c).unwrap();upgrade(&mut c).unwrap();
  let states:Vec<String>=c.prepare("SELECT status FROM annual_goals ORDER BY id").unwrap().query_map([],|r|r.get(0)).unwrap().map(Result::unwrap).collect();assert_eq!(states,vec!["missed","done"]);
  c.execute("INSERT INTO annual_goals(id,completed) VALUES('new',0)",[]).unwrap();assert_eq!(c.query_row("SELECT status FROM annual_goals WHERE id='new'",[],|r|r.get::<_,String>(0)).unwrap(),"todo");
  c.execute("UPDATE annual_goals SET status='done' WHERE id='a'",[]).unwrap();assert_eq!(c.query_row("SELECT completed FROM annual_goals WHERE id='a'",[],|r|r.get::<_,i64>(0)).unwrap(),1);
  assert!(c.execute("UPDATE annual_goals SET status='doing'",[]).is_err());assert!(c.execute("UPDATE day_tasks SET priority='P4'",[]).is_err());
  c.execute("UPDATE week_assignments SET priority='P0'",[]).unwrap();assert_eq!(c.query_row("SELECT priority FROM day_tasks WHERE id='d'",[],|r|r.get::<_,Option<String>>(0)).unwrap(),None);
  c.execute("UPDATE week_assignments SET priority=NULL",[]).unwrap();
 }
 #[test] fn failed_upgrade_rolls_back(){
  let mut c=Connection::open_in_memory().unwrap();c.execute_batch("CREATE TABLE annual_goals(id TEXT,completed INTEGER); CREATE TABLE week_assignments(id TEXT); CREATE TABLE day_tasks(id TEXT,parent TEXT REFERENCES week_assignments(id));").unwrap();assert!(upgrade(&mut c).is_err());assert!(!has(&c,"annual_goals","status").unwrap());
 }
}
