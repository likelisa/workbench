use rusqlite::{params,params_from_iter,Connection,Transaction,types::Value as SqlValue};
use serde_json::{Map,Value};
use tauri::AppHandle;
fn stamp()->String{chrono::Utc::now().to_rfc3339()}
fn text<'a>(v:&'a Map<String,Value>,k:&str)->&'a str{v.get(k).and_then(Value::as_str).unwrap_or("")}
fn nullable(v:&Map<String,Value>,k:&str)->Option<String>{let s=text(v,k);if s.is_empty(){None}else{Some(s.to_string())}}
fn exists(tx:&Transaction,table:&str,id:&str)->Result<bool,String>{tx.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?)"),[id],|r|r.get(0)).map_err(|e|e.to_string())}
fn fail<T:std::fmt::Display>(e:T)->String{e.to_string()}
fn migrate_in(c:&mut Connection)->Result<(),String>{
 let tx=c.transaction().map_err(fail)?;
 // Create the project table before adding foreign keys to existing task tables.
 tx.execute_batch("CREATE TABLE IF NOT EXISTS project_types(id TEXT PRIMARY KEY,title TEXT NOT NULL UNIQUE,sort_order INTEGER NOT NULL DEFAULT 0); CREATE TABLE IF NOT EXISTS long_projects(id TEXT PRIMARY KEY,title TEXT NOT NULL,type_id TEXT REFERENCES project_types(id) ON DELETE SET NULL,description TEXT NOT NULL DEFAULT '',focus TEXT NOT NULL DEFAULT '',links TEXT NOT NULL DEFAULT '',status TEXT NOT NULL DEFAULT 'preparing' CHECK(status IN ('preparing','active','paused','archived')),favorite INTEGER NOT NULL DEFAULT 0,created_at TEXT NOT NULL,updated_at TEXT NOT NULL);").map_err(fail)?;
 for table in ["week_assignments","day_tasks","time_entries"]{
  let n:i64=tx.query_row(&format!("SELECT COUNT(*) FROM pragma_table_info('{table}') WHERE name='long_project_id'"),[],|r|r.get(0)).map_err(fail)?;
  if n==0{tx.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN long_project_id TEXT REFERENCES long_projects(id) ON DELETE SET NULL;")).map_err(fail)?;}
 }
 tx.execute_batch(include_str!("long_projects.sql")).map_err(fail)?;
 // Touch both the old and new project when records move or are removed.
 for (table,parent) in [("project_stages","project_id"),("project_plans","project_id"),("project_updates","project_id"),("task_projects","project_id"),("week_assignments","long_project_id"),("day_tasks","long_project_id"),("time_entries","long_project_id")]{
  for op in ["INSERT","UPDATE","DELETE"]{
   let condition=match op{"INSERT"=>format!("id=NEW.{parent}"),"DELETE"=>format!("id=OLD.{parent}"),_=>format!("id=NEW.{parent} OR id=OLD.{parent}")};
   tx.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS activity_{table}_{op} AFTER {op} ON {table} BEGIN UPDATE long_projects SET updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE {condition}; END;")).map_err(fail)?;
  }
 }
 tx.execute_batch("CREATE TRIGGER IF NOT EXISTS activity_task_update AFTER UPDATE ON tasks BEGIN UPDATE long_projects SET updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id IN(SELECT project_id FROM task_projects WHERE task_id=NEW.id); END;").map_err(fail)?;
 for (table,col,parent_table) in [("plan_tasks","task_id","task_projects"),("plan_weeks","assignment_id","week_assignments")]{
  for op in ["INSERT","DELETE"]{let refname=if op=="INSERT"{"NEW"}else{"OLD"};let _= (col,parent_table);
   tx.execute_batch(&format!("CREATE TRIGGER IF NOT EXISTS activity_{table}_{op} AFTER {op} ON {table} BEGIN UPDATE long_projects SET updated_at=strftime('%Y-%m-%dT%H:%M:%fZ','now') WHERE id=(SELECT project_id FROM project_plans WHERE id={refname}.plan_id); END;")).map_err(fail)?;
  }
 }
 let errors:i64=tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check WHERE \"table\" IN ('long_projects','task_projects','project_stages','project_plans','project_updates','plan_tasks','plan_weeks')",[],|r|r.get(0)).map_err(fail)?;
 if errors>0{return Err("长期项目迁移关联校验失败".into());}
 tx.commit().map_err(fail)
}
#[tauri::command]
pub fn init_long_projects(app:AppHandle)->Result<(),String>{
 let dir=super::config_dir(&app)?;let path=dir.join("workbench.db");let mut c=Connection::open(&path).map_err(fail)?;
 c.busy_timeout(std::time::Duration::from_secs(5)).map_err(fail)?;
 let ready:i64=c.query_row("SELECT COUNT(*) FROM pragma_table_info('week_assignments') WHERE name='long_project_id'",[],|r|r.get(0)).map_err(fail)?;
 if ready==0{super::online_backup(&path,&dir.join(format!("before-long-projects-{}.db",chrono::Local::now().format("%Y%m%d-%H%M%S"))))?;}
 c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON").map_err(fail)?;migrate_in(&mut c)
}
fn save_record(tx:&Transaction,table:&str,id:&str,values:&Map<String,Value>,allowed:&str,creating:bool)->Result<(),String>{
 let mut keys=vec![];let mut vals=vec![];
 for (k,v) in values{if !allowed.split(',').any(|a|a==k){return Err(format!("不支持字段 {k}"));}keys.push(k.clone());vals.push(match v{Value::Null=>SqlValue::Null,Value::String(s)=>SqlValue::Text(s.clone()),Value::Number(n)=>SqlValue::Integer(n.as_i64().ok_or("数值无效")?),_=>return Err("字段格式无效".into())});}
 if keys.is_empty(){return Err("没有可保存字段".into());}
 if creating{keys.push("id".into());vals.push(SqlValue::Text(id.into()));tx.execute(&format!("INSERT INTO {table} ({}) VALUES({})",keys.join(","),vec!["?";keys.len()].join(",")),params_from_iter(vals)).map_err(fail)?;}
 else{vals.push(SqlValue::Text(id.into()));let n=tx.execute(&format!("UPDATE {table} SET {} WHERE id=?",keys.iter().map(|k|format!("{k}=?")).collect::<Vec<_>>().join(",")),params_from_iter(vals)).map_err(fail)?;if n==0{return Err("记录已不存在".into());}}
 Ok(())
}
fn save_task_links(tx:&Transaction,task:&str,ids:&[String])->Result<(),String>{
 for p in ids{if !exists(tx,"long_projects",p)?{return Err("项目不存在".into());}}
 // Removing a project from a task doesn't erase existing week project selections.
 tx.execute("DELETE FROM task_projects WHERE task_id=?",[task]).map_err(fail)?;
 for p in ids{tx.execute("INSERT OR IGNORE INTO task_projects VALUES(?,?)",params![task,p]).map_err(fail)?;}
 tx.execute("DELETE FROM plan_tasks WHERE task_id=?1 AND plan_id IN(SELECT id FROM project_plans WHERE project_id NOT IN(SELECT project_id FROM task_projects WHERE task_id=?1))",[task]).map_err(fail)?;Ok(())
}
fn apply(c:&mut Connection,entity:&str,action:&str,id:&str,mut values:Map<String,Value>,ids:Vec<String>,plan_id:Option<String>,weeks:Vec<i64>)->Result<(),String>{
 let tx=c.transaction().map_err(fail)?;
 if action=="task_links"{if !exists(&tx,"tasks",id)?{return Err("本轮待办不存在".into());}save_task_links(&tx,id,&ids)?;}
 else if action=="link"||action=="unlink"{
  let plan=plan_id.as_deref().ok_or("请选择规划事项")?;
  let project:String=tx.query_row("SELECT project_id FROM project_plans WHERE id=?",[plan],|r|r.get(0)).map_err(fail)?;
  if entity=="task"{
   if !exists(&tx,"tasks",id)?{return Err("本轮待办不存在".into());}
   if action=="link"{tx.execute("INSERT OR IGNORE INTO task_projects VALUES(?,?)",params![id,project]).map_err(fail)?;tx.execute("INSERT OR IGNORE INTO plan_tasks VALUES(?,?)",params![plan,id]).map_err(fail)?;}else{tx.execute("DELETE FROM plan_tasks WHERE plan_id=? AND task_id=?",params![plan,id]).map_err(fail)?;}
  }else if entity=="assignment"{
   let p:Option<String>=tx.query_row("SELECT long_project_id FROM week_assignments WHERE id=?",[id],|r|r.get(0)).map_err(fail)?;
   if action=="link"{if p.as_deref()!=Some(&project){return Err("请先手动将周任务归属到当前项目".into());}tx.execute("INSERT OR IGNORE INTO plan_weeks VALUES(?,?)",params![plan,id]).map_err(fail)?;}else{tx.execute("DELETE FROM plan_weeks WHERE plan_id=? AND assignment_id=?",params![plan,id]).map_err(fail)?;}
  }else{return Err("关联类型无效".into());}
 }else{
  let (table,allowed)=match entity{
   "project"=>("long_projects","title,type_id,description,focus,links,status,favorite,created_at,updated_at"),
   "type"=>("project_types","title,sort_order"),
   "stage"=>("project_stages","project_id,title,description,start_date,end_date,status,created_at"),
   "plan"=>("project_plans","project_id,stage_id,title,description,horizon,completed,created_at"),
   "update"=>("project_updates","project_id,date,content,created_at"),
   "task"=>("tasks","title,cycle_id,annual_goal_id,due_date,status,created_at"),
   "assignment"=>("week_assignments","task_id,cycle_id,commitment,review,status,priority,created_at,long_project_id"),
   _=>return Err("对象类型无效".into())
  };
  if action=="delete"{
   if matches!(entity,"task"|"assignment"){return Err("请使用任务删除入口".into());}
   tx.execute(&format!("DELETE FROM {table} WHERE id=?"),[id]).map_err(fail)?;
  }else if action=="save"{
   let creating=!exists(&tx,table,id)?;
   let title_key=if entity=="assignment"{"commitment"}else if entity=="update"{"content"}else{"title"};
   if text(&values,title_key).trim().is_empty(){return Err("请填写内容".into());}
   if entity=="project"{values.insert("updated_at".into(),Value::String(stamp()));}
   if entity=="stage"{let a=text(&values,"start_date");let b=text(&values,"end_date");if !a.is_empty()&&!b.is_empty()&&a>b{return Err("阶段结束日期不能早于开始日期".into());}}
   if entity=="plan"{if let Some(stage)=nullable(&values,"stage_id"){let parent:String=tx.query_row("SELECT project_id FROM project_stages WHERE id=?",[stage],|r|r.get(0)).map_err(fail)?;if parent!=text(&values,"project_id"){return Err("阶段与项目不一致".into());}}}
   if creating&&!matches!(entity,"type"){values.insert("created_at".into(),Value::String(stamp()));}
   if entity=="task"&&creating{if nullable(&values,"cycle_id").is_none()||nullable(&values,"annual_goal_id").is_none(){return Err("请选择年度目标和周期".into());}values.insert("status".into(),Value::String("todo".into()));}
   if entity=="assignment"{
    let cycle:Option<String>=tx.query_row("SELECT cycle_id FROM tasks WHERE id=?",[text(&values,"task_id")],|r|r.get(0)).map_err(fail)?;
    if creating&&cycle.is_none(){return Err("请先为本轮待办指定周期".into());}
    values.insert("cycle_id".into(),cycle.map(Value::String).unwrap_or(Value::Null));
    if creating{values.insert("status".into(),Value::String("todo".into()));}
   }
   save_record(&tx,table,id,&values,allowed,creating)?;
   if entity=="task"{save_task_links(&tx,id,&ids)?;}
   if entity=="assignment"&&creating{for w in weeks{tx.execute("INSERT INTO assignment_weeks VALUES(?,?)",params![id,w]).map_err(fail)?;}}
   if let Some(plan)=plan_id{
    let project:String=tx.query_row("SELECT project_id FROM project_plans WHERE id=?",[&plan],|r|r.get(0)).map_err(fail)?;
    if entity=="task"{tx.execute("INSERT OR IGNORE INTO task_projects VALUES(?,?)",params![id,project]).map_err(fail)?;tx.execute("INSERT OR IGNORE INTO plan_tasks VALUES(?,?)",params![plan,id]).map_err(fail)?;}
    else if entity=="assignment"{if nullable(&values,"long_project_id").as_deref()!=Some(&project){return Err("请手动选择当前项目后再关联规划事项".into());}tx.execute("INSERT OR IGNORE INTO plan_weeks VALUES(?,?)",params![plan,id]).map_err(fail)?;}
    else{return Err("此对象不能关联执行规划".into());}
   }
  }else{return Err("操作类型无效".into());}
 }
 tx.commit().map_err(fail)
}
#[tauri::command]
pub fn project_action(app:AppHandle,entity:String,action:String,row_id:String,values:Map<String,Value>,ids:Vec<String>,plan_id:Option<String>,weeks:Vec<i64>)->Result<(),String>{
 let mut c=Connection::open(super::config_dir(&app)?.join("workbench.db")).map_err(fail)?;
 c.busy_timeout(std::time::Duration::from_secs(5)).map_err(fail)?;
 c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON").map_err(fail)?;
 apply(&mut c,&entity,&action,&row_id,values,ids,plan_id,weeks)
}

