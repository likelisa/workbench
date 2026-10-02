use rusqlite::Connection;
use tauri::AppHandle;
const FIXED:&str="'睡觉','吃饭','娱乐','健身','学习','社交','恋爱','杂事'";
fn upgraded(c:&Connection)->Result<bool,String>{c.query_row("SELECT COUNT(*) FROM pragma_table_info('categories') WHERE name='kind'",[],|r|r.get::<_,i64>(0)).map(|n|n>0).map_err(|e|e.to_string())}
// Replacing categories removes the old global UNIQUE(name), while every original ID remains intact.
fn upgrade(c:&mut Connection)->Result<(),String>{
 let before:i64=c.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 c.execute_batch("PRAGMA foreign_keys=OFF").map_err(|e|e.to_string())?;
 let tx=c.transaction().map_err(|e|e.to_string())?;
 if !upgraded(&tx)? {
  tx.execute_batch(&format!("CREATE TABLE categories_new(id TEXT PRIMARY KEY,name TEXT NOT NULL,color TEXT NOT NULL,created_at TEXT NOT NULL,kind TEXT NOT NULL CHECK(kind IN ('fixed','goal','legacy')),annual_goal_id TEXT REFERENCES annual_goals(id) ON DELETE SET NULL,goal_year INTEGER,active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1)));
   INSERT INTO categories_new(id,name,color,created_at,kind,active) SELECT id,CASE WHEN name='睡眠' AND NOT EXISTS(SELECT 1 FROM categories WHERE name='睡觉') THEN '睡觉' ELSE name END,color,created_at,CASE WHEN name IN ({FIXED}) OR (name='睡眠' AND NOT EXISTS(SELECT 1 FROM categories WHERE name='睡觉')) THEN 'fixed' ELSE 'legacy' END,CASE WHEN name IN ({FIXED}) OR (name='睡眠' AND NOT EXISTS(SELECT 1 FROM categories WHERE name='睡觉')) THEN 1 ELSE 0 END FROM categories;
   DROP TABLE categories; ALTER TABLE categories_new RENAME TO categories;
   CREATE UNIQUE INDEX category_fixed_name ON categories(name) WHERE kind='fixed'; CREATE UNIQUE INDEX category_goal_id ON categories(annual_goal_id) WHERE kind='goal' AND annual_goal_id IS NOT NULL;" )).map_err(|e|e.to_string())?;
  // Fixed IDs may already exist under another UUID: only add categories whose names are missing.
  let fixed=include_str!("time_categories.sql");
  tx.execute_batch(fixed).map_err(|e|e.to_string())?;
 }
 let name="CAST(g.year AS TEXT) || ' · ' || COALESCE(d.title,'未归属方向') || ' · ' || g.title";
 let color="CASE (length(COALESCE(g.direction_id,g.id))+unicode(substr(COALESCE(g.direction_id,g.id),1,1))) % 6 WHEN 0 THEN '#9072ba' WHEN 1 THEN '#d99166' WHEN 2 THEN '#557cb2' WHEN 3 THEN '#5aa88e' WHEN 4 THEN '#b883a0' ELSE '#849b55' END";
 tx.execute_batch(&format!("INSERT OR IGNORE INTO categories(id,name,color,created_at,kind,annual_goal_id,goal_year,active) SELECT 'goal:'||g.id,{name},{color},g.created_at,'goal',g.id,g.year,1 FROM annual_goals g LEFT JOIN directions d ON d.id=g.direction_id;
 CREATE TRIGGER IF NOT EXISTS category_goal_insert AFTER INSERT ON annual_goals BEGIN INSERT INTO categories(id,name,color,created_at,kind,annual_goal_id,goal_year,active) SELECT 'goal:'||g.id,{name},{color},g.created_at,'goal',g.id,g.year,1 FROM annual_goals g LEFT JOIN directions d ON d.id=g.direction_id WHERE g.id=NEW.id; END;
 CREATE TRIGGER IF NOT EXISTS category_goal_update AFTER UPDATE OF title,year,direction_id ON annual_goals BEGIN UPDATE categories SET name=(SELECT {name} FROM annual_goals g LEFT JOIN directions d ON d.id=g.direction_id WHERE g.id=NEW.id),goal_year=NEW.year WHERE annual_goal_id=NEW.id; END;
 CREATE TRIGGER IF NOT EXISTS category_direction_update AFTER UPDATE OF title ON directions BEGIN UPDATE categories SET name=(SELECT {name} FROM annual_goals g LEFT JOIN directions d ON d.id=g.direction_id WHERE g.id=categories.annual_goal_id) WHERE annual_goal_id IN(SELECT id FROM annual_goals WHERE direction_id=NEW.id); END;
 CREATE TRIGGER IF NOT EXISTS category_goal_delete BEFORE DELETE ON annual_goals BEGIN UPDATE categories SET annual_goal_id=NULL,active=0 WHERE annual_goal_id=OLD.id; END;" )).map_err(|e|e.to_string())?;
 for (table,field) in [("time_entries","category_id"),("day_tasks","scheduled_category_id")] {
  tx.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS {table}_category_insert BEFORE INSERT ON {table} WHEN NEW.{field} IS NOT NULL AND NOT EXISTS(SELECT 1 FROM categories WHERE id=NEW.{field} AND active=1 AND kind IN ('fixed','goal')) BEGIN SELECT RAISE(ABORT,'请选择有效的固定类别或年度目标'); END;
  CREATE TRIGGER IF NOT EXISTS {table}_category_update BEFORE UPDATE OF {field} ON {table} WHEN NEW.{field} IS NOT NULL AND NEW.{field} IS NOT OLD.{field} AND NOT EXISTS(SELECT 1 FROM categories WHERE id=NEW.{field} AND active=1 AND kind IN ('fixed','goal')) BEGIN SELECT RAISE(ABORT,'请选择有效的固定类别或年度目标'); END;" )).map_err(|e|e.to_string())?;
 }
 let after:i64=tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;
 if after>before{return Err("时间分类升级产生无效关联，已回滚".into());}
 tx.commit().map_err(|e|e.to_string())?;
 c.execute_batch("PRAGMA foreign_keys=ON").map_err(|e|e.to_string())
}
#[tauri::command]
pub fn init_time_categories(app:AppHandle)->Result<(),String>{
 let dir=super::config_dir(&app)?;let path=dir.join("workbench.db");let mut c=Connection::open(&path).map_err(|e|e.to_string())?;
 c.busy_timeout(std::time::Duration::from_secs(5)).map_err(|e|e.to_string())?;
 if !upgraded(&c)?{super::online_backup(&path,&dir.join(format!("before-time-categories-{}.db",chrono::Local::now().format("%Y%m%d-%H%M%S"))))?;}
 upgrade(&mut c)
}
#[cfg(test)] mod tests {
 use super::*;
 fn fixture()->Connection {let c=Connection::open_in_memory().unwrap();c.execute_batch("CREATE TABLE directions(id TEXT PRIMARY KEY,title TEXT); CREATE TABLE annual_goals(id TEXT PRIMARY KEY,title TEXT,year INTEGER,direction_id TEXT REFERENCES directions(id),created_at TEXT); CREATE TABLE categories(id TEXT PRIMARY KEY,name TEXT UNIQUE,color TEXT,created_at TEXT); CREATE TABLE time_entries(id TEXT,category_id TEXT REFERENCES categories(id),start_at TEXT,end_at TEXT); CREATE TABLE day_tasks(id TEXT,scheduled_category_id TEXT REFERENCES categories(id)); INSERT INTO directions VALUES('d','写作'); INSERT INTO annual_goals VALUES('a','开篇',2026,'d',''),('b','开篇',2026,'d',''); INSERT INTO categories VALUES('sleep','睡眠','#111111',''),('work','写作','#222222',''); INSERT INTO time_entries VALUES('e','work','2026-01-01','2026-01-02');").unwrap();c}
 #[test] fn migration_preserves_ids_and_is_idempotent(){let mut c=fixture();upgrade(&mut c).unwrap();upgrade(&mut c).unwrap();assert_eq!(c.query_row("SELECT name FROM categories WHERE id='sleep'",[],|r|r.get::<_,String>(0)).unwrap(),"睡觉");assert_eq!(c.query_row("SELECT COUNT(*) FROM categories WHERE kind='fixed'",[],|r|r.get::<_,i64>(0)).unwrap(),8);assert_eq!(c.query_row("SELECT COUNT(*) FROM categories WHERE kind='goal'",[],|r|r.get::<_,i64>(0)).unwrap(),2);assert_eq!(c.query_row("SELECT category_id FROM time_entries WHERE id='e'",[],|r|r.get::<_,String>(0)).unwrap(),"work");assert!(c.execute("INSERT INTO time_entries(id,category_id) VALUES('bad','work')",[]).is_err());c.execute("UPDATE time_entries SET category_id='work' WHERE id='e'",[]).unwrap();assert!(c.execute("INSERT INTO day_tasks VALUES('bad','work')",[]).is_err());}
 #[test] fn goal_lifecycle_keeps_history_and_color(){let mut c=fixture();upgrade(&mut c).unwrap();let color:String=c.query_row("SELECT color FROM categories WHERE annual_goal_id='a'",[],|r|r.get(0)).unwrap();assert_eq!(color,c.query_row("SELECT color FROM categories WHERE annual_goal_id='b'",[],|r|r.get::<_,String>(0)).unwrap());c.execute("UPDATE annual_goals SET title='正文',year=2027 WHERE id='a'",[]).unwrap();assert_eq!(c.query_row("SELECT name FROM categories WHERE id='goal:a'",[],|r|r.get::<_,String>(0)).unwrap(),"2027 · 写作 · 正文");c.execute("INSERT INTO time_entries(id,category_id) VALUES('g','goal:a')",[]).unwrap();c.execute("DELETE FROM annual_goals WHERE id='a'",[]).unwrap();assert_eq!(c.query_row("SELECT active FROM categories WHERE id='goal:a'",[],|r|r.get::<_,i64>(0)).unwrap(),0);c.execute("UPDATE time_entries SET category_id='goal:a' WHERE id='g'",[]).unwrap();assert!(c.execute("INSERT INTO time_entries(id,category_id) VALUES('bad','goal:a')",[]).is_err());c.execute("INSERT INTO annual_goals VALUES('new','目标',2028,NULL,'')",[]).unwrap();assert_eq!(c.query_row("SELECT COUNT(*) FROM categories WHERE annual_goal_id='new'",[],|r|r.get::<_,i64>(0)).unwrap(),1);}
 #[test] fn failed_upgrade_rolls_back_and_empty_database_gets_fixed_categories(){
  let mut c=fixture();c.execute_batch("ALTER TABLE annual_goals RENAME COLUMN title TO missing_title;").unwrap();assert!(upgrade(&mut c).is_err());assert!(!upgraded(&c).unwrap());
  let mut empty=fixture();empty.execute_batch("DELETE FROM time_entries; DELETE FROM categories; DELETE FROM annual_goals;").unwrap();upgrade(&mut empty).unwrap();assert_eq!(empty.query_row("SELECT COUNT(*) FROM categories",[],|r|r.get::<_,i64>(0)).unwrap(),8);
 }
 #[test] fn timed_tasks_copy_category_and_conflicts_roll_back(){
  let mut c=fixture();c.execute_batch("ALTER TABLE day_tasks ADD COLUMN start_at TEXT; ALTER TABLE day_tasks ADD COLUMN end_at TEXT; CREATE TRIGGER copy_entry AFTER INSERT ON day_tasks WHEN NEW.start_at IS NOT NULL BEGIN SELECT RAISE(ABORT,'重叠') WHERE EXISTS(SELECT 1 FROM time_entries WHERE start_at<NEW.end_at AND end_at>NEW.start_at); INSERT INTO time_entries(id,category_id,start_at,end_at) VALUES(NEW.id,NEW.scheduled_category_id,NEW.start_at,NEW.end_at); END;").unwrap();upgrade(&mut c).unwrap();
  c.execute("INSERT INTO day_tasks VALUES('new','goal:a','2026-10-02T23:00:00Z','2026-10-03T01:00:00Z')",[]).unwrap();assert_eq!(c.query_row("SELECT category_id FROM time_entries WHERE id='new'",[],|r|r.get::<_,String>(0)).unwrap(),"goal:a");
  {let tx=c.transaction().unwrap();tx.execute("INSERT INTO day_tasks VALUES('batch1','goal:a','2026-10-04T23:00:00Z','2026-10-05T01:00:00Z')",[]).unwrap();assert!(tx.execute("INSERT INTO day_tasks VALUES('batch2','goal:b','2026-10-04T23:30:00Z','2026-10-05T02:00:00Z')",[]).is_err());}
  assert_eq!(c.query_row("SELECT COUNT(*) FROM day_tasks WHERE id LIKE 'batch%'",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT COUNT(*) FROM time_entries WHERE id LIKE 'batch%'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test] fn real_database_copy_preserves_records(){if let Ok(path)=std::env::var("WORKBENCH_CATEGORY_TEST_COPY"){let mut c=Connection::open(path).unwrap();let tables=["time_entries","day_tasks","tasks","week_assignments","annual_goals"];let before:Vec<i64>=tables.iter().map(|t|c.query_row(&format!("SELECT COUNT(*) FROM {t}"),[],|r|r.get(0)).unwrap()).collect();upgrade(&mut c).unwrap();upgrade(&mut c).unwrap();for (t,n) in tables.iter().zip(before){assert_eq!(n,c.query_row(&format!("SELECT COUNT(*) FROM {t}"),[],|r|r.get::<_,i64>(0)).unwrap());}assert_eq!(c.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).unwrap(),0);}}
}
