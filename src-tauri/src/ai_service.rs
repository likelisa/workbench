use chrono::{Datelike, Duration, NaiveDate};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};

pub fn err(e: impl std::fmt::Display) -> String { e.to_string() }
pub fn hash(v: &Value) -> String { format!("{:x}", Sha256::digest(v.to_string().as_bytes())) }
pub fn text<'a>(v: &'a Value, k: &str) -> &'a str { v[k].as_str().unwrap_or("") }
fn required<'a>(v: &'a Value, k: &str) -> Result<&'a str, String> {
    let s=text(v,k).trim(); if s.is_empty() {Err(format!("缺少字段：{k}"))} else {Ok(s)}
}
fn date(s: &str) -> Result<NaiveDate,String> {NaiveDate::parse_from_str(s,"%Y-%m-%d").map_err(err)}
fn table(kind: &str) -> Result<&'static str,String> {match kind {
    "task"=>Ok("tasks"), "assignment"=>Ok("week_assignments"), "dayTask"=>Ok("day_tasks"),
    _=>Err("任务类型应为 task / assignment / dayTask".into())}}
fn status(s:&str)->Result<(),String>{if ["todo","doing","done","missed"].contains(&s){Ok(())}else{Err("状态无效".into())}}
pub fn schema(c:&Connection)->Result<(),String>{
    c.execute_batch("CREATE TABLE IF NOT EXISTS ai_operations(request_id TEXT PRIMARY KEY,request_json TEXT NOT NULL,result_json TEXT NOT NULL,created_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS ai_audit(id INTEGER PRIMARY KEY AUTOINCREMENT,request_id TEXT NOT NULL,actor TEXT NOT NULL,action TEXT NOT NULL,record_id TEXT, before_json TEXT,after_json TEXT,created_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS ai_snapshots(id TEXT PRIMARY KEY,scope_json TEXT NOT NULL,data_json TEXT NOT NULL,created_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS ai_reviews(id TEXT PRIMARY KEY,snapshot_id TEXT NOT NULL REFERENCES ai_snapshots(id),title TEXT NOT NULL,body TEXT NOT NULL,evidence_json TEXT NOT NULL,created_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS ai_receipts(id TEXT PRIMARY KEY,payload_json TEXT NOT NULL,result_json TEXT NOT NULL,created_at TEXT NOT NULL);").map_err(err)
}
pub fn rows(c:&Connection,sql:&str,args:&[&dyn rusqlite::ToSql])->Result<Vec<Value>,String>{
    let mut s=c.prepare(sql).map_err(err)?; let names=s.column_names().iter().map(|s|s.to_string()).collect::<Vec<_>>();
    let result=s.query_map(args,|r| {let mut m=Map::new();for (i,n) in names.iter().enumerate(){let v=match r.get_ref(i)?{
        rusqlite::types::ValueRef::Null=>Value::Null,rusqlite::types::ValueRef::Integer(v)=>json!(v),
        rusqlite::types::ValueRef::Real(v)=>json!(v),rusqlite::types::ValueRef::Text(v)=>json!(String::from_utf8_lossy(v)),_=>Value::Null};m.insert(n.clone(),v);}Ok(Value::Object(m))
    }).map_err(err)?.collect::<Result<Vec<_>,_>>().map_err(err);result
}
pub fn record(c:&Connection,kind:&str,id:&str)->Result<Value,String>{
    let mut v=rows(c,&format!("SELECT * FROM {} WHERE id=?",table(kind)?),&[&id])?.pop().ok_or("记录不存在")?;
    if kind=="task" {v["project_ids"]=json!(rows(c,"SELECT project_id FROM task_projects WHERE task_id=? ORDER BY project_id",&[&id])?);}
    if kind=="assignment" {v["weeks"]=json!(rows(c,"SELECT week_no FROM assignment_weeks WHERE assignment_id=? ORDER BY week_no",&[&id])?);}
    let version=hash(&v);v["version"]=json!(version);Ok(v)
}
pub fn query(c:&Connection,a:&Value)->Result<Value,String>{
    let kind=required(a,"kind")?;
    let t=match kind {"project"=>"long_projects","cycle"=>"cycles","annual"=>"annual_goals","direction"=>"directions","time"=>"time_entries","plan"=>"plan_blocks","category"=>"categories","review"=>"ai_reviews","snapshot"=>"ai_snapshots","audit"=>"ai_audit",_=>table(kind)?};
    let limit=a["limit"].as_u64().unwrap_or(50).clamp(1,100);let offset=a["offset"].as_u64().unwrap_or(0).min(1_000_000);
    let mut sql=format!("SELECT r.* FROM {t} r WHERE 1=1");let mut values:Vec<String>=vec![];
    for (key,col) in [("id","id"),("status","status"),("cycle_id","cycle_id")] {
        if let Some(v)=a[key].as_str().filter(|v|!v.is_empty()) {
            if key=="cycle_id"&&kind=="dayTask" {sql.push_str(" AND r.task_id IN (SELECT id FROM tasks WHERE cycle_id=?)");}
            else if (key=="cycle_id"&&!matches!(kind,"task"|"assignment"))||(key=="status"&&matches!(kind,"time"|"plan"|"review"|"snapshot"|"audit"|"cycle"|"direction"|"category")) {return Err(format!("{kind} 不支持 {key} 筛选"));}
            else {sql.push_str(&format!(" AND r.{col}=?"));}values.push(v.into());
        }
    }
    if let Some(p)=a["project_id"].as_str().filter(|v|!v.is_empty()) {
        sql.push_str(match kind {"task"=>" AND r.id IN (SELECT task_id FROM task_projects WHERE project_id=?)","assignment"|"dayTask"|"time"=>" AND r.long_project_id=?","project"=>" AND r.id=?",_=>return Err("此类型不支持项目筛选".into())});values.push(p.into());
    }
    for (key,op) in [("date_from",">="),("date_to","<=")] {if let Some(d)=a[key].as_str().filter(|v|!v.is_empty()){
        date(d)?;let col=match kind{"dayTask"|"plan"=>"date","time"=>"substr(start_at,1,10)","task"=>"due_date",_=>return Err("此类型不支持日期筛选".into())};
        if kind=="time" {let boundary=format!("{d}T00:00:00+08:00");let end=if op=="<="{format!("{}T00:00:00+08:00",date(d)?+Duration::days(1))}else{boundary};sql.push_str(if op==">="{" AND julianday(r.end_at)>julianday(?)"}else{" AND julianday(r.start_at)<julianday(?)"});values.push(end);}
        else {sql.push_str(&format!(" AND {col}{op}?"));values.push(d.into());}
    }}
    let ordering=if matches!(kind,"review"|"snapshot"|"audit"){"r.created_at DESC,r.id DESC"}else{"r.id"};
    sql.push_str(&format!(" ORDER BY {ordering} LIMIT {} OFFSET {offset}",limit+1));
    let refs=values.iter().map(|s|s as &dyn rusqlite::ToSql).collect::<Vec<_>>();let mut items=rows(c,&sql,&refs)?;let more=items.len()>limit as usize;items.truncate(limit as usize);
    if matches!(kind,"task"|"assignment"|"dayTask"){items=items.iter().map(|v|record(c,kind,text(v,"id"))).collect::<Result<_,_>>()?;}
    Ok(json!({"items":items,"next_offset":if more{Some(offset+limit)}else{None},"timezone":"Asia/Shanghai"}))
}

