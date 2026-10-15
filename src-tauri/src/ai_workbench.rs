//! Typed workbench operations and durable, owner-bound confirmation drafts.
use super::ai_service::{err, hash, rows, text};
use chrono::{Duration, NaiveDate};
use rusqlite::{params, params_from_iter, types::Value as SqlValue, Connection, OptionalExtension, Transaction};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;

const ENTITIES: &[(&str, &str, &str)] = &[
    ("item", "items", "title,type,status,priority,description,project_id,parent_id,annual_goal_id,cycle_goal_id,cycle_id,level,version_id,milestone_id"),
    ("placement", "placements", "item_id,kind,date,week_start,cycle_id,week_no"),
    ("cycleGoal", "cycle_goals", "title,cycle_id,annual_goal_id"),
    ("projectVersion", "project_versions", "project_id,title,description"),
    ("direction", "directions", "title"),
    ("annual", "annual_goals", "title,direction_id,year,status"),
    ("cycle", "cycles", "title,start_date,review"),
    ("task", "tasks", "title,cycle_id,annual_goal_id,due_date,status"),
    ("assignment", "week_assignments", "task_id,commitment,status,review,priority,long_project_id"),
    ("dayTask", "day_tasks", "title,date,status,assignment_id,priority,scheduled_start_at,scheduled_end_at,scheduled_category_id"),
    ("project", "long_projects", "title,type_id,description,focus,links,status,favorite"),
    ("projectType", "project_types", "title,sort_order"),
    ("stage", "project_stages", "project_id,title,description,start_date,end_date,status"),
    ("projectPlan", "project_plans", "project_id,stage_id,title,description,horizon,completed"),
    ("projectUpdate", "project_updates", "project_id,date,content"),
    ("plan", "plan_blocks", "date,start_min,end_min,title,day_task_id,item_id,schedule_kind,category_id,work_kind,life_category,category_manual,long_project_id,primary_goal_id,direction_id"),
    ("dailyPlan", "daily_plan_templates", "start_min,end_min,title"),
    ("time", "time_entries", "start_at,end_at,title,category_id,day_task_id,item_id,work_kind,life_category,primary_goal_id,category_manual,long_project_id,source_plan_id,direction_id"),
    ("recurrence", "recurrences", "kind,frequency,weekdays,start_date,end_date,title,start_min,end_min,task_id,category_id,active"),
    ("category", "categories", "color"),
];
const LINKS: &[&str] = &["task_project_items","item_projects","task_projects", "assignment_weeks", "plan_tasks", "plan_weeks", "recurrence_exceptions"];
fn entity(kind: &str) -> Result<(&'static str, &'static str), String> {
    ENTITIES.iter().find(|e| e.0 == kind).map(|e| (e.1,e.2)).ok_or_else(|| format!("不支持对象：{kind}"))
}
fn required<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    let s = text(v,key).trim(); if s.is_empty() { Err(format!("缺少字段：{key}")) } else { Ok(s) }
}
fn date(s: &str) -> Result<NaiveDate,String> { NaiveDate::parse_from_str(s,"%Y-%m-%d").map_err(err) }
fn now() -> String { chrono::Utc::now().to_rfc3339() }
fn optional<'a>(v: &'a Value, k: &str) -> Option<&'a str> { v[k].as_str().filter(|s| !s.is_empty()) }
fn ids(v: &Value) -> Result<Vec<String>,String> {
    if v.is_null() { return Ok(vec![]); }
    v.as_array().ok_or("应为编号数组")?.iter().map(|s| s.as_str().map(str::to_owned).ok_or("编号无效".into())).collect()
}
fn weeks(v: &Value) -> Result<Vec<i64>,String> {
    if v.is_null() { return Ok(vec![]); }
    v.as_array().ok_or("应为周次数组")?.iter().map(|w| w.as_i64().filter(|n| (1..=12).contains(n)).ok_or("周次须为1—12".into())).collect()
}
fn choice(v: &Value,k: &str, allowed: &[&str]) -> Result<(),String> {
    if let Some(s) = optional(v,k) { if !allowed.contains(&s) { return Err(format!("{k} 无效")); } } Ok(())
}
fn exists(c: &Connection,kind: &str,id: &str) -> Result<(),String> {
    let found:bool=c.query_row(&format!("SELECT EXISTS(SELECT 1 FROM {} WHERE id=?)",entity(kind)?.0),[id],|r|r.get(0)).map_err(err)?;
    if found { Ok(()) } else { Err(format!("{kind} 记录不存在：{id}")) }
}
pub fn schema(c: &Connection) -> Result<(),String> {
    super::unified::schema(c)?;
    c.execute_batch("CREATE TABLE IF NOT EXISTS ai_drafts(id TEXT PRIMARY KEY,owner TEXT NOT NULL,revision INTEGER NOT NULL,state TEXT NOT NULL,payload_json TEXT NOT NULL,context_hash TEXT NOT NULL,preview_json TEXT NOT NULL,source_message_id TEXT NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL);").map_err(err)
}
pub fn record(c: &Connection,kind: &str,id: &str) -> Result<Value,String> {
    let mut v=rows(c,&format!("SELECT * FROM {} WHERE id=?",entity(kind)?.0),&[&id])?.pop().ok_or("记录不存在")?;
    if kind=="item" {
        v["project_item_ids"]=json!(rows(c,"SELECT project_item_id FROM task_project_items WHERE task_id=? ORDER BY project_item_id",&[&id])?.iter().map(|r|text(r,"project_item_id")).collect::<Vec<_>>());
        v["referencing_task_ids"]=json!(rows(c,"SELECT task_id FROM task_project_items WHERE project_item_id=? ORDER BY task_id",&[&id])?.iter().map(|r|text(r,"task_id")).collect::<Vec<_>>());
    }
    let related = match kind {
        "task" => vec![("project_ids","SELECT project_id FROM task_projects WHERE task_id=? ORDER BY project_id"),("plan_ids","SELECT plan_id FROM plan_tasks WHERE task_id=? ORDER BY plan_id")],
        "assignment" => vec![("weeks","SELECT week_no FROM assignment_weeks WHERE assignment_id=? ORDER BY week_no"),("plan_ids","SELECT plan_id FROM plan_weeks WHERE assignment_id=? ORDER BY plan_id")],
        _ => vec![],
    };
    for (k,sql) in related { v[k]=json!(rows(c,sql,&[&id])?); }
    v["version"]=json!(hash(&v)); Ok(v)
}
pub fn query(c: &Connection,a: &Value) -> Result<Value,String> {
    let kind=required(a,"kind")?;
    if matches!(kind,"review"|"audit"|"snapshot") { return super::ai_service::query(c,a); }
    let (table,allowed)=entity(kind)?;
    let mut sql=format!("SELECT * FROM {table} WHERE 1=1"); let mut values=vec![];
    for key in ["id","status","cycle_id","annual_goal_id","task_id","assignment_id","direction_id","type_id","stage_id","item_id","parent_id","cycle_goal_id"] {
        if let Some(s)=optional(a,key) {
            if key!="id" && !allowed.split(',').any(|x| x==key) && !(key=="cycle_id" && kind=="assignment") { return Err(format!("{kind} 不支持 {key} 筛选")); }
            sql.push_str(&format!(" AND {key}=?")); values.push(s.to_string());
        }
    }
    if let Some(p)=optional(a,"project_item_id") {if kind!="item" {return Err("project_item_id只适用于事项".into());}sql.push_str(" AND id IN(SELECT task_id FROM task_project_items WHERE project_item_id=?)");values.push(p.into());}
    if let Some(p)=optional(a,"project_id") {
        sql.push_str(match kind {
            "item" => " AND id IN(SELECT id FROM items WHERE project_id=? UNION SELECT item_id FROM item_projects WHERE project_id=?)",
            "projectVersion" => " AND project_id=?",
            "task" => " AND id IN (SELECT task_id FROM task_projects WHERE project_id=?)",
            "assignment"|"dayTask"|"time" => " AND long_project_id=?",
            "stage"|"projectPlan"|"projectUpdate" => " AND project_id=?",
            "project" => " AND id=?", _=>return Err("此对象不支持项目筛选".into()),
        }); values.push(p.to_string());if kind=="item" {values.push(p.to_string());}
    }
    if let Some(s)=optional(a,"search") {
        let col=if kind=="assignment" {"commitment"} else if kind=="projectUpdate" {"content"} else if kind=="category" {"name"} else {"title"};
        if kind=="placement" {sql.push_str(" AND item_id IN(SELECT id FROM items WHERE instr(lower(title),lower(?))>0)");}else{sql.push_str(&format!(" AND instr(lower({col}),lower(?))>0"));} values.push(s.to_string());
    }
    if let Some(w)=a["week_no"].as_i64() {if kind=="placement" {sql.push_str(" AND week_no=?");values.push(w.to_string());}else {if kind!="assignment" || !(1..=12).contains(&w) {return Err("week_no只适用于周任务，范围1—12".into());}sql.push_str(" AND id IN(SELECT assignment_id FROM assignment_weeks WHERE week_no=?)");values.push(w.to_string());}}
    if let Some(y)=a["year"].as_i64() {if kind!="annual" {return Err("year只适用于年度目标".into());}sql.push_str(" AND year=?");values.push(y.to_string());}
    for (key,op) in [("date_from",">="),("date_to","<=")] {
        if let Some(d)=optional(a,key) {
            date(d)?;
            if kind=="time" {
                sql.push_str(if key=="date_from" {" AND julianday(end_at)>julianday(?)"} else {" AND julianday(start_at)<julianday(?)"});
                values.push(format!("{}T00:00:00+08:00",if key=="date_to" {date(d)?+Duration::days(1)} else {date(d)?}));
            } else {
                let col=match kind {"placement"|"dayTask"|"plan"|"projectUpdate"=>"date","task"=>"due_date","cycle"|"recurrence"|"stage"=>"start_date",_=>return Err("此对象不支持日期筛选".into())};
                sql.push_str(&format!(" AND {col}{op}?")); values.push(d.into());
            }
        }
    }
    let limit=a["limit"].as_u64().unwrap_or(50).clamp(1,100); let offset=a["offset"].as_u64().unwrap_or(0).min(1_000_000);
    sql.push_str(&format!(" ORDER BY id LIMIT {} OFFSET {offset}",limit+1));
    let refs=values.iter().map(|s|s as &dyn rusqlite::ToSql).collect::<Vec<_>>();let mut items=rows(c,&sql,&refs)?;
    let more=items.len()>limit as usize;items.truncate(limit as usize);
    let items=items.iter().map(|v|record(c,kind,text(v,"id"))).collect::<Result<Vec<_>,_>>()?;
    Ok(json!({"items":items,"next_offset":if more {Some(offset+limit)} else {None},"timezone":"Asia/Shanghai"}))
}

