use super::ai_service::{self as service,err,text};
use rusqlite::Connection;
use serde_json::{json,Value};
use std::{fs,io::{BufRead,BufReader,Write},os::unix::{fs::PermissionsExt,net::{UnixListener,UnixStream}},path::Path,sync::{Mutex,OnceLock},time::Duration};
use tauri::{AppHandle,Emitter};
static LOCK:Mutex<()>=Mutex::new(());
static STARTED:OnceLock<()>=OnceLock::new();
fn open(dir:&Path)->Result<Connection,String>{let c=Connection::open(dir.join("workbench.db")).map_err(err)?;c.busy_timeout(Duration::from_secs(5)).map_err(err)?;c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON").map_err(err)?;service::schema(&c)?;super::ai_workbench::schema(&c)?;Ok(c)}
fn dispatch(app:&AppHandle,dir:&Path,method:&str,args:&Value,actor:&str,context:&Value)->Result<Value,String>{
    let mut c=open(dir)?;
    let result=match method {
        "query"=>super::ai_workbench::query(&c,args)?,
        "create_task" if actor!="workbench-ui"=>return Err("新增待办必须先create_draft并等待用户确认".into()),
        "create_task"|"save_review"=>service::write(&mut c,method,args,actor)?,
        "set_task_status"=>super::ai_workbench::execute(&mut c,&json!({"request_id":args["request_id"],"intent":"explicit","operation":{"kind":args["kind"],"action":"update","id":args["id"],"expected_version":args["expected_version"],"values":{"status":args["status"]}}}),actor)?,
        "statistics"=>super::ai_workbench::statistics(&c,args)?,
        "execute_operation"=>super::ai_workbench::execute(&mut c,args,actor)?,
        "create_draft"|"revise_draft"|"confirm_draft"|"cancel_draft"=>{
            let confirmation=if actor.starts_with("weixin:"){context["text"].as_str()}else{args["confirmation_text"].as_str()};
            super::ai_workbench::draft(&mut c,method,args,actor,text(context,"messageId"),confirmation)?
        },
        "query_drafts"=>super::ai_workbench::query_drafts(&c,args,actor)?,
        "review_snapshot"=>service::snapshot(&mut c,args)?,
        "operation_receipt"=>{let s:String=c.query_row("SELECT result_json FROM ai_operations WHERE request_id=?",[text(args,"request_id")],|r|r.get(0)).map_err(err)?;serde_json::from_str(&s).map_err(err)?},
        _=>return Err("不支持的工具".into())
    };
    let mut result=result;
    if matches!(method,"create_task"|"set_task_status"|"save_review"|"review_snapshot"|"execute_operation"|"create_draft"|"revise_draft"|"cancel_draft"|"confirm_draft"){
        if let Err(e)=super::backup_database(app.clone(),false,None){result["backup_error"]=json!(e);}else{result["backup"]=json!("saved");}
        if method=="review_snapshot"||result["status"]=="saved" {let _=app.emit("workbench-changed",());}
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
        let _guard=LOCK.lock().map_err(err)?;
        if token==text(&creds,"manage") {dispatch(app,dir,method,&req["args"],"codex",&Value::Null)}
        else if !text(&creds,"weixin").is_empty() && token==text(&creds,"weixin") {
            // Context is written by the authenticated bridge, never supplied by the model.
            let context:Value=serde_json::from_slice(&fs::read(dir.join("weixin/context.json")).map_err(err)?).map_err(err)?;
            let sender=text(&context,"senderId");let account=text(&context,"accountId");
            if sender.is_empty()||account.is_empty(){return Err("微信消息上下文缺失".into());}
            let cfg:Value=serde_json::from_slice(&fs::read(dir.join("weixin/bridge-config.json")).map_err(err)?).map_err(err)?;
            let allowed=cfg["allowedSenderIds"].as_array().is_some_and(|a|a.len()==1&&a.iter().any(|v|v.as_str()==Some(sender)));
            if !allowed || text(&cfg,"authorizedAccountId")!=account {return Err("微信用户未授权，第一版仅允许一个账号".into());}
            let age=chrono::Utc::now().timestamp_millis()-context["receivedAt"].as_i64().ok_or("缺少消息时间")?;
            if !(0..=1_800_000).contains(&age){return Err("微信消息上下文已过期，请重发".into());}
            dispatch(app,dir,method,&req["args"],&format!("weixin:{account}:{sender}"),&context)
        } else {Err("未授权".into())}
    })();
    let response=match result{Ok(v)=>json!({"result":v}),Err(e)=>json!({"error":e})};
    writeln!(stream,"{response}").map_err(err)
}
#[tauri::command]
pub fn ai_initialize(app:AppHandle)->Result<(),String>{
    let dir=super::config_dir(&app)?;let _guard=LOCK.lock().map_err(err)?;open(&dir)?;
    if STARTED.get().is_some(){return Ok(());}
    let socket=dir.join("ai.sock");
    if socket.exists(){if UnixStream::connect(&socket).is_ok(){return Err("已有工作台实例提供 AI 接口，请只保留一个实例".into());}fs::remove_file(&socket).map_err(err)?;}
    let listener=UnixListener::bind(&socket).map_err(err)?;fs::set_permissions(&socket,fs::Permissions::from_mode(0o600)).map_err(err)?;
    listener.set_nonblocking(true).map_err(err)?;let _=STARTED.set(());
    std::thread::spawn(move||{loop{
        match listener.accept(){Ok((mut s,_))=>{let _=handle(&app,&dir,&mut s);},Err(e) if e.kind()==std::io::ErrorKind::WouldBlock=>{},Err(_)=>break}
        std::thread::sleep(Duration::from_millis(100));
    }});Ok(())
}
#[tauri::command]
pub fn ai_call(app:AppHandle,method:String,args:Value)->Result<Value,String>{
    let _g=LOCK.lock().map_err(err)?;dispatch(&app,&super::config_dir(&app)?,&method,&args,"workbench-ui",&Value::Null)
}
