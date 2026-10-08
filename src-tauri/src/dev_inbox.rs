use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{fs, path::Path, sync::Mutex, time::Duration};
use tauri::AppHandle;

static LOCK: Mutex<()> = Mutex::new(());
fn err(e: impl std::fmt::Display) -> String { e.to_string() }
pub(crate) fn schema(c: &Connection) -> Result<(), String> {
    c.execute_batch("PRAGMA foreign_keys=ON;
      CREATE TABLE IF NOT EXISTS dev_inbox_events(id TEXT PRIMARY KEY, imported_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS dev_inbox_todos(
        id TEXT PRIMARY KEY, source_key TEXT NOT NULL, source_name TEXT NOT NULL, source_root TEXT NOT NULL,
        title TEXT NOT NULL, body TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'todo',
        linked_project_id TEXT REFERENCES long_projects(id) ON DELETE SET NULL,
        created_at TEXT NOT NULL, updated_at TEXT NOT NULL, deleted INTEGER NOT NULL DEFAULT 0);
      CREATE TABLE IF NOT EXISTS dev_inbox_sources(
        todo_id TEXT NOT NULL REFERENCES dev_inbox_todos(id), event_id TEXT NOT NULL,
        session_id TEXT NOT NULL, quote TEXT NOT NULL, captured_at TEXT NOT NULL,
        PRIMARY KEY(todo_id,event_id));
      CREATE INDEX IF NOT EXISTS dev_inbox_project ON dev_inbox_todos(linked_project_id);") .map_err(err)
}
#[derive(Deserialize)]
struct IncomingTodo { id: String, title: String, body: String, quote: String }
#[derive(Deserialize)]
pub(crate) struct Event { version: u32, event_id: String, session_id: String, source_key: String,
    source_name: String, source_root: String, captured_at: String, text: String, todos: Vec<IncomingTodo> }
pub(crate) fn import(c: &mut Connection, event: &Event) -> Result<(), String> {
    if event.version != 1 || event.event_id.is_empty() || event.source_key.is_empty() {
        return Err("收集记录格式无效".into());
    }
    let tx = c.transaction().map_err(err)?;
    let seen: bool = tx.query_row("SELECT EXISTS(SELECT 1 FROM dev_inbox_events WHERE id=?)", [&event.event_id], |r|r.get(0)).map_err(err)?;
    if seen { return Ok(()); }
    for item in &event.todos {
        if item.id.is_empty() || item.title.trim().is_empty() || item.title.chars().count()>500 || item.body.len()>100000 || item.quote.is_empty() || !event.text.contains(&item.quote) {
            return Err("待办缺少有效标题或来源依据".into());
        }
        tx.execute("INSERT OR IGNORE INTO dev_inbox_todos(id,source_key,source_name,source_root,title,body,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)",
          params![item.id,event.source_key,event.source_name,event.source_root,item.title.trim(),item.body,event.captured_at,event.captured_at]).map_err(err)?;
        // Existing user edits, status, links and deletion tombstones always win.
        tx.execute("INSERT OR IGNORE INTO dev_inbox_sources(todo_id,event_id,session_id,quote,captured_at) VALUES(?,?,?,?,?)",
          params![item.id,event.event_id,event.session_id,item.quote,event.captured_at]).map_err(err)?;
    }
    tx.execute("INSERT INTO dev_inbox_events VALUES(?,?)",params![event.event_id,chrono::Utc::now().to_rfc3339()]).map_err(err)?;
    tx.commit().map_err(err)
}
#[derive(Serialize)]
pub struct Todo { id:String, source_key:String, source_name:String, source_root:String, title:String,
    body:String, status:String, linked_project_id:Option<String>, created_at:String, updated_at:String, sources:Vec<Value> }
fn list(c:&Connection, project:Option<&str>)->Result<Vec<Todo>,String>{
    let mut stmt=c.prepare("SELECT id,source_key,source_name,source_root,title,body,status,linked_project_id,created_at,updated_at FROM dev_inbox_todos WHERE deleted=0 AND (?1 IS NULL OR linked_project_id=?1) ORDER BY created_at DESC,id").map_err(err)?;
    let mut rows=stmt.query_map([project],|r|Ok(Todo{id:r.get(0)?,source_key:r.get(1)?,source_name:r.get(2)?,source_root:r.get(3)?,title:r.get(4)?,body:r.get(5)?,status:r.get(6)?,linked_project_id:r.get(7)?,created_at:r.get(8)?,updated_at:r.get(9)?,sources:vec![]})).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
    for item in &mut rows {
        let mut q=c.prepare("SELECT session_id,quote,captured_at FROM dev_inbox_sources WHERE todo_id=? ORDER BY captured_at DESC").map_err(err)?;
        item.sources=q.query_map([&item.id],|r|Ok(json!({"session_id":r.get::<_,String>(0)?,"quote":r.get::<_,String>(1)?,"captured_at":r.get::<_,String>(2)?}))).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err)?;
    }
    Ok(rows)
}
fn read_status(path:&Path)->Value{fs::read(path).ok().and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or(Value::Null)}
fn count_json(path:&Path)->usize{fs::read_dir(path).map(|d|d.filter_map(Result::ok).filter(|e|e.path().extension().is_some_and(|x|x=="json")).count()).unwrap_or(0)}
#[tauri::command]
pub fn dev_inbox_read(app:AppHandle, project_id:Option<String>)->Result<Value,String>{
    let _guard=LOCK.lock().map_err(err)?;
    let dir=super::config_dir(&app)?;
    let root=dir.join("codex-inbox");
    let mut c=Connection::open(dir.join("workbench.db")).map_err(err)?;
    c.busy_timeout(Duration::from_secs(5)).map_err(err)?;
    schema(&c)?;
    let mut imported=vec![];let mut errors=vec![];
    if let Ok(files)=fs::read_dir(root.join("ready")){
        for file in files.filter_map(Result::ok){
            let path=file.path();
            if path.extension().is_none_or(|x|x!="json"){continue;}
            let result=fs::read(&path).map_err(err).and_then(|b|serde_json::from_slice::<Event>(&b).map_err(err)).and_then(|e|import(&mut c,&e));
            match result {Ok(())=>imported.push(path),Err(e)=>errors.push(e)}
        }
    }
    if !imported.is_empty(){
        super::backup_database(app.clone(),false,None)?;
        fs::create_dir_all(root.join("processed")).map_err(err)?;
        for path in imported {let target=root.join("processed").join(path.file_name().ok_or("事件文件名无效")?);fs::rename(path,target).map_err(err)?;}
    }
    Ok(json!({"items":list(&c,project_id.as_deref())?,"pending":count_json(&root.join("pending")),
      "capture":read_status(&root.join("capture-status.json")),"worker":read_status(&root.join("worker-status.json")),"errors":errors}))
}
#[derive(Deserialize)]
pub struct Edit { pub id:String, pub title:String, pub body:String, pub status:String, pub linked_project_id:Option<String> }
fn save(c:&Connection,item:&Edit)->Result<(),String>{
    if item.title.trim().is_empty() || item.title.chars().count()>500{return Err("请填写 1–500 字的标题".into());}
    if !["todo","doing","done","dismissed"].contains(&item.status.as_str()){return Err("待办状态无效".into());}
    let n=c.execute("UPDATE dev_inbox_todos SET title=?,body=?,status=?,linked_project_id=?,updated_at=? WHERE id=? AND deleted=0",
      params![item.title.trim(),item.body,item.status,item.linked_project_id,chrono::Utc::now().to_rfc3339(),item.id]).map_err(err)?;
    if n==0{return Err("该待办已删除或不存在".into());}Ok(())
}
#[tauri::command]
pub fn dev_inbox_save(app:AppHandle,item:Edit)->Result<(),String>{
    let _guard=LOCK.lock().map_err(err)?;
    let c=Connection::open(super::config_dir(&app)?.join("workbench.db")).map_err(err)?;
    c.busy_timeout(Duration::from_secs(5)).map_err(err)?;schema(&c)?;save(&c,&item)?;
    super::backup_database(app,false,None)?;Ok(())
}
#[tauri::command]
pub fn dev_inbox_delete(app:AppHandle,id:String)->Result<(),String>{
    let _guard=LOCK.lock().map_err(err)?;
    let c=Connection::open(super::config_dir(&app)?.join("workbench.db")).map_err(err)?;
    c.busy_timeout(Duration::from_secs(5)).map_err(err)?;schema(&c)?;
    c.execute("UPDATE dev_inbox_todos SET deleted=1,updated_at=? WHERE id=?",params![chrono::Utc::now().to_rfc3339(),id]).map_err(err)?;
    super::backup_database(app,false,None)?;Ok(())
}
#[tauri::command]
pub fn dev_inbox_open_chat(session_id:String)->Result<(),String>{
    if session_id.is_empty() || !session_id.chars().all(|c|c.is_ascii_alphanumeric()||c=='-'||c=='_'){return Err("来源聊天标识无效".into());}
    std::process::Command::new("/usr/bin/open").arg(format!("codex://threads/{session_id}")).status().map_err(err).and_then(|s|if s.success(){Ok(())}else{Err("无法打开 Codex，请复制聊天标识查找".into())})
}
#[cfg(test)]
mod tests{
 use super::*;
 fn db()->Connection{let c=Connection::open_in_memory().unwrap();c.execute_batch("CREATE TABLE long_projects(id TEXT PRIMARY KEY);INSERT INTO long_projects VALUES('p');CREATE TABLE tasks(id TEXT);INSERT INTO tasks VALUES('untouched');").unwrap();schema(&c).unwrap();c}
 fn event(id:&str)->Event{Event{version:1,event_id:id.into(),session_id:"chat-1".into(),source_key:"repo".into(),source_name:"来源".into(),source_root:"/repo".into(),captured_at:"2026-10-07".into(),text:"下次增加搜索".into(),todos:vec![IncomingTodo{id:"todo1".into(),title:"增加搜索".into(),body:"说明".into(),quote:"增加搜索".into()}]}}
 #[test]fn imports_are_idempotent_and_preserve_edits(){let mut c=db();import(&mut c,&event("e1")).unwrap();import(&mut c,&event("e1")).unwrap();save(&c,&Edit{id:"todo1".into(),title:"我编辑的标题".into(),body:"手写说明".into(),status:"done".into(),linked_project_id:Some("p".into())}).unwrap();import(&mut c,&event("e2")).unwrap();let rows=list(&c,None).unwrap();assert_eq!(rows.len(),1);assert_eq!(rows[0].title,"我编辑的标题");assert_eq!(rows[0].status,"done");assert_eq!(rows[0].sources.len(),2);assert_eq!(list(&c,Some("p")).unwrap().len(),1);assert_eq!(c.query_row("SELECT id FROM tasks",[],|r|r.get::<_,String>(0)).unwrap(),"untouched");}
 #[test]fn delete_does_not_resurrect_and_project_delete_unlinks(){let mut c=db();import(&mut c,&event("e1")).unwrap();save(&c,&Edit{id:"todo1".into(),title:"标题".into(),body:"".into(),status:"todo".into(),linked_project_id:Some("p".into())}).unwrap();c.execute("DELETE FROM long_projects",[]).unwrap();assert!(list(&c,None).unwrap()[0].linked_project_id.is_none());c.execute("UPDATE dev_inbox_todos SET deleted=1",[]).unwrap();import(&mut c,&event("e2")).unwrap();assert!(list(&c,None).unwrap().is_empty());}
 #[test]fn invalid_event_rolls_back_and_invalid_link_rejected(){let mut c=db();let mut e=event("e1");e.todos[0].quote="没有这句话".into();assert!(import(&mut c,&e).is_err());assert!(list(&c,None).unwrap().is_empty());import(&mut c,&event("e2")).unwrap();assert!(save(&c,&Edit{id:"todo1".into(),title:"标题".into(),body:"".into(),status:"todo".into(),linked_project_id:Some("missing".into())}).is_err());}
}