// A conservative business-data guard also catches newly added candidates/children.
// Internal drafts, receipts, audits and snapshots never invalidate one another.
fn state(c: &Connection) -> Result<Value,String> {
    let mut out=Map::new();
    for (_,t,_) in ENTITIES { out.insert(t.to_string(),json!(rows(c,&format!("SELECT * FROM {t} ORDER BY id"),&[])?)); }
    for t in LINKS { out.insert(t.to_string(),json!(rows(c,&format!("SELECT * FROM {t} ORDER BY 1,2"),&[])?)); }
    Ok(Value::Object(out))
}
fn changes(before: &Value,after: &Value) -> Vec<Value> {
    let mut result=vec![];
    for (table,b) in before.as_object().unwrap() {
        let key=|v: &Value| if !v["id"].is_null() {v["id"].to_string()} else {v.to_string()};
        let old=b.as_array().unwrap().iter().map(|v|(key(v),v)).collect::<BTreeMap<_,_>>();
        let new=after[table].as_array().unwrap().iter().map(|v|(key(v),v)).collect::<BTreeMap<_,_>>();
        for (id,v) in &old { if new.get(id)!=Some(v) { result.push(json!({"table":table,"id":id,"before":v,"after":new.get(id)})); } }
        for (id,v) in &new { if !old.contains_key(id) { result.push(json!({"table":table,"id":id,"before":null,"after":v})); } }
    } result
}
fn sql_save(tx: &Transaction,table: &str,id: &str,v: &Map<String,Value>,creating: bool) -> Result<(),String> {
    let mut names=vec![];let mut values=vec![];
    for (k,v) in v {
        names.push(k.clone());values.push(match v {Value::Null=>SqlValue::Null,Value::String(s)=>SqlValue::Text(s.clone()),Value::Number(n)=>SqlValue::Integer(n.as_i64().ok_or("数值须为整数")?),_=>return Err("字段须为字符串、整数或null".into())});
    }
    if creating { names.push("id".into());values.push(SqlValue::Text(id.into()));tx.execute(&format!("INSERT INTO {table} ({}) VALUES ({})",names.join(","),vec!["?";names.len()].join(",")),params_from_iter(values)).map_err(err)?; }
    else {values.push(SqlValue::Text(id.into()));tx.execute(&format!("UPDATE {table} SET {} WHERE id=?",names.iter().map(|n|format!("{n}=?")).collect::<Vec<_>>().join(",")),params_from_iter(values)).map_err(err)?;}
    Ok(())
}
fn validate(c: &Connection,kind: &str,v: &mut Value,id: &str,check_schedule:bool,defer_overlap:bool) -> Result<(),String> {
    super::unified::validate_project_boundary(c,kind,v,id)?;
    if kind=="item" {super::unified::normalize_week_item(c,v,id)?;super::unified::normalize_day_item(c,v,id,chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(28800).unwrap()).date_naive())?;}
    let title=if kind=="assignment" {"commitment"} else if kind=="projectUpdate" {"content"} else {"title"};
    if kind!="category" && kind!="time" && kind!="placement" && kind!="plan" { required(v,title)?; }
    choice(v,"priority",&["P0","P1","P2","P3"])?;
    match kind {
        "item"|"task"|"assignment"|"dayTask"=>choice(v,"status",&["todo","doing","done","missed"]),
        "annual"=>choice(v,"status",&["todo","done","missed"]),
        "project"=>choice(v,"status",&["preparing","active","paused","archived"]),
        "stage"=>choice(v,"status",&["planned","active","done"]),_=>Ok(()),
    }?;
    choice(v,"horizon",&["now","next","later"])?;
    for k in ["date","due_date","start_date","end_date"] { if let Some(d)=optional(v,k) { date(d)?; } }
    if optional(v,"start_date").is_some() && optional(v,"end_date").is_some() && text(v,"end_date")<text(v,"start_date") { return Err("结束日期不能早于开始日期".into()); }
    for (field,parent) in [("direction_id","direction"),("annual_goal_id","annual"),("cycle_id","cycle"),("task_id","task"),("assignment_id","assignment"),("type_id","projectType"),("project_id","project"),("stage_id","stage"),("day_task_id","dayTask"),("item_id","item"),("parent_id","item"),("cycle_goal_id","cycleGoal"),("primary_goal_id","annual"),("long_project_id","project")] {
        if field=="project_id" && !matches!(kind,"stage"|"projectPlan"|"projectUpdate"|"item"|"projectVersion") {continue;}
        if let Some(p)=optional(v,field) { exists(c,parent,p)?; }
    }
    if kind=="item" && !super::unified::is_project_item(c,v)? {if let Some(g)=optional(v,"cycle_goal_id") {let goal=record(c,"cycleGoal",g)?;for key in ["cycle_id","annual_goal_id"]{if v[key].is_null(){v[key]=goal[key].clone();}else if v[key]!=goal[key]{return Err("本轮目标与周期或年度目标不一致".into());}}}}
    if matches!(kind,"item"|"placement") {super::unified::validate(c,kind,v,id)?;}
    choice(v,"type",&["requirement","bug","todo","idea"])?;
    choice(v,"level",&["backlog","cycle","week","day"])?;
    choice(v,"schedule_kind",&["fixed","flexible"])?;
    choice(v,"work_kind",&["work","life","legacy"])?;
    if kind=="annual" { if !v["year"].as_i64().is_some_and(|y|(1..=9998).contains(&y)) {return Err("年份无效".into());} }
    if matches!(kind,"plan"|"projectUpdate") {required(v,"date")?;}
    if kind=="cycle" {required(v,"start_date")?;}
    if kind=="assignment" { required(v,"task_id")?; }
    if kind=="dayTask" {
        let d=date(required(v,"date")?)?;
        if check_schedule {if let Some(a)=optional(v,"assignment_id") {
            let (task,project,start):(String,Option<String>,String)=c.query_row("SELECT w.task_id,t.project_id,c.start_date FROM week_assignments w JOIN tasks t ON t.id=w.task_id JOIN cycles c ON c.id=w.cycle_id WHERE w.id=?",[a],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(err)?;
            let delta=(d-date(&start)?).num_days(); let w=delta.div_euclid(7)+1;
            let placed:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM assignment_weeks WHERE assignment_id=? AND week_no=?)",params![a,w],|r|r.get(0)).map_err(err)?;
            if !(0..84).contains(&delta) || !placed {return Err("该周任务未安排在日任务日期所在周".into());}
            v["task_id"]=json!(task);v["project_id"]=json!(project);
        } else {v["task_id"]=Value::Null;v["project_id"]=Value::Null;}}
        let has=["scheduled_start_at","scheduled_end_at","scheduled_category_id"].iter().any(|k|optional(v,k).is_some());
        if has {
            if check_schedule {valid_time(c,required(v,"scheduled_start_at")?,required(v,"scheduled_end_at")?,required(v,"scheduled_category_id")?,&format!("{id}:time"),false)?;}
            let s=chrono::DateTime::parse_from_rfc3339(text(v,"scheduled_start_at")).map_err(err)?;
            if s.with_timezone(&chrono::FixedOffset::east_opt(28800).unwrap()).date_naive()!=d {return Err("开始时间与日任务日期不一致".into());}
            for k in ["scheduled_start_at","scheduled_end_at"] { v[k]=json!(normalize(text(v,k))?); }
        } else {for k in ["scheduled_start_at","scheduled_end_at","scheduled_category_id"] {v[k]=Value::Null;}}
    }
    if matches!(kind,"plan"|"dailyPlan") || (kind=="recurrence" && text(v,"kind")=="plan") {
        let (s,e)=(v["start_min"].as_i64().ok_or("缺少开始分钟")?,v["end_min"].as_i64().ok_or("缺少结束分钟")?);
        if s<0 || e>1440 || e<=s {return Err("计划时段无效".into());}
    }
    if kind=="time" && optional(v,"day_task_id").is_some() && optional(v,"item_id").is_none() && record(c,"time",id).is_err() {
        let mapped=format!("day:{}",text(v,"day_task_id"));if exists(c,"item",&mapped).is_ok(){v["item_id"]=json!(mapped);}
    }
    if matches!(kind,"time"|"plan") && optional(v,"long_project_id").is_some() && record(c,kind,id).is_err() && v["category_manual"].as_i64()!=Some(1) {v["work_kind"]=json!("work");}
    if matches!(kind,"time"|"plan") && optional(v,"item_id").is_some() {
        let item=record(c,"item",text(v,"item_id"))?;
        let old=record(c,kind,id).ok();
        if old.is_none() {
            if v["category_manual"].as_i64()!=Some(1) {v["work_kind"]=json!("work");}
            if v["long_project_id"].is_null() {v["long_project_id"]=item["project_id"].clone();}
            if v["primary_goal_id"].is_null() {v["primary_goal_id"]=item["annual_goal_id"].clone();}
        }
    }
    if matches!(kind,"time"|"plan") && record(c,kind,id).is_err() {
        if let Some(item)=optional(v,"item_id").map(str::to_string) {
            for field in ["project_id","annual_goal_id"] {
                let target=if field=="project_id" {"long_project_id"}else{"primary_goal_id"};
                if v[target].is_null() {let candidates=rows(c,&format!("WITH RECURSIVE family AS (SELECT id,parent_id,{field} FROM items WHERE id=? UNION SELECT i.id,i.parent_id,i.{field} FROM items i JOIN family f ON i.id=f.parent_id) SELECT DISTINCT {field} candidate FROM family WHERE {field} IS NOT NULL"),&[&item])?;if candidates.len()==1{v[target]=candidates[0]["candidate"].clone();}}
            }
        }
        if v["primary_goal_id"].is_null() {if let Some(project)=optional(v,"long_project_id") {let candidates=rows(c,"SELECT DISTINCT annual_goal_id candidate FROM items WHERE annual_goal_id IS NOT NULL AND (project_id=?1 OR id IN(SELECT item_id FROM item_projects WHERE project_id=?1))",&[&project])?;if candidates.len()==1{v["primary_goal_id"]=candidates[0]["candidate"].clone();}}}
    }
    if matches!(kind,"time"|"plan") {
        if record(c,kind,id).is_err() && v["direction_id"].is_null() {if let Some(g)=optional(v,"primary_goal_id"){v["direction_id"]=record(c,"annual",g)?["direction_id"].clone();}}
        if text(v,"work_kind")=="work" {v["category_id"]=json!("system:work");v["life_category"]=Value::Null;}
        if text(v,"work_kind")=="life" {
            let cat=required(v,"category_id")?;
            let name:Option<String>=c.query_row("SELECT name FROM categories WHERE id=? AND kind='fixed' AND active=1 AND name<>'工作'",[cat],|r|r.get(0)).optional().map_err(err)?;
            v["life_category"]=json!(name.ok_or("请选择生活类别")?);
        }
        if kind=="plan" {if let Some(cat)=optional(v,"category_id") {exists(c,"category",cat)?;}}
        if text(v,"title").trim().is_empty() {v["title"]=json!(if text(v,"work_kind")=="work" {"工作"}else{optional(v,"life_category").unwrap_or("时间记录")});}
    }
    if kind=="time" {
        if let Some(source)=optional(v,"source_plan_id") {
            if record(c,"time",id).ok().as_ref().and_then(|old|optional(old,"source_plan_id"))!=Some(source){exists(c,"plan",source)?;}
            let duplicate:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM time_entries WHERE source_plan_id=? AND id<>?)",params![source,id],|r|r.get(0)).map_err(err)?;
            if duplicate && !defer_overlap{return Err("这个计划时段已经回填实际时间".into());}
        }
    }
    if kind=="time" { valid_time(c,required(v,"start_at")?,required(v,"end_at")?,required(v,"category_id")?,id,defer_overlap)?;for k in ["start_at","end_at"] {v[k]=json!(normalize(text(v,k))?);} }
    if matches!(kind,"time"|"plan") {
        if let Some(d)=optional(v,"day_task_id") {
            let day=record(c,"dayTask",d)?;
            if kind=="time" && v["item_id"].is_null() && record(c,"time",id).is_err() {v["task_id"]=day["task_id"].clone();v["project_id"]=day["project_id"].clone();}
            else if kind=="plan" && text(v,"date")!=text(&day,"date") {return Err("计划时段和日任务日期不一致".into());}
        } else if kind=="time" && record(c,kind,id).is_err() {v["task_id"]=Value::Null;v["project_id"]=Value::Null;}
    }
    if kind=="recurrence" {
        choice(v,"kind",&["day_task","plan"])?;required(v,"kind")?;choice(v,"frequency",&["daily","weekly"])?;required(v,"frequency")?;required(v,"start_date")?;
        if text(v,"frequency")=="weekly" {let list=required(v,"weekdays")?.split(',').collect::<Vec<_>>();if list.iter().any(|s|s.parse::<u8>().map_or(true,|n|n>6)) {return Err("星期应为0—6，用逗号分隔".into());}}
    }
    if kind=="category" {let color=required(v,"color")?;if color.len()!=7 || !color.starts_with('#') || !color[1..].bytes().all(|b|b.is_ascii_hexdigit()) {return Err("颜色应为#RRGGBB".into());}}
    for k in ["completed","favorite","active","category_manual"] {if !v[k].is_null() && !v[k].as_i64().is_some_and(|x|x==0||x==1) {return Err(format!("{k} 应为0或1"));}}
    Ok(())
}
fn normalize(s: &str) -> Result<String,String> {Ok(chrono::DateTime::parse_from_rfc3339(s).map_err(err)?.with_timezone(&chrono::Utc).to_rfc3339_opts(chrono::SecondsFormat::Millis,true))}
fn valid_time(c: &Connection,s: &str,e: &str,cat: &str,except: &str,defer_overlap:bool) -> Result<(),String> {
    let s=chrono::DateTime::parse_from_rfc3339(s).map_err(err)?;let e=chrono::DateTime::parse_from_rfc3339(e).map_err(err)?;
    if e<=s {return Err("结束时间必须晚于开始时间".into());}
    let valid:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM categories WHERE id=?1 AND ((active=1 AND kind IN ('fixed','goal')) OR EXISTS(SELECT 1 FROM time_entries WHERE id=?2 AND category_id=?1)))",params![cat,except],|r|r.get(0)).map_err(err)?;
    if !valid {return Err("请选择有效时间分类".into());}
    if defer_overlap {return Ok(());}
    let overlap:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM time_entries WHERE id!=? AND julianday(start_at)<julianday(?) AND julianday(end_at)>julianday(?))",params![except,e.to_rfc3339(),s.to_rfc3339()],|r|r.get(0)).map_err(err)?;
    if overlap {return Err("时间与已有记录重叠".into());}Ok(())
}