pub fn create_day(tx:&Transaction,id:&str,v:&Value)->Result<(),String>{
    let d=required(v,"date")?;let day=date(d)?;let title=required(v,"title")?;status(text(v,"status"))?;
    let priority=v["priority"].as_str().filter(|s|!s.is_empty());if priority.is_some_and(|p|!["P0","P1","P2","P3"].contains(&p)){return Err("优先级无效".into());}
    let assignment=v["assignment_id"].as_str().filter(|s|!s.is_empty());let mut task:Option<String>=None;let mut project:Option<String>=None;
    if let Some(a)=assignment {
        let (t,p,start):(String,Option<String>,String)=tx.query_row("SELECT w.task_id,t.project_id,c.start_date FROM week_assignments w JOIN tasks t ON t.id=w.task_id JOIN cycles c ON c.id=w.cycle_id WHERE w.id=?",[a],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(err)?;
        let delta=(day-date(&start)?).num_days();let week=delta.div_euclid(7)+1;
        let ok:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM assignment_weeks WHERE assignment_id=? AND week_no=?)",params![a,week],|r|r.get(0)).map_err(err)?;
        if delta<0||delta>=84||!ok{return Err("该周任务未安排在日任务日期所在周".into());}task=Some(t);project=p;
    }
    let start=v["scheduled_start_at"].as_str();let end=v["scheduled_end_at"].as_str();let category=v["scheduled_category_id"].as_str();
    if start.is_some()||end.is_some()||category.is_some(){
        let (s,e,k)=(start.ok_or("缺少开始时间")?,end.ok_or("缺少结束时间")?,category.ok_or("缺少时间分类")?);
        let s=chrono::DateTime::parse_from_rfc3339(s).map_err(err)?;let e=chrono::DateTime::parse_from_rfc3339(e).map_err(err)?;
        let tz=chrono::FixedOffset::east_opt(8*3600).unwrap();if e<=s||s.with_timezone(&tz).date_naive()!=day{return Err("时间范围与日任务日期不一致".into());}
        let valid:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM categories WHERE id=? AND active=1 AND kind IN ('fixed','goal'))",[k],|r|r.get(0)).map_err(err)?;
        if !valid{return Err("请选择有效时间分类".into());}
        let overlap:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM time_entries WHERE julianday(start_at)<julianday(?) AND julianday(end_at)>julianday(?))",params![e.to_rfc3339(),s.to_rfc3339()],|r|r.get(0)).map_err(err)?;
        if overlap{return Err("时间与已有实际记录重叠".into());}
    }
    let normalize=|s:Option<&str>|s.map(|s|chrono::DateTime::parse_from_rfc3339(s).map(|d|d.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Millis,true)).map_err(err)).transpose();
    tx.execute("INSERT INTO day_tasks(id,date,title,status,assignment_id,task_id,project_id,priority,scheduled_start_at,scheduled_end_at,scheduled_category_id,created_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",params![id,d,title,text(v,"status"),assignment,task,project,priority,normalize(start)?,normalize(end)?,category,chrono::Utc::now().to_rfc3339()]).map_err(err)?;Ok(())
}