#[cfg(test)]
mod tests{
 use super::*;
 fn db()->Connection{
  let mut c=Connection::open_in_memory().unwrap();
  c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON;
   CREATE TABLE tasks(id TEXT PRIMARY KEY,title TEXT NOT NULL,cycle_id TEXT,annual_goal_id TEXT,due_date TEXT,status TEXT,created_at TEXT);
   CREATE TABLE week_assignments(id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(id),cycle_id TEXT,commitment TEXT,status TEXT,review TEXT,created_at TEXT);
   CREATE TABLE assignment_weeks(assignment_id TEXT NOT NULL REFERENCES week_assignments(id) ON DELETE CASCADE,week_no INTEGER CHECK(week_no BETWEEN 1 AND 12),PRIMARY KEY(assignment_id,week_no));
   CREATE TABLE day_tasks(id TEXT PRIMARY KEY,assignment_id TEXT REFERENCES week_assignments(id),task_id TEXT,date TEXT,title TEXT,status TEXT);
   CREATE TABLE time_entries(id TEXT PRIMARY KEY,day_task_id TEXT REFERENCES day_tasks(id),task_id TEXT,title TEXT,start_at TEXT,end_at TEXT);
   CREATE TABLE projects(id TEXT PRIMARY KEY,title TEXT);
   INSERT INTO projects VALUES('legacy','旧兼容项目');
   INSERT INTO tasks VALUES('t','待办','c','g',NULL,'todo','old');
   INSERT INTO tasks VALUES('t2','另一个待办','c2','g2',NULL,'todo','old');
   INSERT INTO week_assignments VALUES('w','t','c','周任务','todo','原复盘','old');
   INSERT INTO assignment_weeks VALUES('w',1);
   INSERT INTO day_tasks VALUES('d','w','t','2026-10-01','日任务','todo');
   INSERT INTO time_entries VALUES('e','d','t','实际记录','2026-10-01T16:00:00Z','2026-10-01T17:00:00Z');").unwrap();
  migrate_in(&mut c).unwrap();c
 }
 fn vals(v:Value)->Map<String,Value>{v.as_object().unwrap().clone()}
 fn save(c:&mut Connection,entity:&str,id:&str,v:Value)->Result<(),String>{apply(c,entity,"save",id,vals(v),vec![],None,vec![])}
 fn projects(c:&mut Connection){for p in ["p","q"]{save(c,"project",p,serde_json::json!({"title":p,"status":"active"})).unwrap();}}
 fn scalar(c:&Connection,sql:&str)->i64{c.query_row(sql,[],|r|r.get(0)).unwrap()}
 #[test] fn migration_preserves_old_ids_and_never_infers_project(){
  let mut c=db();assert_eq!(scalar(&c,"SELECT COUNT(*) FROM projects WHERE id='legacy'"),1);assert_eq!(scalar(&c,"SELECT COUNT(*) FROM week_assignments WHERE id='w' AND review='原复盘' AND long_project_id IS NULL"),1);assert_eq!(scalar(&c,"SELECT COUNT(*) FROM assignment_weeks WHERE assignment_id='w' AND week_no=1"),1);
  c.execute("DELETE FROM project_types WHERE id='media'",[]).unwrap();migrate_in(&mut c).unwrap();assert_eq!(scalar(&c,"SELECT COUNT(*) FROM project_types WHERE id='media'"),0);assert_eq!(scalar(&c,"SELECT COUNT(*) FROM pragma_foreign_key_check"),0);
 }
 #[test] fn manual_project_selection_syncs_day_and_time_but_task_links_do_not(){
  let mut c=db();projects(&mut c);
  apply(&mut c,"task","task_links","t",Map::new(),vec!["p".into(),"q".into()],None,vec![]).unwrap();
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM week_assignments WHERE long_project_id IS NULL"),1);
  save(&mut c,"assignment","w",serde_json::json!({"task_id":"t","commitment":"周任务","long_project_id":"p","review":"保留复盘"})).unwrap();
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM day_tasks WHERE long_project_id='p'"),1);assert_eq!(scalar(&c,"SELECT COUNT(*) FROM time_entries WHERE long_project_id='p'"),1);
  apply(&mut c,"task","task_links","t",Map::new(),vec!["q".into()],None,vec![]).unwrap();
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM week_assignments WHERE long_project_id='p'"),1);
  save(&mut c,"assignment","w",serde_json::json!({"task_id":"t","commitment":"改内容","long_project_id":"p"})).unwrap();
  assert!(save(&mut c,"assignment","w",serde_json::json!({"task_id":"t2","commitment":"不允许","long_project_id":"p"})).is_err());
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM week_assignments WHERE commitment='改内容'"),1);
  save(&mut c,"assignment","w",serde_json::json!({"task_id":"t","commitment":"改内容","long_project_id":null})).unwrap();
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM time_entries WHERE long_project_id IS NULL"),1);
 }
 #[test] fn deletion_unlinks_tasks_but_keeps_historical_attribution(){
  let mut c=db();projects(&mut c);apply(&mut c,"task","task_links","t",Map::new(),vec!["p".into()],None,vec![]).unwrap();
  save(&mut c,"assignment","w",serde_json::json!({"task_id":"t","commitment":"周任务","long_project_id":"p"})).unwrap();
  c.execute("UPDATE day_tasks SET assignment_id=NULL,task_id=NULL WHERE id='d'",[]).unwrap();c.execute("DELETE FROM assignment_weeks",[]).unwrap();c.execute("DELETE FROM week_assignments WHERE id='w'",[]).unwrap();
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM time_entries WHERE long_project_id='p'"),1);
  c.execute("UPDATE time_entries SET day_task_id=NULL WHERE id='e'",[]).unwrap();c.execute("DELETE FROM day_tasks WHERE id='d'",[]).unwrap();assert_eq!(scalar(&c,"SELECT COUNT(*) FROM time_entries WHERE long_project_id='p'"),1);
 }
 #[test] fn failed_plan_link_rolls_back_new_week_and_placement(){
  let mut c=db();projects(&mut c);save(&mut c,"plan","plan",serde_json::json!({"project_id":"p","title":"想法"})).unwrap();
  let result=apply(&mut c,"assignment","save","new",vals(serde_json::json!({"task_id":"t","commitment":"新任务","long_project_id":null})),vec![],Some("plan".into()),vec![2]);assert!(result.is_err());
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM week_assignments WHERE id='new'"),0);assert_eq!(scalar(&c,"SELECT COUNT(*) FROM assignment_weeks WHERE assignment_id='new'"),0);
 }
 #[test] fn plans_share_tasks_without_duplicates_and_project_delete_preserves_execution(){
  let mut c=db();projects(&mut c);for p in ["a","b"]{save(&mut c,"plan",p,serde_json::json!({"project_id":"p","title":p})).unwrap();for _ in 0..2{apply(&mut c,"task","link","t",Map::new(),vec![],Some(p.into()),vec![]).unwrap();}}
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM plan_tasks"),2);assert_eq!(scalar(&c,"SELECT COUNT(*) FROM task_projects"),1);
  save(&mut c,"assignment","w",serde_json::json!({"task_id":"t","commitment":"周任务","long_project_id":"p"})).unwrap();
  apply(&mut c,"project","delete","p",Map::new(),vec![],None,vec![]).unwrap();
  assert_eq!(scalar(&c,"SELECT COUNT(*) FROM project_plans"),0);assert_eq!(scalar(&c,"SELECT COUNT(*) FROM tasks"),2);assert_eq!(scalar(&c,"SELECT COUNT(*) FROM day_tasks"),1);assert_eq!(scalar(&c,"SELECT COUNT(*) FROM time_entries"),1);assert_eq!(scalar(&c,"SELECT COUNT(*) FROM pragma_foreign_key_check"),0);
 }
 #[test] fn reassociation_updates_records_and_stage_delete_preserves_plans(){
  let mut c=db();projects(&mut c);apply(&mut c,"task","task_links","t",Map::new(),vec!["p".into(),"q".into()],None,vec![]).unwrap();
  save(&mut c,"assignment","w",serde_json::json!({"task_id":"t","commitment":"周任务","long_project_id":"p"})).unwrap();
  save(&mut c,"assignment","w2",serde_json::json!({"task_id":"t","commitment":"另一个周任务","long_project_id":"q"})).unwrap();
  c.execute("UPDATE day_tasks SET assignment_id='w2' WHERE id='d'",[]).unwrap();assert_eq!(scalar(&c,"SELECT COUNT(*) FROM time_entries WHERE long_project_id='q'"),1);
  save(&mut c,"stage","s",serde_json::json!({"project_id":"p","title":"版本"})).unwrap();save(&mut c,"plan","a",serde_json::json!({"project_id":"p","title":"想法","stage_id":"s"})).unwrap();
  assert!(save(&mut c,"plan","bad",serde_json::json!({"project_id":"q","title":"错误阶段","stage_id":"s"})).is_err());
  apply(&mut c,"stage","delete","s",Map::new(),vec![],None,vec![]).unwrap();assert_eq!(scalar(&c,"SELECT COUNT(*) FROM project_plans WHERE id='a' AND stage_id IS NULL"),1);
 }
}
