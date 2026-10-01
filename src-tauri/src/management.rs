use rusqlite::{Connection, params, params_from_iter, types::Value as SqlValue};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::path::Path;
use tauri::AppHandle;

pub fn migrate(path:&Path, dir:&Path)->Result<(),String>{
    if !path.exists(){return Ok(());}
    let mut c=Connection::open(path).map_err(|e|e.to_string())?;
    let annual:i64=c.query_row("SELECT COUNT(*) FROM pragma_table_info('annual_goals') WHERE name='direction_id' AND \"notnull\"=1",[],|r|r.get(0)).map_err(|e|e.to_string())?;
    let weeks:i64=c.query_row("SELECT COUNT(*) FROM pragma_table_info('week_assignments') WHERE name='cycle_id' AND \"notnull\"=1",[],|r|r.get(0)).map_err(|e|e.to_string())?;
    if annual+weeks==0{return Ok(());}
    super::online_backup(path,&dir.join(format!("before-management-{}.db",chrono::Local::now().format("%Y%m%d-%H%M%S"))))?;
    let before:i64=c.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;
    c.execute_batch("PRAGMA foreign_keys=OFF").map_err(|e|e.to_string())?;
    let tx=c.transaction().map_err(|e|e.to_string())?;
    if annual>0{
        let completed:i64=tx.query_row("SELECT COUNT(*) FROM pragma_table_info('annual_goals') WHERE name='completed'",[],|r|r.get(0)).map_err(|e|e.to_string())?;
        if completed==0{tx.execute_batch("ALTER TABLE annual_goals ADD COLUMN completed INTEGER NOT NULL DEFAULT 0").map_err(|e|e.to_string())?;}
        tx.execute_batch("CREATE TABLE annual_goals_new (id TEXT PRIMARY KEY,direction_id TEXT REFERENCES directions(id),title TEXT NOT NULL,year INTEGER NOT NULL,completed INTEGER NOT NULL DEFAULT 0,created_at TEXT NOT NULL); INSERT INTO annual_goals_new SELECT id,direction_id,title,year,completed,created_at FROM annual_goals; DROP TABLE annual_goals; ALTER TABLE annual_goals_new RENAME TO annual_goals;").map_err(|e|e.to_string())?;
    }
    if weeks>0{
        // A trigger on day_tasks references tasks, so keep all IDs while replacing only this table.
        tx.execute_batch("DROP TRIGGER IF EXISTS sync_week_cycle_on_task_change; CREATE TABLE week_assignments_new (id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),cycle_id TEXT REFERENCES cycles(id),commitment TEXT NOT NULL,status TEXT NOT NULL,review TEXT NOT NULL DEFAULT '',created_at TEXT NOT NULL); INSERT INTO week_assignments_new SELECT id,task_id,cycle_id,commitment,status,review,created_at FROM week_assignments; DROP TABLE week_assignments; ALTER TABLE week_assignments_new RENAME TO week_assignments;").map_err(|e|e.to_string())?;
    }
    let after:i64=tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get(0)).map_err(|e|e.to_string())?;
    if after>before{return Err("迁移产生无效关联，已回滚".into());}
    tx.commit().map_err(|e|e.to_string())
}
fn connection(app:&AppHandle)->Result<Connection,String>{
    let c=Connection::open(super::config_dir(app)?.join("workbench.db")).map_err(|e|e.to_string())?;
    c.busy_timeout(std::time::Duration::from_secs(5)).map_err(|e|e.to_string())?;
    c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON").map_err(|e|e.to_string())?;
    Ok(c)
}
#[tauri::command]
pub fn delete_direction(app:AppHandle,row_id:String)->Result<(),String>{
    delete_direction_in(&mut connection(&app)?,&row_id)
}
fn delete_direction_in(c:&mut Connection,row_id:&str)->Result<(),String>{
    let tx=c.transaction().map_err(|e|e.to_string())?;
    tx.execute("UPDATE annual_goals SET direction_id=NULL WHERE direction_id=?",[&row_id]).map_err(|e|e.to_string())?;
    tx.execute("DELETE FROM directions WHERE id=?",[&row_id]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e|e.to_string())
}
#[tauri::command]
pub fn delete_cycle(app:AppHandle,row_id:String)->Result<(),String>{
    delete_cycle_in(&mut connection(&app)?,&row_id)
}
fn delete_cycle_in(c:&mut Connection,row_id:&str)->Result<(),String>{
    let tx=c.transaction().map_err(|e|e.to_string())?;
    for sql in [
        "DELETE FROM assignment_weeks WHERE assignment_id IN (SELECT id FROM week_assignments WHERE cycle_id=? OR task_id IN (SELECT id FROM tasks WHERE cycle_id=?1))",
        "UPDATE projects SET cycle_goal_id=NULL WHERE cycle_goal_id IN (SELECT id FROM cycle_goals WHERE cycle_id=?)",
        "UPDATE tasks SET cycle_goal_id=NULL WHERE cycle_goal_id IN (SELECT id FROM cycle_goals WHERE cycle_id=?)",
        "DELETE FROM cycle_goals WHERE cycle_id=?",
        "UPDATE week_assignments SET cycle_id=NULL WHERE cycle_id=?",
        "UPDATE tasks SET cycle_id=NULL WHERE cycle_id=?",
        "DELETE FROM cycles WHERE id=?"
    ]{tx.execute(sql,[&row_id]).map_err(|e|e.to_string())?;}
    tx.commit().map_err(|e|e.to_string())
}
#[derive(Deserialize)]
pub struct BatchRow{values:Map<String,Value>,#[serde(default)] weeks:Vec<i64>}
fn text<'a>(v:&'a Map<String,Value>,key:&str)->&'a str{v.get(key).and_then(Value::as_str).unwrap_or("")}
fn insert_rows(c:&mut Connection,kind:&str,rows:Vec<BatchRow>)->Result<usize,String>{
    let (table,allowed,required)=match kind{
        "direction"=>("directions","id,title,created_at","id,title,created_at"),
        "annual"=>("annual_goals","id,title,direction_id,year,completed,created_at","id,title,direction_id,created_at"),
        "task"=>("tasks","id,title,annual_goal_id,cycle_id,due_date,status,created_at","id,title,annual_goal_id,cycle_id,status,created_at"),
        "assignment"=>("week_assignments","id,task_id,cycle_id,commitment,status,review,created_at","id,task_id,cycle_id,commitment,status,created_at"),
        "dayTask"=>("day_tasks","id,date,title,status,assignment_id,task_id,project_id,scheduled_start_at,scheduled_end_at,scheduled_category_id,created_at","id,date,title,status,created_at"),
        _=>return Err("不支持的批量类型".into())
    };
    if rows.is_empty(){return Err("请填写至少一行".into());}
    let tx=c.transaction().map_err(|e|e.to_string())?;
    let count=rows.len();
    for (index,row) in rows.into_iter().enumerate(){
        let run=||->Result<(),String>{
            for key in required.split(','){if text(&row.values,key).trim().is_empty(){return Err(format!("请填写 {key}"));}}
            if kind=="annual" && row.values.get("year").and_then(Value::as_i64).is_none(){return Err("请填写有效年份".into());}
            if let Some(status)=row.values.get("status"){if !matches!(status.as_str(),Some("todo"|"doing"|"done"|"missed")){return Err("任务状态无效".into());}}
            if kind=="assignment"{
                let parent:Option<String>=tx.query_row("SELECT cycle_id FROM tasks WHERE id=?",[text(&row.values,"task_id")],|r|r.get(0)).map_err(|e|e.to_string())?;
                if parent.as_deref()!=Some(text(&row.values,"cycle_id")){return Err("本轮待办和周期不一致".into());}
            }
            if kind=="dayTask"{
                if chrono::NaiveDate::parse_from_str(text(&row.values,"date"),"%Y-%m-%d").is_err(){return Err("日期无效".into());}
                let assignment=text(&row.values,"assignment_id");
                if !assignment.is_empty(){
                    let valid:i64=tx.query_row("SELECT COUNT(*) FROM week_assignments w JOIN cycles c ON c.id=w.cycle_id JOIN assignment_weeks a ON a.assignment_id=w.id WHERE w.id=?1 AND ?2>=date(c.start_date,'+' || ((a.week_no-1)*7) || ' days') AND ?2<date(c.start_date,'+' || (a.week_no*7) || ' days') AND w.task_id=?3",params![assignment,text(&row.values,"date"),text(&row.values,"task_id")],|r|r.get(0)).map_err(|e|e.to_string())?;
                    if valid==0{return Err("日任务日期所在周未安排此周任务".into());}
                }
                let start=text(&row.values,"scheduled_start_at");let end=text(&row.values,"scheduled_end_at");let cat=text(&row.values,"scheduled_category_id");
                if !start.is_empty()||!end.is_empty()||!cat.is_empty(){
                    let a=chrono::DateTime::parse_from_rfc3339(start).map_err(|_|"开始时间无效")?;
                    let b=chrono::DateTime::parse_from_rfc3339(end).map_err(|_|"结束时间无效")?;
                    if b<=a||cat.is_empty(){return Err("请完整填写具体时间及分类".into());}
                }
            }
            let mut keys=vec![];let mut values=vec![];
            for (key,value) in &row.values{
                if !allowed.split(',').any(|field|field==key){return Err("不支持的字段".into());}
                keys.push(key.as_str());values.push(match value{Value::Null=>SqlValue::Null,Value::String(s)=>SqlValue::Text(s.clone()),Value::Number(n)=>SqlValue::Integer(n.as_i64().ok_or("数值无效")?),_=>return Err("字段格式无效".into())});
            }
            tx.execute(&format!("INSERT INTO {table} ({}) VALUES ({})",keys.join(","),vec!["?";keys.len()].join(",")),params_from_iter(values)).map_err(|e|e.to_string())?;
            if kind=="assignment"{for week in &row.weeks{tx.execute("INSERT INTO assignment_weeks (assignment_id,week_no) VALUES (?,?)",params![text(&row.values,"id"),week]).map_err(|e|e.to_string())?;}}
            Ok(())
        };
        run().map_err(|e|format!("第 {} 行：{e}",index+1))?;
    }
    tx.commit().map_err(|e|e.to_string())?;Ok(count)
}
#[tauri::command]
pub fn batch_create(app:AppHandle,kind:String,rows:Vec<BatchRow>)->Result<usize,String>{insert_rows(&mut connection(&app)?,&kind,rows)}