fn delete(tx: &Transaction,kind: &str,id: &str) -> Result<(),String> {
    let statements:Vec<&str>=match kind {
        "direction"=>vec!["UPDATE annual_goals SET direction_id=NULL WHERE direction_id=?"],
        "annual"=>vec!["UPDATE projects SET cycle_goal_id=NULL WHERE cycle_goal_id IN(SELECT id FROM cycle_goals WHERE annual_goal_id=?)","UPDATE tasks SET cycle_goal_id=NULL WHERE cycle_goal_id IN(SELECT id FROM cycle_goals WHERE annual_goal_id=?)","UPDATE tasks SET annual_goal_id=NULL WHERE annual_goal_id=?","DELETE FROM cycle_goals WHERE annual_goal_id=?"],
        "cycle"=>vec!["DELETE FROM assignment_weeks WHERE assignment_id IN(SELECT id FROM week_assignments WHERE cycle_id=? OR task_id IN(SELECT id FROM tasks WHERE cycle_id=?1))","UPDATE projects SET cycle_goal_id=NULL WHERE cycle_goal_id IN(SELECT id FROM cycle_goals WHERE cycle_id=?)","UPDATE tasks SET cycle_goal_id=NULL WHERE cycle_goal_id IN(SELECT id FROM cycle_goals WHERE cycle_id=?)","DELETE FROM cycle_goals WHERE cycle_id=?","UPDATE week_assignments SET cycle_id=NULL WHERE cycle_id=?","UPDATE tasks SET cycle_id=NULL WHERE cycle_id=?"],
        "task"=>vec!["UPDATE day_tasks SET assignment_id=NULL,task_id=NULL,project_id=NULL WHERE assignment_id IN(SELECT id FROM week_assignments WHERE task_id=?)","UPDATE day_tasks SET task_id=NULL,project_id=NULL WHERE task_id=?","UPDATE time_entries SET task_id=NULL WHERE task_id=?","UPDATE recurrences SET task_id=NULL WHERE task_id=?","DELETE FROM assignment_weeks WHERE assignment_id IN(SELECT id FROM week_assignments WHERE task_id=?)","DELETE FROM week_assignments WHERE task_id=?"],
        "assignment"=>vec!["UPDATE day_tasks SET assignment_id=NULL,task_id=NULL,project_id=NULL WHERE assignment_id=?","DELETE FROM assignment_weeks WHERE assignment_id=?"],
        "dayTask"=>vec!["INSERT OR IGNORE INTO recurrence_exceptions SELECT recurrence_id,date FROM day_tasks WHERE id=? AND recurrence_id IS NOT NULL","UPDATE plan_blocks SET day_task_id=NULL WHERE day_task_id=?","UPDATE time_entries SET day_task_id=NULL WHERE day_task_id=?"],
        "cycleGoal"=>vec!["UPDATE tasks SET cycle_goal_id=NULL WHERE cycle_goal_id=?","UPDATE projects SET cycle_goal_id=NULL WHERE cycle_goal_id=?"],
        "plan"=>vec!["INSERT OR IGNORE INTO recurrence_exceptions SELECT recurrence_id,date FROM plan_blocks WHERE id=? AND recurrence_id IS NOT NULL"],
        // Deleting a rule retains already generated history and detaches it.
        "recurrence"=>vec!["UPDATE day_tasks SET recurrence_id=NULL WHERE recurrence_id=?","UPDATE plan_blocks SET recurrence_id=NULL WHERE recurrence_id=?","DELETE FROM recurrence_exceptions WHERE recurrence_id=?"],
        "category"=>return Err("分类仅允许修改颜色".into()),_=>vec![],
    };
    for sql in statements {tx.execute(sql,[id]).map_err(err)?;}
    tx.execute(&format!("DELETE FROM {} WHERE id=?",entity(kind)?.0),[id]).map_err(err)?;Ok(())
}
fn project_entity(kind: &str) -> &str { match kind {"projectType"=>"type","projectPlan"=>"plan","projectUpdate"=>"update",_=>kind} }
fn apply(tx: &Transaction,op: &Value,defer_overlap:bool) -> Result<Value,String> {
    let kind=required(op,"kind")?;let action=required(op,"action")?;let id=required(op,"id")?;
    let (table,allowed)=entity(kind)?;let creating=action=="create";
    if creating {if exists(tx,kind,id).is_ok() {return Err("新增编号已存在".into());}}
    else {exists(tx,kind,id)?;}
    if action=="delete" {if kind=="placement" {super::unified::validate_project_boundary(tx,kind,&record(tx,kind,id)?,id)?;}delete(tx,kind,id)?;return Ok(json!({"kind":kind,"id":id,"deleted":true}));}
    if matches!(action,"link"|"unlink") {
        if !matches!(kind,"task"|"assignment") {return Err("只有本轮待办和周任务可关联项目规划".into());}
        super::long_projects::apply_in(tx,kind,action,id,Map::new(),vec![],Some(required(op,"plan_id")?.into()),vec![])?;
        return record(tx,kind,id);
    }
    if action=="weeks" {
        if kind!="assignment" {return Err("只有周任务可安排周次".into());}
        let ws=weeks(&op["weeks"])?;tx.execute("DELETE FROM assignment_weeks WHERE assignment_id=?",[id]).map_err(err)?;
        for w in ws {tx.execute("INSERT OR IGNORE INTO assignment_weeks VALUES(?,?)",params![id,w]).map_err(err)?;}
        return record(tx,kind,id);
    }
    if !matches!(action,"create"|"update") {return Err("操作应为create/update/delete/link/unlink/weeks".into());}
    if kind=="category" && creating {return Err("固定分类保持既有集合，目标分类随年度目标自动生成".into());}
    let fields=op["values"].as_object().ok_or("缺少values")?;
    if fields.keys().any(|k|!(kind=="item" && k=="project_item_ids") && !allowed.split(',').any(|a|a==k)) {return Err(format!("{kind} 包含不支持字段"));}
    if !creating && fields.is_empty() && op["project_ids"].is_null() && op["weeks"].is_null() {return Err("没有修改内容".into());}
    let mut value=if creating {json!({})} else {record(tx,kind,id)?};
    for (k,v) in fields {value[k]=v.clone();}
    if creating {
        value["created_at"]=op["_stamp"].clone();
        let defaults=match kind {
            "annual"=>json!({"status":"todo","completed":0}),
            "task"|"assignment"|"dayTask"=>json!({"status":"todo"}),
            "item"=>json!({"status":"todo","type":"todo","priority":"P2","description":"","level":"backlog"}),
            "projectVersion"=>json!({"description":""}),
            "cycle"=>json!({"review":""}),
            "project"=>json!({"status":"preparing","favorite":0,"description":"","focus":"","links":""}),
            "projectType"=>json!({"sort_order":0}),"stage"=>json!({"status":"planned","description":""}),
            "projectPlan"=>json!({"horizon":"later","completed":0,"description":""}),
            "recurrence"=>json!({"active":1,"weekdays":""}),"time"|"plan"=>json!({"title":""}),_=>json!({}),
        };
        for (k,v) in defaults.as_object().unwrap() {if value[k].is_null() {value[k]=v.clone();}}
        if kind=="projectType" {value.as_object_mut().unwrap().remove("created_at");}
        if kind=="assignment" {if value["review"].is_null() {value["review"]=json!("");}}

    }
    if kind=="project" {value["updated_at"]=op["_stamp"].clone();}
    if kind=="annual" {value["completed"]=json!(if text(&value,"status")=="done" {1} else {0});}
    let schedule_change=creating || fields.keys().any(|k|["date","assignment_id","scheduled_start_at","scheduled_end_at","scheduled_category_id"].contains(&k.as_str()));
    validate(tx,kind,&mut value,id,schedule_change,defer_overlap)?;
    if matches!(kind,"project"|"projectType"|"stage"|"projectPlan"|"projectUpdate"|"task"|"assignment") {
        let mut saved=Map::new();for (k,v) in value.as_object().unwrap() {if allowed.split(',').any(|s|s==k) || k=="created_at" || k=="updated_at" {saved.insert(k.clone(),v.clone());}}
        let project_ids=if op["project_ids"].is_null() && !creating && kind=="task" {rows(tx,"SELECT project_id FROM task_projects WHERE task_id=? ORDER BY project_id",&[&id])?.iter().map(|v|text(v,"project_id").to_string()).collect()} else {ids(&op["project_ids"])?};
        super::long_projects::apply_in(tx,project_entity(kind),"save",id,saved,project_ids,optional(op,"plan_id").map(str::to_owned),weeks(&op["weeks"])?)?;
        // Existing saves do not change placements in the native project API.
        if kind=="assignment" && !creating && !op["weeks"].is_null() {apply(tx,&json!({"kind":kind,"action":"weeks","id":id,"weeks":op["weeks"]}),false)?;}
        // Native task creation defaults to todo; preserve an explicitly requested state.
        if creating && matches!(kind,"task"|"assignment") {tx.execute(&format!("UPDATE {table} SET status=? WHERE id=?"),params![text(&value,"status"),id]).map_err(err)?;}
    } else if kind=="dayTask" && creating {super::ai_service::create_day(tx,id,&value)?;}
    else {
        if kind=="dayTask" && !creating && optional(&value,"recurrence_id").is_some() && text(&value,"date")!=text(&record(tx,kind,id)?,"date") {
            tx.execute("INSERT OR IGNORE INTO recurrence_exceptions SELECT recurrence_id,date FROM day_tasks WHERE id=?",[id]).map_err(err)?;value["recurrence_id"]=Value::Null;
        }
        let mut saved=Map::new();for (k,v) in value.as_object().unwrap() {
            if allowed.split(',').any(|s|s==k) || (creating && k=="created_at") || (kind=="annual" && k=="completed") || (matches!(kind,"dayTask"|"time") && matches!(k.as_str(),"project_id"|"task_id")) || (kind=="dayTask" && k=="recurrence_id") {saved.insert(k.clone(),v.clone());}
        }
        sql_save(tx,table,id,&saved,creating)?;

    }
    if kind=="item" && fields.contains_key("project_item_ids") {
        let links=fields["project_item_ids"].as_array().ok_or("project_item_ids须为编号数组")?;
        if !matches!(text(&value,"level"),"day"|"week"|"cycle") {return Err("只有执行任务可以关联项目事项".into());}
        tx.execute("DELETE FROM task_project_items WHERE task_id=?",[id]).map_err(err)?;
        for link in links {let target=link.as_str().ok_or("项目事项编号无效")?;if target==id{return Err("不能引用自身".into());}let item=record(tx,"item",target)?;if !super::unified::is_project_item(tx,&item)? {return Err("只能引用有项目归属的事项池记录".into());}tx.execute("INSERT OR IGNORE INTO task_project_items VALUES(?,?)",params![id,target]).map_err(err)?;}
    }
    record(tx,kind,id)
}
fn prepare(a: &Value,seed: &str) -> Result<Value,String> {
    let operations=a["operations"].as_array().filter(|x|!x.is_empty() && x.len()<=if x.iter().all(|o|matches!(text(o,"kind"),"time"|"plan"|"dailyPlan")){500}else{50}).ok_or("时间批次最多500项，其他批次最多50项")?;
    let mut refs=BTreeMap::new();let mut ops=vec![];let stamp=now();
    for (i,op) in operations.iter().enumerate() {
        let obj=op.as_object().ok_or("操作须为对象")?;
        if obj.keys().any(|k|!["action","kind","ref","id","values","project_ids","weeks","plan_id","expected_version"].contains(&k.as_str())) {return Err("操作包含不支持字段".into());}
        let mut op=op.clone();entity(required(&op,"kind")?)?;
        if text(&op,"action")=="create" {
            let id=format!("ai-{}",&hash(&json!([seed,i]))[..32]);op["id"]=json!(id);
            if let Some(r)=optional(&op,"ref") {if refs.insert(r.to_string(),id).is_some() {return Err("草稿ref重复".into());}}
        } else {required(&op,"id")?;}
        op["_stamp"]=json!(stamp);ops.push(op);
    }
    fn resolve(v: &mut Value,refs: &BTreeMap<String,String>) -> Result<(),String> {
        match v {
            Value::String(s) if s.starts_with('$')=>{*s=refs.get(&s[1..]).cloned().ok_or("未知草稿引用")?;},
            Value::String(s) if s.starts_with("goal:$")=>{*s=format!("goal:{}",refs.get(&s[6..]).ok_or("未知目标分类引用")?);},
            Value::Array(a)=>for v in a {resolve(v,refs)?;},Value::Object(o)=>for (_,v) in o {resolve(v,refs)?;},_=>{},
        } Ok(())
    }
    for op in &mut ops {resolve(op,&refs)?;}
    Ok(json!({"operations":ops,"editable_operations":a["operations"],"summary":a["summary"],"inferences":a["inferences"],"refs":refs}))
}
fn versions(c: &Connection,ops: &Value) -> Result<(),String> {
    for op in ops.as_array().unwrap() {
        if text(op,"action")!="create" {
            let before=record(c,required(op,"kind")?,required(op,"id")?)?;
            if optional(op,"expected_version").is_some() && text(&before,"version")!=text(op,"expected_version") {return Err("记录已变化，请重新查询".into());}
        }
    } Ok(())
}
fn apply_all(tx: &Transaction,ops: &Value) -> Result<Vec<Value>,String> {
    let mut records=vec![];
    let batch_time=ops.as_array().unwrap().iter().all(|op|matches!(text(op,"kind"),"time"|"plan"));
    for (i,op) in ops.as_array().unwrap().iter().enumerate() {records.push(apply(tx,op,batch_time).map_err(|e|format!("第{}项：{e}",i+1))?);}
    if batch_time {
        for (i,op) in ops.as_array().unwrap().iter().enumerate() {
            if text(op,"kind")=="time" && text(op,"action")!="delete" {
                let v=record(tx,"time",text(op,"id"))?;
                valid_time(tx,text(&v,"start_at"),text(&v,"end_at"),text(&v,"category_id"),text(op,"id"),false).map_err(|e|format!("第{}项：{e}",i+1))?;
                if let Some(source)=optional(&v,"source_plan_id") {let duplicate:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM time_entries WHERE source_plan_id=? AND id<>?)",params![source,text(op,"id")],|r|r.get(0)).map_err(err)?;if duplicate{return Err(format!("第{}项：这个计划时段已经回填实际时间",i+1));}}

            }
        }
    }
    super::unified::validate_project_links(tx)?;
    // A parent/date/placement change must not leave its descendants in another cycle/week.
    for op in ops.as_array().unwrap() {
        if text(op,"action")=="delete" {continue;}
        let k=text(op,"kind");let id=text(op,"id");
        let structural=matches!(text(op,"action"),"create"|"weeks") || ["cycle_id","task_id","assignment_id","date","start_date"].iter().any(|f|op["values"].get(f).is_some()) || !op["weeks"].is_null();
        if !structural {continue;}
        let sql=match k {
            "task"=>"SELECT d.* FROM day_tasks d JOIN week_assignments w ON d.assignment_id=w.id WHERE w.task_id=?",
            "assignment"=>"SELECT * FROM day_tasks WHERE assignment_id=?",
            "cycle"=>"SELECT d.* FROM day_tasks d JOIN week_assignments w ON d.assignment_id=w.id WHERE w.cycle_id=?",
            _=>continue,
        };
        for mut d in rows(tx,sql,&[&id])? {let did=text(&d,"id").to_string();validate(tx,"dayTask",&mut d,&did,true,false)?;}
    }
    // Later operations may change the reverse references of an earlier item.
    for (saved,op) in records.iter_mut().zip(ops.as_array().unwrap()) {
        if text(op,"kind")=="item" && text(op,"action")!="delete" {*saved=record(tx,"item",text(op,"id"))?;}
    }
    Ok(records)
}
fn preview(tx: &Transaction,payload: &Value) -> Result<Value,String> {
    versions(tx,&payload["operations"])?;
    let before=state(tx)?;tx.execute_batch("SAVEPOINT draft_preview").map_err(err)?;
    let result=(|| {let records=apply_all(tx,&payload["operations"])?;Ok(json!({"records":records,"changes":changes(&before,&state(tx)?),"notice":"计划与实际独立保存；计划时段不会自动生成实际记录。"}))})();
    tx.execute_batch("ROLLBACK TO draft_preview; RELEASE draft_preview").map_err(err)?;result
}