fn audit(tx:&Transaction,request:&str,actor:&str,action:&str,id:&str,before:&Value,after:&Value)->Result<(),String>{
    tx.execute("INSERT INTO ai_audit(request_id,actor,action,record_id,before_json,after_json,created_at) VALUES(?,?,?,?,?,?,?)",params![request,actor,action,id,before.to_string(),after.to_string(),chrono::Utc::now().to_rfc3339()]).map_err(err)?;Ok(())
}
pub fn write(c:&mut Connection,method:&str,a:&Value,actor:&str)->Result<Value,String>{
    let request=required(a,"request_id")?;if request.len()>200{return Err("请求编号过长".into());}
    let input=json!({"method":method,"args":a,"actor":actor}).to_string();
    let tx=c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
    let previous:Option<(String,String)>=tx.query_row("SELECT request_json,result_json FROM ai_operations WHERE request_id=?",[request],|r|Ok((r.get(0)?,r.get(1)?))).optional().map_err(err)?;
    if let Some((old,result))=previous {if old!=input{return Err("请求编号已用于不同内容".into());}return serde_json::from_str(&result).map_err(err);}
    let id=if actor=="workbench-ui"{request.to_string()}else{format!("ai-{}",&hash(&json!(request))[..32])};
    let result=match method {
        "create_task"=>{
            let kind=required(a,"kind")?;let v=a["values"].as_object().ok_or("缺少任务内容")?.clone();
            if kind=="dayTask" {
                let allowed=if actor=="workbench-ui"{"id,title,date,status,assignment_id,priority,scheduled_start_at,scheduled_end_at,scheduled_category_id,project_id,task_id,recurrence_id,created_at"}else{"title,date,status,assignment_id,priority,scheduled_start_at,scheduled_end_at,scheduled_category_id"};
                if v.keys().any(|k|!allowed.split(',').any(|a|a==k)){return Err("日任务包含不支持字段".into());}
            }

            if kind=="dayTask"{let mut day=a["values"].clone();if day["status"].is_null(){day["status"]=json!("todo");}create_day(&tx,&id,&day)?;}
            else {
                table(kind)?;
                if let Some(d)=v.get("due_date").and_then(Value::as_str).filter(|s|!s.is_empty()){date(d)?;}
                let ids=a["project_ids"].as_array().map(|v|v.iter().map(|s|s.as_str().map(str::to_owned).ok_or("项目编号无效".to_string())).collect::<Result<Vec<_>,_>>()).transpose()?.unwrap_or_default();
                let weeks=a["weeks"].as_array().map(|v|v.iter().map(|s|s.as_i64().filter(|w|(1..=12).contains(w)).ok_or("周次须为1—12".to_string())).collect::<Result<Vec<_>,_>>()).transpose()?.unwrap_or_default();
                if kind=="task"&&!weeks.is_empty(){return Err("本轮待办不能直接安排周次".into());}
                super::long_projects::apply_in(&tx,kind,"save",&id,v,ids,None,weeks)?;
            }
            let after=record(&tx,kind,&id)?;audit(&tx,request,actor,method,&id,&Value::Null,&after)?;json!({"status":"saved","record":after})
        },
        "set_task_status"=>{
            let kind=required(a,"kind")?;let id=required(a,"id")?;let before=record(&tx,kind,id)?;
            if text(&before,"version")!=required(a,"expected_version")?{return Err("记录已变化，请重新查询后再操作".into());}
            let state=required(a,"status")?;status(state)?;
            tx.execute(&format!("UPDATE {} SET status=? WHERE id=?",table(kind)?),params![state,id]).map_err(err)?;
            let after=record(&tx,kind,id)?;audit(&tx,request,actor,method,id,&before,&after)?;json!({"status":"saved","record":after})
        },
        "save_review"=>{
            let sid=required(a,"snapshot_id")?;let title=required(a,"title")?;let body=required(a,"body")?;
            let evidence=a["evidence"].as_array().ok_or("请提供数据依据")?;if evidence.is_empty(){return Err("数据依据不能为空".into());}
            let snap:String=tx.query_row("SELECT data_json FROM ai_snapshots WHERE id=?",[sid],|r|r.get(0)).map_err(err)?;
            let snap:Value=serde_json::from_str(&snap).map_err(err)?;
            for e in evidence {let p=e.as_str().ok_or("依据应为快照的 JSON Pointer")?;if !p.starts_with('/')||snap.pointer(p).is_none(){return Err(format!("快照中不存在依据 {p}"));}}
            tx.execute("INSERT INTO ai_reviews VALUES(?,?,?,?,?,?)",params![id,sid,title,body,a["evidence"].to_string(),chrono::Utc::now().to_rfc3339()]).map_err(err)?;
            let after=rows(&tx,"SELECT * FROM ai_reviews WHERE id=?",&[&id])?.pop().ok_or("报告保存失败")?;audit(&tx,request,actor,method,&id,&Value::Null,&after)?;json!({"status":"saved","review":after})
        },_=>return Err("不支持此写操作".into())
    };
    let mut result=result;result["operation_id"]=json!(request);
    tx.execute("INSERT INTO ai_operations VALUES(?,?,?,?)",params![request,input,result.to_string(),chrono::Utc::now().to_rfc3339()]).map_err(err)?;
    tx.commit().map_err(err)?;Ok(result)
}