#[cfg(test)]
mod tests {
 use super::*;
 const SCHEMA:&str="PRAGMA foreign_keys=ON;
 CREATE TABLE cycles(id TEXT PRIMARY KEY,title TEXT,start_date TEXT,created_at TEXT);
 CREATE TABLE directions(id TEXT PRIMARY KEY,title TEXT,created_at TEXT);
 CREATE TABLE annual_goals(id TEXT PRIMARY KEY,direction_id TEXT REFERENCES directions(id),title TEXT NOT NULL,year INTEGER NOT NULL,completed INTEGER NOT NULL DEFAULT 0,created_at TEXT NOT NULL);
 CREATE TABLE cycle_goals(id TEXT PRIMARY KEY,cycle_id TEXT REFERENCES cycles(id),annual_goal_id TEXT REFERENCES annual_goals(id));
 CREATE TABLE projects(id TEXT PRIMARY KEY,cycle_goal_id TEXT REFERENCES cycle_goals(id));
 CREATE TABLE tasks(id TEXT PRIMARY KEY,title TEXT,cycle_id TEXT REFERENCES cycles(id),annual_goal_id TEXT REFERENCES annual_goals(id),cycle_goal_id TEXT REFERENCES cycle_goals(id),project_id TEXT,status TEXT,due_date TEXT,created_at TEXT);
 CREATE TABLE week_assignments(id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),cycle_id TEXT REFERENCES cycles(id),commitment TEXT,status TEXT,review TEXT,created_at TEXT);
 CREATE TABLE assignment_weeks(assignment_id TEXT REFERENCES week_assignments(id),week_no INTEGER CHECK(week_no BETWEEN 1 AND 12),PRIMARY KEY(assignment_id,week_no));
 CREATE TABLE categories(id TEXT PRIMARY KEY);
 CREATE TABLE day_tasks(id TEXT PRIMARY KEY,date TEXT,title TEXT,status TEXT,assignment_id TEXT REFERENCES week_assignments(id),task_id TEXT REFERENCES tasks(id),project_id TEXT,scheduled_start_at TEXT,scheduled_end_at TEXT,scheduled_category_id TEXT REFERENCES categories(id),created_at TEXT);
 CREATE TABLE time_entries(id TEXT PRIMARY KEY,start_at TEXT,end_at TEXT,title TEXT,category_id TEXT REFERENCES categories(id),day_task_id TEXT REFERENCES day_tasks(id),project_id TEXT,task_id TEXT REFERENCES tasks(id),created_at TEXT);
 INSERT INTO directions VALUES('dir','方向',''); INSERT INTO annual_goals VALUES('a','dir','目标',2026,1,''); INSERT INTO cycles VALUES('c','周期','2026-10-01',''); INSERT INTO tasks(id,title,cycle_id,annual_goal_id,status,created_at) VALUES('t','任务','c','a','todo',''); INSERT INTO categories VALUES('cat');";
 fn database()->Connection{let c=Connection::open_in_memory().unwrap();c.execute_batch(SCHEMA).unwrap();let src=include_str!("../../src/db.ts");let begin=src.find("CREATE TRIGGER IF NOT EXISTS create_time_entry_for_timed_day_task").unwrap();let end=src[begin..].find("END\");").unwrap()+3;c.execute_batch(&src[begin..begin+end]).unwrap();c}
 fn row(v:Value)->BatchRow{BatchRow{values:v.as_object().unwrap().clone(),weeks:vec![]}}
 fn timed(uid:&str,start:&str,end:&str)->BatchRow{row(serde_json::json!({"id":uid,"title":"面试","date":"2026-10-01","status":"todo","created_at":"now","scheduled_start_at":start,"scheduled_end_at":end,"scheduled_category_id":"cat","assignment_id":null,"task_id":null,"project_id":null}))}
 #[test] fn batch_failure_rolls_back_every_row(){let mut c=database();let result=insert_rows(&mut c,"direction",vec![row(serde_json::json!({"id":"one","title":"有效","created_at":"now"})),row(serde_json::json!({"id":"two","title":"","created_at":"now"}))]);assert!(result.unwrap_err().contains("第 2 行"));assert_eq!(c.query_row("SELECT COUNT(*) FROM directions WHERE id='one'",[],|r|r.get::<_,i64>(0)).unwrap(),0);}
 #[test] fn timing_conflict_rolls_back_tasks_and_entries(){let mut c=database();let start="2026-10-01T15:00:00.000Z";let end="2026-10-02T00:00:00.000Z";let result=insert_rows(&mut c,"dayTask",vec![timed("one",start,end),timed("two","2026-10-01T16:00:00.000Z",end)]);assert!(result.unwrap_err().contains("第 2 行"));for table in ["day_tasks","time_entries"]{assert_eq!(c.query_row(&format!("SELECT COUNT(*) FROM {table}"),[],|r|r.get::<_,i64>(0)).unwrap(),0);}assert_eq!(insert_rows(&mut c,"dayTask",vec![timed("night",start,end)]).unwrap(),1);assert!(insert_rows(&mut c,"dayTask",vec![timed("conflict",start,end)]).is_err());assert_eq!(c.query_row("SELECT COUNT(*) FROM time_entries",[],|r|r.get::<_,i64>(0)).unwrap(),1);}
 #[test] fn deleting_parents_preserves_records(){let mut c=database();c.execute_batch("INSERT INTO week_assignments VALUES('w','t','c','周任务','doing','复盘',''); INSERT INTO assignment_weeks VALUES('w',1); INSERT INTO day_tasks(id,date,title,status,assignment_id,task_id,created_at) VALUES('d','2026-10-01','日任务','todo','w','t',''); INSERT INTO time_entries VALUES('e','start','end','记录','cat','d',NULL,'t','');").unwrap();delete_direction_in(&mut c,"dir").unwrap();assert_eq!(c.query_row("SELECT completed FROM annual_goals WHERE id='a' AND direction_id IS NULL",[],|r|r.get::<_,i64>(0)).unwrap(),1);delete_cycle_in(&mut c,"c").unwrap();assert_eq!(c.query_row("SELECT COUNT(*) FROM tasks WHERE id='t' AND cycle_id IS NULL AND annual_goal_id='a'",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert_eq!(c.query_row("SELECT COUNT(*) FROM week_assignments WHERE id='w' AND cycle_id IS NULL",[],|r|r.get::<_,i64>(0)).unwrap(),1);assert_eq!(c.query_row("SELECT COUNT(*) FROM assignment_weeks",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert_eq!(c.query_row("SELECT assignment_id FROM day_tasks WHERE id='d'",[],|r|r.get::<_,String>(0)).unwrap(),"w");assert_eq!(c.query_row("SELECT day_task_id FROM time_entries WHERE id='e'",[],|r|r.get::<_,String>(0)).unwrap(),"d");}
 #[test] fn migration_keeps_ids_and_can_restart(){let dir=std::env::temp_dir().join(format!("workbench-migration-{}",chrono::Utc::now().timestamp_nanos_opt().unwrap()));std::fs::create_dir_all(&dir).unwrap();let path=dir.join("workbench.db");let c=Connection::open(&path).unwrap();let old=SCHEMA.replace("direction_id TEXT REFERENCES","direction_id TEXT NOT NULL REFERENCES").replace("task_id TEXT NOT NULL REFERENCES tasks(id),cycle_id TEXT REFERENCES","task_id TEXT NOT NULL REFERENCES tasks(id),cycle_id TEXT NOT NULL REFERENCES");c.execute_batch(&old).unwrap();c.execute_batch("INSERT INTO week_assignments VALUES('w','t','c','周任务','doing','复盘',''); INSERT INTO assignment_weeks VALUES('w',1); INSERT INTO day_tasks(id,date,assignment_id) VALUES('d','2026-10-01','w'); CREATE TRIGGER sync_week_cycle_on_task_change AFTER UPDATE OF cycle_id ON tasks BEGIN UPDATE week_assignments SET cycle_id=NEW.cycle_id WHERE task_id=NEW.id; END;").unwrap();drop(c);migrate(&path,&dir).unwrap();migrate(&path,&dir).unwrap();let c=Connection::open(&path).unwrap();assert_eq!(c.query_row("SELECT assignment_id FROM day_tasks WHERE id='d'",[],|r|r.get::<_,String>(0)).unwrap(),"w");assert_eq!(c.query_row("SELECT completed FROM annual_goals WHERE id='a'",[],|r|r.get::<_,i64>(0)).unwrap(),1);let invalid:i64=c.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get(0)).unwrap();assert_eq!(invalid,0);drop(c);std::fs::remove_dir_all(dir).unwrap();}
}