pub fn draft(c: &mut Connection,method: &str,a: &Value,owner: &str,source: &str,confirmation: Option<&str>) -> Result<Value,String> {
    let request=required(a,"request_id")?;if request.len()>200 {return Err("请求编号过长".into());}
    let input=json!({"method":method,"args":a,"actor":owner}).to_string();
    let tx=c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
    if let Some((old,s))=tx.query_row("SELECT request_json,result_json FROM ai_operations WHERE request_id=?",[request],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(err)? {
        if old!=input {return Err("请求编号已用于不同内容".into());}return serde_json::from_str(&s).map_err(err);
    }
    let mut result=if method=="create_draft" {
        let id=format!("draft-{}",&hash(&json!([owner,request]))[..12]);let payload=prepare(a,&id)?;let p=preview(&tx,&payload)?;
        tx.execute("INSERT INTO ai_drafts VALUES(?,?,1,'pending',?,?,?,?,?,?)",params![id,owner,payload.to_string(),hash(&state(&tx)?),p.to_string(),source,now(),now()]).map_err(err)?;
        json!({"status":"pending_confirmation","draft_id":id,"revision":1,"payload":payload,"preview":p,"confirmation":format!("确认 {id} 1")})
    } else {
        let id=required(a,"draft_id")?;
        let (stored_owner,rev,state_name,raw,guard,src):(String,i64,String,String,String,String)=tx.query_row("SELECT owner,revision,state,payload_json,context_hash,source_message_id FROM ai_drafts WHERE id=?",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).map_err(err)?;
        if stored_owner!=owner {return Err("该草稿不属于当前用户".into());}
        if a["revision"].as_i64()!=Some(rev) {return Err("草稿版本已变化，请确认最新版".into());}
        if !matches!(state_name.as_str(),"pending"|"stale") {return Err("草稿已取消或已提交，请查询原回执".into());}
        match method {
            "revise_draft"=>{
                let payload=prepare(a,&format!("{id}:{next}",next=rev+1))?;let p=preview(&tx,&payload)?;
                tx.execute("UPDATE ai_drafts SET revision=revision+1,state='pending',payload_json=?,context_hash=?,preview_json=?,source_message_id=?,updated_at=? WHERE id=?",params![payload.to_string(),hash(&state(&tx)?),p.to_string(),source,now(),id]).map_err(err)?;
                json!({"status":"pending_confirmation","draft_id":id,"revision":rev+1,"payload":payload,"preview":p,"confirmation":format!("确认 {id} {}",rev+1)})
            },
            "cancel_draft"=>{tx.execute("UPDATE ai_drafts SET state='cancelled',updated_at=? WHERE id=?",params![now(),id]).map_err(err)?;json!({"status":"cancelled","draft_id":id,"revision":rev})},
            "confirm_draft"=>{
                let msg=confirmation.ok_or("缺少用户确认消息")?.trim();
                let exact=format!("确认 {id} {rev}");
                let pending:i64=tx.query_row("SELECT COUNT(*) FROM ai_drafts WHERE owner=? AND state='pending'",[owner],|r|r.get(0)).map_err(err)?;
                if msg!=exact && !(msg=="确认" && pending==1) {return Err(format!("请回复：{exact}；多份草稿不能用一句确认提交"));}
                if owner.starts_with("weixin:") && (source.is_empty() || source==src) {return Err("需要草稿生成之后的微信确认消息".into());}
                if state_name=="stale" || guard!=hash(&state(&tx)?) {
                    tx.execute("UPDATE ai_drafts SET state='stale',updated_at=? WHERE id=?",params![now(),id]).map_err(err)?;
                    json!({"status":"needs_revision","draft_id":id,"revision":rev,"reason":"工作台数据已变化，请重新查询、修订并再次确认"})
                } else {
                    let payload:Value=serde_json::from_str(&raw).map_err(err)?;versions(&tx,&payload["operations"])?;
                    let before=state(&tx)?;let records=apply_all(&tx,&payload["operations"])?;let changed=changes(&before,&state(&tx)?);
                    tx.execute("UPDATE ai_drafts SET state='committed',updated_at=? WHERE id=?",params![now(),id]).map_err(err)?;
                    audit(&tx,request,owner,"confirm_draft",&before,&json!(changed))?;
                    json!({"status":"saved","draft_id":id,"revision":rev,"records":records,"changes":changed})
                }
            },_=>return Err("不支持草稿操作".into()),
        }
    };
    result["operation_id"]=json!(request);
    tx.execute("INSERT INTO ai_operations VALUES(?,?,?,?)",params![request,input,result.to_string(),now()]).map_err(err)?;tx.commit().map_err(err)?;Ok(result)
}
fn audit(tx: &Transaction,request: &str,owner: &str,action: &str,before: &Value,after: &Value) -> Result<(),String> {
    tx.execute("INSERT INTO ai_audit(request_id,actor,action,before_json,after_json,created_at) VALUES(?,?,?,?,?,?)",params![request,owner,action,before.to_string(),after.to_string(),now()]).map_err(err)?;Ok(())
}
pub fn execute(c: &mut Connection,a: &Value,owner: &str) -> Result<Value,String> {
    let request=required(a,"request_id")?;if request.len()>200 {return Err("请求编号过长".into());}
    if text(a,"intent")!="explicit" {return Err("AI推断的修改须创建草稿并确认".into());}
    let payload=prepare(&if owner=="workbench-ui" && a["operations"].is_array() {json!({"operations":a["operations"]})}else{json!({"operations":[a["operation"]]})},request)?;let op=&payload["operations"][0];
    if owner!="workbench-ui" && (text(op,"action")=="delete" || (text(op,"action")=="create" && matches!(text(op,"kind"),"item"|"placement"|"task"|"assignment"|"dayTask"|"recurrence"))) {return Err("新增待办、重复事项及删除必须先创建草稿并确认".into());}
    if text(op,"action")!="create" {required(op,"expected_version")?;}
    let input=json!({"method":"execute_operation","args":a,"actor":owner}).to_string();
    let tx=c.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).map_err(err)?;
    if let Some((old,s))=tx.query_row("SELECT request_json,result_json FROM ai_operations WHERE request_id=?",[request],|r|Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?))).optional().map_err(err)? {if old!=input {return Err("请求编号已用于不同内容".into());}return serde_json::from_str(&s).map_err(err);}
    versions(&tx,&payload["operations"])?;let before=state(&tx)?;let records=apply_all(&tx,&payload["operations"])?;let changed=changes(&before,&state(&tx)?);
    audit(&tx,request,owner,"execute_operation",&before,&json!(changed))?;
    let result=json!({"status":"saved","operation_id":request,"records":records,"changes":changed});
    tx.execute("INSERT INTO ai_operations VALUES(?,?,?,?)",params![request,input,result.to_string(),now()]).map_err(err)?;tx.commit().map_err(err)?;Ok(result)
}
pub fn query_drafts(c: &Connection,a: &Value,owner: &str) -> Result<Value,String> {
    let drafts=if let Some(id)=optional(a,"draft_id") {rows(c,"SELECT id,revision,state,payload_json,preview_json,created_at,updated_at FROM ai_drafts WHERE owner=? AND id=?",&[&owner,&id])?} else {rows(c,"SELECT id,revision,state,payload_json,preview_json,created_at,updated_at FROM ai_drafts WHERE owner=? AND state IN ('pending','stale') ORDER BY updated_at DESC LIMIT 50",&[&owner])?};
    Ok(json!({"items":drafts}))
}