pub fn snapshot(c:&mut Connection,a:&Value)->Result<Value,String>{
    let tx=c.transaction().map_err(err)?;
    let (start,end,cycle)=if text(a,"scope")=="cycle" {
        let id=required(a,"cycle_id")?;let s:String=tx.query_row("SELECT start_date FROM cycles WHERE id=?",[id],|r|r.get(0)).map_err(err)?;let start=date(&s)?;(start,start+Duration::days(83),Some(id))
    }else if text(a,"scope")=="year"{let y=a["year"].as_i64().filter(|y|(1..=9998).contains(y)).ok_or("年份无效")? as i32;(NaiveDate::from_ymd_opt(y,1,1).unwrap(),NaiveDate::from_ymd_opt(y,12,31).unwrap(),None)}else{return Err("复盘范围应为 year 或 cycle".into());};
    let s=start.to_string();let e=end.to_string();
    let tasks=if let Some(id)=cycle{rows(&tx,"SELECT * FROM tasks WHERE cycle_id=? ORDER BY id",&[&id])?}else{rows(&tx,"SELECT t.* FROM tasks t LEFT JOIN annual_goals a ON a.id=t.annual_goal_id WHERE a.year=? ORDER BY t.id",&[&start.year()])?};
    let days=rows(&tx,"SELECT * FROM day_tasks WHERE date>=? AND date<=? ORDER BY date,id",&[&s,&e])?;
    let weeks=if let Some(id)=cycle{rows(&tx,"SELECT * FROM week_assignments WHERE cycle_id=? ORDER BY id",&[&id])?}else{rows(&tx,"SELECT w.* FROM week_assignments w JOIN tasks t ON t.id=w.task_id JOIN annual_goals a ON a.id=t.annual_goal_id WHERE a.year=? ORDER BY w.id",&[&start.year()])?};
    let from=format!("{s}T00:00:00+08:00");let until=format!("{}T00:00:00+08:00",end+Duration::days(1));
    let entries=rows(&tx,"SELECT *,max(0,(min(julianday(end_at),julianday(?2))-max(julianday(start_at),julianday(?1)))*1440) AS minutes_in_range FROM time_entries WHERE julianday(end_at)>julianday(?1) AND julianday(start_at)<julianday(?2) ORDER BY start_at,id",&[&from,&until])?;
    let plans=rows(&tx,"SELECT * FROM plan_blocks WHERE date>=? AND date<=? ORDER BY date,id",&[&s,&e])?;
    let summary=|v:&Vec<Value>|{let done=v.iter().filter(|t|text(t,"status")=="done").count();json!({"total":v.len(),"done":done,"completion_rate":if v.is_empty(){Value::Null}else{json!(done as f64/v.len() as f64)}})};
    let minutes: f64=entries.iter().filter_map(|v|v["minutes_in_range"].as_f64()).sum();
    let planned:i64=plans.iter().map(|v|v["end_min"].as_i64().unwrap_or(0)-v["start_min"].as_i64().unwrap_or(0)).sum();
    let notes=rows(&tx,"SELECT id,title,review FROM cycles WHERE start_date<=? AND date(start_date,'+83 days')>=? ORDER BY start_date",&[&e,&s])?;
    let updates=rows(&tx,"SELECT * FROM project_updates WHERE date>=? AND date<=? ORDER BY date,id",&[&s,&e])?;
    let data=json!({"scope":a,"projects":rows(&tx,"SELECT * FROM long_projects ORDER BY id",&[])?,"annual_goals":rows(&tx,"SELECT * FROM annual_goals ORDER BY id",&[])?,"categories":rows(&tx,"SELECT * FROM categories ORDER BY id",&[])?,"date_from":s,"date_to":e,"timezone":"Asia/Shanghai","as_of":chrono::Utc::now().to_rfc3339(),"metrics":{"tasks":summary(&tasks),"weeks":summary(&weeks),"days":summary(&days),"recorded_minutes":minutes,"planned_block_minutes":planned},"tasks":tasks,"weeks":weeks,"days":days,"time_entries":entries,"plan_blocks":plans,"cycle_reviews":notes,"project_updates":updates,"limitations":["完成率是生成快照时的当前状态，不是历史时点状态。","年度任务按年度目标年份归属；周期任务按周期归属。日任务和时间记录按日期范围统计。","已记录用时不等于经核实的实际投入；创建带时间的日任务可能由现有规则生成时间记录。","计划时段与已记录用时分别统计，不推测未记录时间。"]});
    let id=hash(&data);tx.execute("INSERT OR IGNORE INTO ai_snapshots VALUES(?,?,?,?)",params![id,a.to_string(),data.to_string(),chrono::Utc::now().to_rfc3339()]).map_err(err)?;tx.commit().map_err(err)?;
    Ok(json!({"snapshot_id":id,"data":data,"evidence_format":"用 JSON Pointer 引用 data 内字段，例如 /metrics/tasks 或 /days/0"}))
}

