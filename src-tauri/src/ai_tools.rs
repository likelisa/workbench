use super::ai_service::{self as service,err,hash,text};
use rusqlite::{params,Connection,OptionalExtension};
use sha2::{Digest,Sha256};
use serde_json::{json,Value};
use std::{fs,io::{BufRead,BufReader,Write},os::unix::{fs::PermissionsExt,net::{UnixListener,UnixStream}},path::Path,sync::{Mutex,OnceLock},time::{Duration,Instant}};
use tauri::{AppHandle,Emitter};
static LOCK:Mutex<()>=Mutex::new(());
static STARTED:OnceLock<()>=OnceLock::new();
fn open(dir:&Path)->Result<Connection,String>{let c=Connection::open(dir.join("workbench.db")).map_err(err)?;c.busy_timeout(Duration::from_secs(5)).map_err(err)?;c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON").map_err(err)?;service::schema(&c)?;super::dev_inbox::schema(&c)?;Ok(c)}
fn atomic(path:&Path,v:&Value)->Result<(),String>{let temp=path.with_extension("tmp");fs::write(&temp,v.to_string()).map_err(err)?;fs::rename(temp,path).map_err(err)}

fn import_one(c:&mut Connection,v:&Value)->Result<Value,String>{
    let request=text(v,"request_id");if request.is_empty()||request.len()>200{return Err("提交编号无效".into());}
    let key=hash(&json!(request));let canonical=v.to_string();
    let old:Option<(String,String)>=c.query_row("SELECT payload_json,result_json FROM ai_receipts WHERE id=?",[&key],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
    if let Some((payload,result))=old{if payload!=canonical{return Err("提交编号已用于不同内容".into());}return serde_json::from_str(&result).map_err(err);}
    let items=v["todos"].as_array().filter(|x|!x.is_empty()&&x.len()<=100).ok_or("待办数量必须为1—100")?;
    let root=text(v,"source_root");if !Path::new(root).is_absolute(){return Err("来源项目必须是绝对路径".into());}
    let source_key=format!("{:x}",Sha256::digest(root.as_bytes()));let mut todos=vec![];let mut raw=String::new();let mut ids=vec![];
    for (i,t) in items.iter().enumerate(){
        let title=text(t,"title").trim();let body=text(t,"body");let quote=text(t,"quote");let priority=text(t,"priority");
        if title.is_empty()||title.chars().count()>500||body.len()>60000||quote.is_empty()||quote.len()>60000{return Err("待办需有效标题、说明和来源原文".into());}
        if !priority.is_empty()&&!["P0","P1","P2","P3"].contains(&priority){return Err("优先级无效".into());}
        raw.push_str(quote);raw.push('\n');let id=format!("submit-{key}-{i}");ids.push(id.clone());
        todos.push(json!({"id":id,"title":title,"body":if priority.is_empty(){body.to_string()}else{format!("{priority} · {body}")},"quote":quote}));
    }
    let event=json!({"version":1,"event_id":format!("submit-{key}"),"session_id":text(v,"session_id"),"source_key":source_key,"source_name":Path::new(root).file_name().unwrap_or_default().to_string_lossy(),"source_root":root,"captured_at":text(v,"created_at"),"text":raw,"todos":todos});
    let event=serde_json::from_value(event).map_err(err)?;super::dev_inbox::import(c,&event)?;
    let result=json!({"status":"imported","receipt_id":key,"request_id":request,"count":ids.len(),"record_ids":ids,"imported_at":chrono::Utc::now().to_rfc3339()});
    c.execute("INSERT INTO ai_receipts VALUES(?,?,?,?)",params![key,canonical,result.to_string(),chrono::Utc::now().to_rfc3339()]).map_err(err)?;Ok(result)
}
fn import_queue(app:&AppHandle,dir:&Path)->Result<(),String>{
    let folder=dir.join("ai-tools/submissions");let mut c=open(dir)?;let mut changed=false;
    if let Ok(files)=fs::read_dir(&folder){for entry in files.filter_map(Result::ok){
        let p=entry.path();if p.extension().is_none_or(|x|x!="json"){continue;}
        let key=p.file_stem().unwrap_or_default().to_string_lossy().to_string();
        let result=fs::read(&p).map_err(err).and_then(|s|serde_json::from_slice::<Value>(&s).map_err(err)).and_then(|v|import_one(&mut c,&v));
        match result {
            Ok(mut receipt)=>{
                // Keep the queued submission if backup fails. Import replay is idempotent.
                match super::backup_database(app.clone(),false,None) {
                    Ok(_)=>{receipt["backup"]=json!("saved");atomic(&dir.join(format!("ai-tools/receipts/{key}.json")),&receipt)?;fs::rename(&p,dir.join(format!("ai-tools/processed/{key}.json"))).map_err(err)?;changed=true;},
                    Err(e)=>{receipt["backup_error"]=json!(e);atomic(&dir.join(format!("ai-tools/receipts/{key}.json")),&receipt)?;}
                }
            },
            Err(e)=>{atomic(&dir.join(format!("ai-tools/receipts/{key}.json")),&json!({"status":"failed","receipt_id":key,"error":e}))?;}
        }
    }}
    if changed{let _=app.emit("workbench-changed",());}Ok(())
}
fn dispatch(app:&AppHandle,dir:&Path,method:&str,args:&Value,actor:&str)->Result<Value,String>{
    let mut c=open(dir)?;
    let result=match method {
        "query"=>service::query(&c,args)?,
        "connection_status"=>{
            let count=|p:&Path|fs::read_dir(p).map(|d|d.filter_map(Result::ok).filter(|e|e.path().extension().is_some_and(|s|s=="json")).count()).unwrap_or(0);
            let failed=fs::read_dir(dir.join("ai-tools/receipts")).into_iter().flatten().filter_map(Result::ok).filter(|e|fs::read(e.path()).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).is_some_and(|v|v["status"]=="failed")).count();
            let status:Value=fs::read(dir.join("ai-tools/import-status.json")).ok().and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or(Value::Null);
            json!({"queued":count(&dir.join("ai-tools/submissions")),"failed":failed,"legacy_pending":count(&dir.join("codex-inbox/pending")),"error":status["error"]})
        },
        "create_task"|"set_task_status"|"save_review"=>service::write(&mut c,method,args,actor)?,
        "review_snapshot"=>service::snapshot(&mut c,args)?,
        "operation_receipt"=>{let s:String=c.query_row("SELECT result_json FROM ai_operations WHERE request_id=?",[text(args,"request_id")],|r|r.get(0)).map_err(err)?;serde_json::from_str(&s).map_err(err)?},
        "sync_submissions"=>{drop(c);import_queue(app,dir)?;return Ok(json!({"status":"checked"}));},
        _=>return Err("不支持的工具".into())
    };
    let mut result=result;
    if matches!(method,"create_task"|"set_task_status"|"save_review"|"review_snapshot"){
        if let Err(e)=super::backup_database(app.clone(),false,None){result["backup_error"]=json!(e);}else{result["backup"]=json!("saved");}
        let _=app.emit("workbench-changed",());
    }Ok(result)
}
fn handle(app:&AppHandle,dir:&Path,stream:&mut UnixStream)->Result<(),String>{
    stream.set_read_timeout(Some(Duration::from_secs(3))).map_err(err)?;stream.set_write_timeout(Some(Duration::from_secs(5))).map_err(err)?;
    let mut input=String::new();use std::io::Read;
    BufReader::new(&mut *stream).take(1_000_001).read_line(&mut input).map_err(err)?;
    if input.len()>1_000_000{return Err("请求过大".into());}
    let req:Value=serde_json::from_str(&input).map_err(err)?;
    let result=(||{
        let creds:Value=serde_json::from_slice(&fs::read(dir.join("ai-tools/capabilities.json")).map_err(err)?).map_err(err)?;
        let token=text(&req,"token");if token.is_empty(){return Err("未授权".into());}
        let method=text(&req,"method");
        if token==text(&creds,"submit") {if method!="sync_submissions"{return Err("提交入口无管理权限".into());}}
        else if token!=text(&creds,"manage"){return Err("未授权".into());}
        let _guard=LOCK.lock().map_err(err)?;dispatch(app,dir,method,&req["args"],"codex")
    })();
    let response=match result{Ok(v)=>json!({"result":v}),Err(e)=>json!({"error":e})};
    writeln!(stream,"{response}").map_err(err)
}
#[tauri::command]
pub fn ai_initialize(app:AppHandle)->Result<(),String>{
    let dir=super::config_dir(&app)?;let _guard=LOCK.lock().map_err(err)?;open(&dir)?;
    for name in ["submissions","receipts","processed"]{fs::create_dir_all(dir.join("ai-tools").join(name)).map_err(err)?;}
    if STARTED.get().is_some(){return Ok(());}
    let socket=dir.join("ai.sock");
    if socket.exists(){if UnixStream::connect(&socket).is_ok(){return Err("已有工作台实例提供 AI 接口，请只保留一个实例".into());}fs::remove_file(&socket).map_err(err)?;}
    let listener=UnixListener::bind(&socket).map_err(err)?;fs::set_permissions(&socket,fs::Permissions::from_mode(0o600)).map_err(err)?;
    listener.set_nonblocking(true).map_err(err)?;let _=STARTED.set(());
    std::thread::spawn(move||{let mut last=Instant::now()-Duration::from_secs(10);loop{
        match listener.accept(){Ok((mut s,_))=>{let _=handle(&app,&dir,&mut s);},Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{},Err(_)=>break}
        if last.elapsed()>=Duration::from_secs(2){if let Ok(_g)=LOCK.lock(){if let Err(e)=import_queue(&app,&dir){let _=atomic(&dir.join("ai-tools/import-status.json"),&json!({"error":e}));}else{let _=atomic(&dir.join("ai-tools/import-status.json"),&json!({"last_check":chrono::Utc::now().to_rfc3339()}));}}last=Instant::now();}
        std::thread::sleep(Duration::from_millis(100));
    }});Ok(())
}
#[tauri::command]
pub fn ai_call(app:AppHandle,method:String,args:Value)->Result<Value,String>{
    let _g=LOCK.lock().map_err(err)?;dispatch(&app,&super::config_dir(&app)?,&method,&args,"workbench-ui")
}

#[cfg(test)]
mod tests {
 use super::*;
 #[test]
 fn submission_preserves_legacy_source_identity_and_replays_once(){
  let mut c=Connection::open_in_memory().unwrap();
  c.execute_batch("CREATE TABLE long_projects(id TEXT PRIMARY KEY)").unwrap();
  service::schema(&c).unwrap();super::super::dev_inbox::schema(&c).unwrap();
  let v=json!({"request_id":"test-import","source_root":"/tmp/project","session_id":"","created_at":"2026-10-08T00:00:00Z","todos":[{"title":"后续测试","body":"说明","quote":"原文","priority":"P1"}]});
  let first=import_one(&mut c,&v).unwrap();assert_eq!(first,import_one(&mut c,&v).unwrap());
  let (count,key):(i64,String)=c.query_row("SELECT count(*),source_key FROM dev_inbox_todos",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
  assert_eq!(count,1);assert_eq!(key,format!("{:x}",Sha256::digest(b"/tmp/project")));
 }
}