pub fn statistics(c: &Connection,a: &Value) -> Result<Value,String> {
    let from=date(required(a,"date_from")?)?;let to=date(required(a,"date_to")?)?;
    if to<from || (to-from).num_days()>3660 {return Err("统计范围应为0—3660天".into());}
    let mut counts=Map::new();
    for kind in ["task","assignment","dayTask"] {
        let mut q=json!({"kind":kind,"limit":100,"offset":0});
        if optional(a,"cycle_id").is_some() && kind!="dayTask" {q["cycle_id"]=a["cycle_id"].clone();}
        if optional(a,"project_id").is_some() {q["project_id"]=a["project_id"].clone();}
        if kind=="dayTask" {q["date_from"]=json!(from.to_string());q["date_to"]=json!(to.to_string());}
        let mut total=json!({"todo":0,"doing":0,"done":0,"missed":0,"total":0});
        loop {
            let page=query(c,&q)?;
            for row in page["items"].as_array().unwrap() {
                if kind=="dayTask" && optional(a,"cycle_id").is_some() {
                    let linked=optional(row,"task_id").and_then(|id|record(c,"task",id).ok());
                    if linked.as_ref().map(|r|text(r,"cycle_id"))!=Some(text(a,"cycle_id")) {continue;}
                }
                if kind=="assignment" {
                    let cycle=optional(row,"cycle_id").and_then(|id|record(c,"cycle",id).ok());
                    let in_range=if let Some(cycle)=cycle {let start=date(text(&cycle,"start_date"))?;row["weeks"].as_array().unwrap().iter().any(|w|{let s=start+Duration::days((w["week_no"].as_i64().unwrap()-1)*7);s<=to && s+Duration::days(6)>=from})} else {false};
                    if !in_range {continue;}
                }
                let k=text(row,"status");if !total[k].is_null() {total[k]=json!(total[k].as_i64().unwrap()+1);}total["total"]=json!(total["total"].as_i64().unwrap()+1);
            }
            if page["next_offset"].is_null() {break;}q["offset"]=page["next_offset"].clone();
        }
        counts.insert(kind.into(),total);
    }
    let lo=format!("{from}T00:00:00+08:00");let hi=format!("{}T00:00:00+08:00",to+Duration::days(1));
    let entries=rows(c,"SELECT *,max(0,(min(julianday(end_at),julianday(?2))-max(julianday(start_at),julianday(?1)))*1440) AS minutes FROM time_entries WHERE julianday(end_at)>julianday(?1) AND julianday(start_at)<julianday(?2)",&[&lo,&hi])?;
    let mut categories=BTreeMap::<String,f64>::new();let mut total=0.0;
    for e in entries {
        if optional(a,"project_id").is_some() && text(&e,"long_project_id")!=text(a,"project_id") {continue;}
        if optional(a,"cycle_id").is_some() && optional(&e,"task_id").and_then(|id|record(c,"task",id).ok()).as_ref().map(|t|text(t,"cycle_id"))!=Some(text(a,"cycle_id")) {continue;}
        let min=e["minutes"].as_f64().unwrap_or(0.0);total+=min;*categories.entry(text(&e,"category_id").to_string()).or_default()+=min;
    }
    let mut planned=0i64;
    for p in rows(c,"SELECT * FROM plan_blocks WHERE date>=? AND date<=?",&[&from.to_string(),&to.to_string()])? {
        let day=optional(&p,"day_task_id").and_then(|id|record(c,"dayTask",id).ok());
        if optional(a,"project_id").is_some() && day.as_ref().map(|d|text(d,"long_project_id"))!=Some(text(a,"project_id")) {continue;}
        if optional(a,"cycle_id").is_some() && day.as_ref().and_then(|d|optional(d,"task_id")).and_then(|id|record(c,"task",id).ok()).as_ref().map(|t|text(t,"cycle_id"))!=Some(text(a,"cycle_id")) {continue;}
        planned+=p["end_min"].as_i64().unwrap_or(0)-p["start_min"].as_i64().unwrap_or(0);
    }
    let unified=if c.query_row("SELECT EXISTS(SELECT 1 FROM pragma_table_info('time_entries') WHERE name='work_kind')",[],|r|r.get::<_,bool>(0)).map_err(err)? {super::unified::statistics(c,a)?}else{Value::Null};
    Ok(json!({"unified":unified,"date_from":from.to_string(),"date_to":to.to_string(),"timezone":"Asia/Shanghai","status_counts":counts,"recorded_minutes":total,"category_minutes":categories,"planned_minutes":planned,"limitations":["完成情况是当前状态；本轮待办按周期/项目筛选，周任务按安排周与范围交集，日任务按日期筛选。","已记录用时包含带时间日任务生成的记录，不等于经核实的实际投入。"]}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn database() -> Connection {
        let mut c=Connection::open_in_memory().unwrap();c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON;").unwrap();
        let src=include_str!("../../src/db.ts");
        for line in src.lines().map(str::trim).filter(|s|s.starts_with("'CREATE TABLE") || s.starts_with("'CREATE INDEX")) {
            let sql=line.trim_start_matches('\'').trim_end_matches(',').trim_end_matches('\'').replace("\\'","'");c.execute_batch(&sql).unwrap();
        }
        super::super::long_projects::migrate_in(&mut c).unwrap();super::super::task_management::upgrade(&mut c).unwrap();super::super::time_categories::upgrade(&mut c).unwrap();
        for name in ["create_time_entry_for_timed_day_task","sync_time_entry_parent_on_day_task_change","sync_day_task_parent_on_week_task_change","sync_week_cycle_on_task_change"] {
            let start=src.find(&format!("CREATE TRIGGER IF NOT EXISTS {name}")).unwrap();let end=src[start..].find("END").unwrap()+start+3;c.execute_batch(&src[start..end]).unwrap();
        }
        super::super::ai_service::schema(&c).unwrap();schema(&c).unwrap();c
    }
    fn graph() -> Value {json!([
        {"kind":"direction","action":"create","ref":"dir","values":{"title":"职业发展"}},
        {"kind":"annual","action":"create","ref":"goal","values":{"title":"提升职业能力","year":2026,"direction_id":"$dir"}},
        {"kind":"cycle","action":"create","ref":"cycle","values":{"title":"本轮","start_date":"2026-10-01"}},
        {"kind":"project","action":"create","ref":"project","values":{"title":"求职"}},
        {"kind":"task","action":"create","ref":"task","values":{"title":"准备面试","cycle_id":"$cycle","annual_goal_id":"$goal"},"project_ids":["$project"]},
        {"kind":"assignment","action":"create","ref":"week","values":{"task_id":"$task","commitment":"完成面试准备","long_project_id":"$project","priority":"P1"},"weeks":[2]},
        {"kind":"dayTask","action":"create","ref":"day","values":{"title":"超眼智能面试","date":"2026-10-10","assignment_id":"$week","scheduled_start_at":"2026-10-10T15:30:00+08:00","scheduled_end_at":"2026-10-10T16:30:00+08:00","scheduled_category_id":"goal:$goal"}}
    ])}
    fn make(c: &mut Connection,request: &str,ops: Value) -> Value {draft(c,"create_draft",&json!({"request_id":request,"operations":ops,"summary":"待办方案","inferences":["补齐上级"]}),"weixin:a:u","message-1",None).unwrap()}
    fn confirm(c: &mut Connection,d: &Value,request: &str) -> Result<Value,String> {draft(c,"confirm_draft",&json!({"request_id":request,"draft_id":d["draft_id"],"revision":d["revision"]}),"weixin:a:u","message-2",Some(text(d,"confirmation")))}
    fn count(c: &Connection,t: &str)->i64 {c.query_row(&format!("SELECT COUNT(*) FROM {t}"),[],|r|r.get(0)).unwrap()}
    #[test] fn project_item_references_are_atomic_independent_and_versioned() {
        let mut c=time_database();
        let ops=json!([
          {"kind":"project","action":"create","ref":"p","values":{"title":"产品"}},
          {"kind":"item","action":"create","ref":"a","values":{"title":"需求","project_id":"$p"}},
          {"kind":"item","action":"create","ref":"b","values":{"title":"Bug","type":"bug","project_id":"$p"}},
          {"kind":"item","action":"create","ref":"t","values":{"title":"推进","level":"cycle","project_item_ids":["$a","$b","$a"]}},
          {"kind":"item","action":"create","values":{"title":"另一个任务","level":"week","project_item_ids":["$a"]}}
        ]);
        let before=state(&c).unwrap();let draft=make(&mut c,"links-draft",ops);assert_eq!(before,state(&c).unwrap());
        let receipt=confirm(&mut c,&draft,"links-confirm").unwrap();let a=text(&receipt["records"][1],"id");let b=text(&receipt["records"][2],"id");let t=text(&receipt["records"][3],"id");
        assert_eq!(count(&c,"task_project_items"),3);
        assert_eq!(receipt["records"][1]["version"],record(&c,"item",a).unwrap()["version"]);
        assert_eq!(query(&c,&json!({"kind":"item","project_item_id":a})).unwrap()["items"].as_array().unwrap().len(),2);
        let original=record(&c,"item",t).unwrap();assert!(original["project_id"].is_null());assert!(original["parent_id"].is_null());
        save_times(&mut c,"status-only",json!([{"kind":"item","action":"update","id":t,"values":{"status":"done"}}])).unwrap();
        assert_eq!(record(&c,"item",a).unwrap()["status"],"todo");assert_eq!(count(&c,"task_project_items"),3);
        let version=record(&c,"item",t).unwrap()["version"].clone();
        save_times(&mut c,"unlink",json!([{"kind":"item","action":"update","id":t,"values":{"project_item_ids":[]}}])).unwrap();
        assert_eq!(count(&c,"task_project_items"),1);assert_ne!(record(&c,"item",t).unwrap()["version"],version);
        assert!(save_times(&mut c,"stale-link",json!([{"kind":"item","action":"update","id":t,"expected_version":version,"values":{"project_item_ids":[a]}}])).is_err());
        let before=state(&c).unwrap();assert!(save_times(&mut c,"bad-link",json!([{"kind":"item","action":"update","id":t,"values":{"title":"不应保存","project_item_ids":[b,"missing"]}}])).is_err());assert_eq!(state(&c).unwrap(),before);
        save_times(&mut c,"delete-target",json!([{"kind":"item","action":"delete","id":a}])).unwrap();assert_eq!(count(&c,"task_project_items"),0);assert!(record(&c,"item",t).is_ok());assert!(record(&c,"item",b).is_ok());
    }
    #[test] fn project_item_boundary_preserves_history_but_rejects_new_schedules() {
        let mut c=time_database();
        c.execute_batch("INSERT INTO long_projects(id,title,created_at,updated_at) VALUES('p','项目','',''); INSERT INTO items(id,title,project_id,level,created_at) VALUES('pool','事项','p','backlog',''); INSERT INTO items(id,title,level,created_at) VALUES('task','任务','cycle',''); UPDATE items SET parent_id='task' WHERE id='pool'; INSERT INTO placements(id,item_id,kind,date,created_at) VALUES('old','pool','day','2026-10-01','');").unwrap();
        save_times(&mut c,"rename",json!([{"kind":"item","action":"update","id":"pool","values":{"title":"重命名","status":"done"}}])).unwrap();
        assert_eq!(record(&c,"item","pool").unwrap()["parent_id"],"task");assert_eq!(count(&c,"placements"),1);
        for (n,op) in [
          json!({"kind":"item","action":"update","id":"pool","values":{"level":"week"}}),
          json!({"kind":"item","action":"update","id":"pool","values":{"project_id":null}}),
          json!({"kind":"item","action":"update","id":"task","values":{"parent_id":"pool"}}),
          json!({"kind":"item","action":"update","id":"pool","values":{"parent_id":null}}),
          json!({"kind":"placement","action":"create","values":{"item_id":"pool","kind":"day","date":"2026-10-15"}}),
          json!({"kind":"placement","action":"update","id":"old","values":{"date":"2026-10-16"}}),
          json!({"kind":"item","action":"update","id":"pool","values":{"project_item_ids":["task"]}}),
          json!({"kind":"item","action":"update","id":"task","values":{"project_item_ids":["task"]}})
        ].into_iter().enumerate(){let before=state(&c).unwrap();assert!(save_times(&mut c,&format!("invalid-{n}"),json!([op])).is_err(),"invalid operation {n} must be rejected");assert_eq!(state(&c).unwrap(),before);}
        let before=state(&c).unwrap();super::super::unified::migrate(&mut c).unwrap();assert_eq!(state(&c).unwrap(),before);
    }
    #[test] fn preview_is_readonly_and_commit_creates_complete_hierarchy() {
        let mut c=database();let before=state(&c).unwrap();let d=make(&mut c,"new",graph());assert_eq!(state(&c).unwrap(),before);
        assert!(d["preview"]["changes"].as_array().unwrap().iter().any(|v|v["table"]=="time_entries"));
        let saved=confirm(&mut c,&d,"confirmed").unwrap();assert_eq!(saved["status"],"saved");
        for t in ["directions","annual_goals","cycles","tasks","week_assignments","day_tasks","time_entries"] {assert_eq!(count(&c,t),1);}
        let day=&saved["records"][6];assert_eq!(day["task_id"],saved["records"][4]["id"]);assert_eq!(day["long_project_id"],saved["records"][3]["id"]);
        assert_eq!(confirm(&mut c,&d,"confirmed").unwrap(),saved);assert!(confirm(&mut c,&d,"another").is_err());
    }
    #[test] fn direct_task_creation_and_delete_are_blocked() {
        let mut c=database();for action in ["create","delete"] {assert!(execute(&mut c,&json!({"request_id":action,"intent":"explicit","operation":{"kind":"dayTask","action":action,"id":"x","values":{"title":"任务","date":"2026-10-10"}}}),"codex").unwrap_err().contains("草稿"));}assert_eq!(count(&c,"day_tasks"),0);
    }
    #[test] fn cancellation_revisions_ownership_and_real_confirmation() {
        let mut c=database();let d=make(&mut c,"one",graph());let args=json!({"request_id":"confirm","draft_id":d["draft_id"],"revision":1});
        assert!(draft(&mut c,"confirm_draft",&args,"weixin:a:other","message-2",Some("确认")).unwrap_err().contains("不属于"));
        assert!(draft(&mut c,"confirm_draft",&args,"weixin:a:u","message-1",Some(text(&d,"confirmation"))).is_err());
        assert!(draft(&mut c,"confirm_draft",&args,"weixin:a:u","message-2",Some("把会议改到明天")).is_err());
        let revised=draft(&mut c,"revise_draft",&json!({"request_id":"revise","draft_id":d["draft_id"],"revision":1,"operations":d["payload"]["editable_operations"],"summary":"新版","inferences":[]}),"weixin:a:u","message-2",None).unwrap();
        assert_eq!(revised["revision"],2);assert!(confirm(&mut c,&d,"old").unwrap_err().contains("版本"));
        draft(&mut c,"cancel_draft",&json!({"request_id":"cancel","draft_id":d["draft_id"],"revision":2}),"weixin:a:u","message-3",None).unwrap();assert!(confirm(&mut c,&revised,"cancelled").is_err());assert_eq!(count(&c,"tasks"),0);
    }
    #[test] fn ambiguous_confirmation_and_changed_context_require_revision() {
        let mut c=database();let d=make(&mut c,"one",graph());make(&mut c,"two",graph());
        assert!(draft(&mut c,"confirm_draft",&json!({"request_id":"ambiguous","draft_id":d["draft_id"],"revision":1}),"weixin:a:u","m2",Some("确认")).is_err());
        c.execute("INSERT INTO directions VALUES('external','并发新增','now')",[]).unwrap();
        assert_eq!(confirm(&mut c,&d,"stale").unwrap()["status"],"needs_revision");assert_eq!(count(&c,"tasks"),0);
    }
    #[test] fn invalid_descendants_and_time_conflicts_roll_back_parents() {
        let mut c=database();let mut ops=graph();ops[6]["values"]["date"]=json!("2026-12-31");
        assert!(draft(&mut c,"create_draft",&json!({"request_id":"bad","operations":ops}),"codex","",None).unwrap_err().contains("日期所在周"));assert_eq!(count(&c,"directions"),0);
        let mut ops=graph();let mut second=ops[6].clone();second["ref"]=json!("second");ops.as_array_mut().unwrap().push(second);
        assert!(draft(&mut c,"create_draft",&json!({"request_id":"overlap","operations":ops}),"codex","",None).is_err());assert_eq!(count(&c,"tasks"),0);assert_eq!(count(&c,"time_entries"),0);
    }
    #[test] fn explicit_status_only_changes_selected_record_and_conflicts_are_detected() {
        let mut c=database();let d=make(&mut c,"graph",graph());let saved=confirm(&mut c,&d,"save").unwrap();let day=&saved["records"][6];
        let args=json!({"request_id":"status","intent":"explicit","operation":{"kind":"dayTask","action":"update","id":day["id"],"expected_version":day["version"],"values":{"status":"doing"}}});
        let result=execute(&mut c,&args,"codex").unwrap();assert_eq!(result["records"][0]["status"],"doing");
        assert_eq!(record(&c,"assignment",text(&saved["records"][5],"id")).unwrap()["status"],"todo");
        let mut stale=args.clone();stale["request_id"]=json!("stale-status");assert!(execute(&mut c,&stale,"codex").is_err());assert_eq!(execute(&mut c,&args,"codex").unwrap(),result);
    }
    #[test] fn status_change_preserves_independently_edited_recorded_time() {
        let mut c=database();let d=make(&mut c,"graph",graph());let saved=confirm(&mut c,&d,"save").unwrap();let day=&saved["records"][6];
        let eid=format!("{}:time",text(day,"id"));
        c.execute("UPDATE time_entries SET start_at='2026-10-10T08:00:00Z',end_at='2026-10-10T09:00:00Z',title='实际面试时间' WHERE id=?",[&eid]).unwrap();
        let before=record(&c,"time",&eid).unwrap();
        execute(&mut c,&json!({"request_id":"mark-done","intent":"explicit","operation":{"kind":"dayTask","action":"update","id":day["id"],"expected_version":day["version"],"values":{"status":"done"}}}),"codex").unwrap();
        assert_eq!(record(&c,"time",&eid).unwrap(),before);
    }
    #[test] fn delete_preview_lists_cascade_but_preserves_daily_and_time_history() {
        let mut c=database();let d=make(&mut c,"graph",graph());let saved=confirm(&mut c,&d,"save").unwrap();let task=&saved["records"][4];
        let del=make(&mut c,"delete",json!([{"kind":"task","action":"delete","id":task["id"],"expected_version":task["version"]}]));
        assert_eq!(count(&c,"tasks"),1);assert!(del["preview"]["changes"].as_array().unwrap().iter().any(|v|v["table"]=="week_assignments" && v["after"].is_null()));
        confirm(&mut c,&del,"del-confirm").unwrap();assert_eq!(count(&c,"tasks"),0);assert_eq!(count(&c,"week_assignments"),0);assert_eq!(count(&c,"day_tasks"),1);assert_eq!(count(&c,"time_entries"),1);
    }
    #[test] fn project_calendar_recurrence_statistics_and_category_operations() {
        let mut c=database();let d=make(&mut c,"graph",graph());let saved=confirm(&mut c,&d,"save").unwrap();let project=&saved["records"][3];let day=&saved["records"][6];
        let created=execute(&mut c,&json!({"request_id":"stage","intent":"explicit","operation":{"kind":"stage","action":"create","values":{"project_id":project["id"],"title":"面试阶段","start_date":"2026-10-10","end_date":"2026-10-20"}}}),"codex").unwrap();let stage=&created["records"][0];
        execute(&mut c,&json!({"request_id":"plan","intent":"explicit","operation":{"kind":"projectPlan","action":"create","values":{"project_id":project["id"],"stage_id":stage["id"],"title":"准备面试","horizon":"now"}}}),"codex").unwrap();
        execute(&mut c,&json!({"request_id":"calendar","intent":"explicit","operation":{"kind":"plan","action":"create","values":{"title":"面试","date":"2026-10-10","start_min":930,"end_min":990,"day_task_id":day["id"]}}}),"codex").unwrap();
        execute(&mut c,&json!({"request_id":"template","intent":"explicit","operation":{"kind":"dailyPlan","action":"create","values":{"title":"晨间规划","start_min":480,"end_min":500}}}),"codex").unwrap();
        let rule=make(&mut c,"rule",json!([{"kind":"recurrence","action":"create","values":{"title":"阅读","kind":"day_task","frequency":"weekly","weekdays":"1,3,5","start_date":"2026-10-10"}}]));confirm(&mut c,&rule,"rule-confirm").unwrap();
        let stats=statistics(&c,&json!({"date_from":"2026-10-10","date_to":"2026-10-10"})).unwrap();assert_eq!(stats["status_counts"]["dayTask"]["total"],1);assert_eq!(stats["planned_minutes"],60);assert!((stats["recorded_minutes"].as_f64().unwrap()-60.0).abs()<0.001);
        let cat=record(&c,"category",text(&day,"scheduled_category_id")).unwrap();execute(&mut c,&json!({"request_id":"color","intent":"explicit","operation":{"kind":"category","action":"update","id":cat["id"],"expected_version":cat["version"],"values":{"color":"#123456"}}}),"codex").unwrap();
        assert_eq!(query(&c,&json!({"kind":"stage","project_id":project["id"]})).unwrap()["items"].as_array().unwrap().len(),1);
    }
    fn time_database()->Connection {let mut c=database();super::super::unified::migrate(&mut c).unwrap();c}
    fn save_times(c:&mut Connection,request:&str,mut ops:Value)->Result<Value,String>{for op in ops.as_array_mut().unwrap(){if text(op,"action")!="create" && op["expected_version"].is_null(){op["expected_version"]=record(c,text(op,"kind"),text(op,"id"))?["version"].clone();}}execute(c,&json!({"request_id":request,"intent":"explicit","operations":ops}),"workbench-ui")}
    #[test] fn day_editor_validates_final_state_and_rolls_back(){
        let mut c=time_database();
        let made=save_times(&mut c,"create-times",json!([
            {"kind":"time","action":"create","values":{"start_at":"2026-10-14T09:00:00+08:00","end_at":"2026-10-14T10:00:00+08:00","work_kind":"work"}},
            {"kind":"time","action":"create","values":{"start_at":"2026-10-14T10:00:00+08:00","end_at":"2026-10-14T11:00:00+08:00","work_kind":"work"}}
        ])).unwrap();
        let a=made["records"][0]["id"].clone();let b=made["records"][1]["id"].clone();
        let va=record(&c,"time",a.as_str().unwrap()).unwrap()["version"].clone();
        save_times(&mut c,"move-boundary",json!([
            {"kind":"time","action":"update","id":a,"expected_version":va,"values":{"end_at":"2026-10-14T10:30:00+08:00"}},
            {"kind":"time","action":"update","id":b,"values":{"start_at":"2026-10-14T10:30:00+08:00"}}
        ])).unwrap();
        let before=state(&c).unwrap();
        assert!(save_times(&mut c,"overlap-reject",json!([
            {"kind":"time","action":"update","id":a,"values":{"title":"must rollback"}},
            {"kind":"time","action":"update","id":b,"values":{"start_at":"2026-10-14T10:00:00+08:00"}}
        ])).is_err());assert_eq!(before,state(&c).unwrap());
        assert!(save_times(&mut c,"stale",json!([{"kind":"time","action":"update","id":a,"expected_version":va,"values":{"title":"stale"}}])).is_err());
        assert_eq!(record(&c,"time",a.as_str().unwrap()).unwrap()["category_id"],"system:work");
    }
    #[test] fn day_editor_large_batches_backfill_and_migration(){
        let mut c=time_database();let mut ops=vec![];
        for n in 0..60 {ops.push(json!({"kind":"plan","action":"create","values":{"date":"2026-10-14","start_min":n*10,"end_min":n*10+10,"work_kind":"work"}}));}
        let saved=save_times(&mut c,"sixty",json!(ops)).unwrap();assert_eq!(saved["records"].as_array().unwrap().len(),60);
        assert_eq!(save_times(&mut c,"sixty",json!(ops)).unwrap(),saved);
        let pid=saved["records"][0]["id"].clone();
        let values=json!({"start_at":"2026-10-14T23:30:00+08:00","end_at":"2026-10-15T00:30:00+08:00","work_kind":"work","source_plan_id":pid});
        save_times(&mut c,"fill",json!([{"kind":"time","action":"create","values":values}])).unwrap();
        assert!(save_times(&mut c,"fill-twice",json!([{"kind":"time","action":"create","values":values}])).is_err());
        assert!(save_times(&mut c,"next-day-overlap",json!([{"kind":"time","action":"create","values":{"start_at":"2026-10-15T00:15:00+08:00","end_at":"2026-10-15T01:00:00+08:00","work_kind":"work"}}])).is_err());
        let before=state(&c).unwrap();super::super::unified::migrate(&mut c).unwrap();assert_eq!(before,state(&c).unwrap());
    }

    #[test] fn day_editor_preview_manual_life_and_direction_snapshots(){
        let mut c=time_database();
        c.execute_batch("INSERT INTO directions VALUES('dir','开发',''); INSERT INTO annual_goals(id,title,year,direction_id,created_at) VALUES('g','产品',2026,'dir',''); INSERT INTO items(id,title,annual_goal_id,created_at) VALUES('item','开发','g','');").unwrap();
        let sleep:String=c.query_row("SELECT id FROM categories WHERE name='睡觉'",[],|r|r.get(0)).unwrap();
        let made=save_times(&mut c,"manual",json!([
          {"kind":"time","action":"create","values":{"start_at":"2026-10-14T08:00:00+08:00","end_at":"2026-10-14T09:00:00+08:00","work_kind":"life","category_manual":1,"category_id":sleep,"item_id":"item"}},
          {"kind":"plan","action":"create","values":{"date":"2026-10-14","start_min":600,"end_min":660,"item_id":"item"}}
        ])).unwrap();
        assert_eq!(made["records"][0]["work_kind"],"life");assert_eq!(made["records"][1]["work_kind"],"work");assert_eq!(made["records"][1]["direction_id"],"dir");
        let before=state(&c).unwrap();let payload=prepare(&json!({"operations":[{"kind":"time","action":"update","id":made["records"][0]["id"],"values":{"end_at":"2026-10-14T09:30:00+08:00"}}]}),"preview-time").unwrap();
        {let tx=c.transaction().unwrap();preview(&tx,&payload).unwrap();tx.commit().unwrap();}assert_eq!(state(&c).unwrap(),before);
        c.execute("UPDATE annual_goals SET direction_id=NULL WHERE id='g'",[]).unwrap();assert_eq!(record(&c,"time",made["records"][0]["id"].as_str().unwrap()).unwrap()["direction_id"],"dir");
    }

    #[test] fn shared_schedule_edits_do_not_generate_or_rewrite_daily_records(){
        let mut c=time_database();
        let before_time=rows(&c,"SELECT * FROM time_entries ORDER BY id",&[]).unwrap();let before_plan=rows(&c,"SELECT * FROM plan_blocks ORDER BY id",&[]).unwrap();
        let result=save_times(&mut c,"standard",json!([{"kind":"dailyPlan","action":"create","values":{"title":"睡觉","start_min":0,"end_min":420}}])).unwrap();
        let id=result["records"][0]["id"].clone();
        save_times(&mut c,"standard-edit",json!([{"kind":"dailyPlan","action":"update","id":id,"values":{"end_min":480}}])).unwrap();
        assert_eq!(record(&c,"dailyPlan",id.as_str().unwrap()).unwrap()["end_min"],480);
        assert_eq!(rows(&c,"SELECT * FROM time_entries ORDER BY id",&[]).unwrap(),before_time);assert_eq!(rows(&c,"SELECT * FROM plan_blocks ORDER BY id",&[]).unwrap(),before_plan);
    }

}