#[cfg(test)]
mod tests {
 use super::*;
 fn db()->Connection {
  let c=Connection::open_in_memory().unwrap();
  c.execute_batch("PRAGMA foreign_keys=ON;
    CREATE TABLE cycles(id TEXT PRIMARY KEY,title TEXT,start_date TEXT,review TEXT);
    CREATE TABLE annual_goals(id TEXT PRIMARY KEY,title TEXT,year INTEGER);
    CREATE TABLE tasks(id TEXT PRIMARY KEY,title TEXT,cycle_id TEXT REFERENCES cycles(id),annual_goal_id TEXT REFERENCES annual_goals(id),project_id TEXT,due_date TEXT,status TEXT,created_at TEXT);
    CREATE TABLE week_assignments(id TEXT PRIMARY KEY,task_id TEXT REFERENCES tasks(id),cycle_id TEXT REFERENCES cycles(id),commitment TEXT,status TEXT,review TEXT,priority TEXT,long_project_id TEXT,created_at TEXT);
    CREATE TABLE assignment_weeks(assignment_id TEXT REFERENCES week_assignments(id),week_no INTEGER,PRIMARY KEY(assignment_id,week_no));
    CREATE TABLE day_tasks(id TEXT PRIMARY KEY,date TEXT,title TEXT,status TEXT,assignment_id TEXT,task_id TEXT,project_id TEXT,priority TEXT,scheduled_start_at TEXT,scheduled_end_at TEXT,scheduled_category_id TEXT,created_at TEXT,long_project_id TEXT);
    CREATE TABLE time_entries(id TEXT PRIMARY KEY,start_at TEXT,end_at TEXT,day_task_id TEXT,category_id TEXT,task_id TEXT,project_id TEXT,long_project_id TEXT);
    CREATE TABLE categories(id TEXT PRIMARY KEY,active INTEGER,kind TEXT);
    CREATE TABLE plan_blocks(id TEXT PRIMARY KEY,date TEXT,start_min INTEGER,end_min INTEGER);
    INSERT INTO cycles VALUES('c','测试周期','2026-10-01','周期笔记');
    INSERT INTO annual_goals VALUES('a','测试目标',2026);
    INSERT INTO categories VALUES('k',1,'fixed');").unwrap();
  c.execute_batch(include_str!("long_projects.sql")).unwrap();schema(&c).unwrap();c
 }
 fn task(c:&mut Connection)->Value {write(c,"create_task",&json!({"request_id":"t","kind":"task","values":{"title":"任务","cycle_id":"c","annual_goal_id":"a"}}),"test").unwrap()}
 #[test]fn idempotency_and_conflict_do_not_duplicate_or_overwrite(){
  let mut c=db();let first=task(&mut c);assert_eq!(task(&mut c),first);
  let args=json!({"request_id":"state","kind":"task","id":first["record"]["id"],"expected_version":first["record"]["version"],"status":"done"});
  let done=write(&mut c,"set_task_status",&args,"test").unwrap();assert_eq!(done["record"]["status"],"done");assert_eq!(write(&mut c,"set_task_status",&args,"test").unwrap(),done);
  let mut stale=args.clone();stale["request_id"]=json!("stale");stale["status"]=json!("doing");assert!(write(&mut c,"set_task_status",&stale,"test").unwrap_err().contains("变化"));
  let mut reuse=args;reuse["status"]=json!("missed");assert!(write(&mut c,"set_task_status",&reuse,"test").is_err());
  assert_eq!(rows(&c,"SELECT * FROM ai_audit",&[]).unwrap().len(),2);
 }
 #[test]fn invalid_relations_roll_back_and_days_require_scheduled_week(){
  let mut c=db();let t=task(&mut c);let id=t["record"]["id"].clone();
  assert!(write(&mut c,"create_task",&json!({"request_id":"bad","kind":"task","values":{"title":"bad","cycle_id":"missing","annual_goal_id":"a"}}),"test").is_err());
  let w=write(&mut c,"create_task",&json!({"request_id":"w","kind":"assignment","values":{"task_id":id,"commitment":"周任务"},"weeks":[2]}),"test").unwrap();
  let mut d=json!({"request_id":"d","kind":"dayTask","values":{"title":"日任务","date":"2026-10-01","assignment_id":w["record"]["id"]}});
  assert!(write(&mut c,"create_task",&d,"test").is_err());d["values"]["date"]=json!("2026-10-08");
  let day=write(&mut c,"create_task",&d,"test").unwrap();assert_eq!(day["record"]["task_id"],id);
  let page=query(&c,&json!({"kind":"dayTask","cycle_id":"c","date_from":"2026-10-08","date_to":"2026-10-08","limit":1})).unwrap();assert_eq!(page["items"].as_array().unwrap().len(),1);
 }
 #[test]fn snapshots_clip_times_and_reports_are_versioned(){
  let mut c=db();task(&mut c);
  c.execute("INSERT INTO time_entries(id,start_at,end_at) VALUES('e','2025-12-31T15:00:00Z','2025-12-31T17:00:00Z')",[]).unwrap();
  let s=snapshot(&mut c,&json!({"scope":"year","year":2026})).unwrap();let m=s["data"]["metrics"]["recorded_minutes"].as_f64().unwrap();assert!((m-60.0).abs()<0.001);
  assert!(s["data"]["metrics"]["days"]["completion_rate"].is_null());
  let mut a=json!({"request_id":"review1","snapshot_id":s["snapshot_id"],"title":"年度复盘","body":"数据事实与推断分开","evidence":["/metrics/tasks"]});
  let first=write(&mut c,"save_review",&a,"test").unwrap();assert_eq!(write(&mut c,"save_review",&a,"test").unwrap(),first);
  a["request_id"]=json!("review2");write(&mut c,"save_review",&a,"test").unwrap();
  assert_eq!(rows(&c,"SELECT * FROM ai_reviews",&[]).unwrap().len(),2);
  a["request_id"]=json!("review3");a["evidence"]=json!(["/missing"]);assert!(write(&mut c,"save_review",&a,"test").is_err());
  assert_eq!(snapshot(&mut c,&json!({"scope":"cycle","cycle_id":"c"})).unwrap()["data"]["date_to"],"2026-12-23");
 }
 #[test]fn timed_day_rejects_overlap_and_unknown_category(){
  let mut c=db();c.execute("INSERT INTO time_entries(id,start_at,end_at) VALUES('e','2026-10-08T01:00:00Z','2026-10-08T02:00:00Z')",[]).unwrap();
  let mut a=json!({"request_id":"timed","kind":"dayTask","values":{"title":"安排","date":"2026-10-08","scheduled_start_at":"2026-10-08T09:30:00+08:00","scheduled_end_at":"2026-10-08T10:30:00+08:00","scheduled_category_id":"k"}});
  assert!(write(&mut c,"create_task",&a,"test").unwrap_err().contains("重叠"));
  a["values"]["scheduled_category_id"]=json!("missing");assert!(write(&mut c,"create_task",&a,"test").unwrap_err().contains("分类"));
 }
}
