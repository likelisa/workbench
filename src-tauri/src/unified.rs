//! Unified identities and independent execution placements. Legacy tables remain recoverable.
use rusqlite::{Connection, params};
use serde_json::{Value, json};
use super::ai_service::{err, text};

pub fn schema(c: &Connection) -> Result<(), String> {
    c.execute_batch("CREATE TABLE IF NOT EXISTS items (
      id TEXT PRIMARY KEY,title TEXT NOT NULL,type TEXT NOT NULL DEFAULT 'todo' CHECK(type IN('requirement','bug','todo','idea')),
      status TEXT NOT NULL DEFAULT 'todo' CHECK(status IN('todo','doing','done','missed')),priority TEXT DEFAULT 'P2' CHECK(priority IS NULL OR priority IN('P0','P1','P2','P3')),
      description TEXT NOT NULL DEFAULT '',project_id TEXT REFERENCES long_projects(id) ON DELETE SET NULL,parent_id TEXT REFERENCES items(id) ON DELETE SET NULL,
      annual_goal_id TEXT REFERENCES annual_goals(id) ON DELETE SET NULL,cycle_goal_id TEXT REFERENCES cycle_goals(id) ON DELETE SET NULL,
      cycle_id TEXT REFERENCES cycles(id) ON DELETE SET NULL,level TEXT NOT NULL DEFAULT 'backlog' CHECK(level IN('backlog','cycle','week','day')),
      version_id TEXT REFERENCES project_versions(id) ON DELETE SET NULL,milestone_id TEXT REFERENCES project_stages(id) ON DELETE SET NULL,created_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS project_versions(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES long_projects(id) ON DELETE CASCADE,title TEXT NOT NULL,description TEXT NOT NULL DEFAULT '',created_at TEXT NOT NULL);
      CREATE TABLE IF NOT EXISTS placements(id TEXT PRIMARY KEY,item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,kind TEXT NOT NULL CHECK(kind IN('day','week')),
      date TEXT,week_start TEXT,cycle_id TEXT REFERENCES cycles(id) ON DELETE SET NULL,week_no INTEGER CHECK(week_no BETWEEN 1 AND 12),created_at TEXT NOT NULL,
      CHECK((kind='day' AND date IS NOT NULL AND week_start IS NULL) OR (kind='week' AND week_start IS NOT NULL AND date IS NULL)));
      CREATE UNIQUE INDEX IF NOT EXISTS placement_day ON placements(item_id,date) WHERE kind='day';
      CREATE UNIQUE INDEX IF NOT EXISTS placement_week ON placements(item_id,week_start) WHERE kind='week';
      CREATE TABLE IF NOT EXISTS task_project_items(task_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,project_item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,PRIMARY KEY(task_id,project_item_id),CHECK(task_id<>project_item_id));
      CREATE INDEX IF NOT EXISTS project_item_references ON task_project_items(project_item_id);
      CREATE TABLE IF NOT EXISTS item_projects(item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,project_id TEXT NOT NULL REFERENCES long_projects(id) ON DELETE CASCADE,PRIMARY KEY(item_id,project_id));
      CREATE TABLE IF NOT EXISTS item_legacy_map(kind TEXT NOT NULL,old_id TEXT NOT NULL,item_id TEXT NOT NULL REFERENCES items(id) ON DELETE CASCADE,PRIMARY KEY(kind,old_id));
      CREATE TABLE IF NOT EXISTS unified_migrations(name TEXT PRIMARY KEY);").map_err(err)
}
fn has(c:&Connection,t:&str,f:&str)->Result<bool,String>{c.query_row(&format!("SELECT EXISTS(SELECT 1 FROM pragma_table_info('{t}') WHERE name=?)"),[f],|r|r.get(0)).map_err(err)}
pub fn migrate(c:&mut Connection)->Result<(),String>{
    schema(c)?;
    let done:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM unified_migrations WHERE name='items-v1')",[],|r|r.get(0)).map_err(err)?;
    c.execute_batch("DROP TRIGGER IF EXISTS create_time_entry_for_timed_day_task; DROP TRIGGER IF EXISTS sync_time_entry_parent_on_day_task_change; DROP TRIGGER IF EXISTS inherit_day_project_insert; DROP TRIGGER IF EXISTS inherit_day_project_update; DROP TRIGGER IF EXISTS inherit_week_project_update; DROP TRIGGER IF EXISTS sync_explicit_day_project;
      CREATE TRIGGER inherit_day_project_insert AFTER INSERT ON day_tasks WHEN NEW.assignment_id IS NOT NULL BEGIN UPDATE day_tasks SET long_project_id=(SELECT long_project_id FROM week_assignments WHERE id=NEW.assignment_id) WHERE id=NEW.id; END;
      CREATE TRIGGER inherit_day_project_update AFTER UPDATE OF assignment_id ON day_tasks WHEN NEW.assignment_id IS NOT OLD.assignment_id AND NEW.assignment_id IS NOT NULL BEGIN UPDATE day_tasks SET long_project_id=(SELECT long_project_id FROM week_assignments WHERE id=NEW.assignment_id) WHERE id=NEW.id; END;
      CREATE TRIGGER inherit_week_project_update AFTER UPDATE OF long_project_id ON week_assignments WHEN NEW.long_project_id IS NOT OLD.long_project_id BEGIN UPDATE day_tasks SET long_project_id=NEW.long_project_id WHERE assignment_id=NEW.id; END;").map_err(err)?;
    if done {repair_cycle_hierarchy(c)?;restore_scheduled_week_items(c)?;return migrate_time_editor(c);}
    let tx=c.transaction().map_err(err)?;
    let old_count:i64=tx.query_row("SELECT (SELECT COUNT(*) FROM tasks)+(SELECT COUNT(*) FROM week_assignments)+(SELECT COUNT(*) FROM day_tasks)+(SELECT COUNT(*) FROM project_plans)",[],|r|r.get(0)).map_err(err)?;
    let old_time:(i64,f64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(julianday(end_at)-julianday(start_at)),0) FROM time_entries",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
    for (table, fields) in [("time_entries",vec![("item_id","TEXT REFERENCES items(id) ON DELETE SET NULL"),("work_kind","TEXT NOT NULL DEFAULT 'legacy'"),("life_category","TEXT"),("primary_goal_id","TEXT REFERENCES annual_goals(id) ON DELETE SET NULL"),("category_manual","INTEGER NOT NULL DEFAULT 0"),("source_plan_id","TEXT")]),("plan_blocks",vec![("item_id","TEXT REFERENCES items(id) ON DELETE SET NULL"),("schedule_kind","TEXT NOT NULL DEFAULT 'flexible'")])] {
        for (field,definition) in fields {if !has(&tx,table,field)?{tx.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {field} {definition};")).map_err(err)?;}}
    }
    tx.execute_batch("DROP TRIGGER IF EXISTS create_time_entry_for_timed_day_task; DROP TRIGGER IF EXISTS sync_time_entry_parent_on_day_task_change; DROP TRIGGER IF EXISTS sync_time_long_project_on_day_change;
      INSERT INTO items(id,title,status,project_id,annual_goal_id,cycle_goal_id,cycle_id,level,created_at)
      SELECT 'task:'||t.id,t.title,t.status,(SELECT MIN(project_id) FROM task_projects WHERE task_id=t.id HAVING COUNT(*)=1),t.annual_goal_id,t.cycle_goal_id,t.cycle_id,'cycle',t.created_at FROM tasks t;
      INSERT INTO items(id,title,status,priority,project_id,parent_id,annual_goal_id,cycle_goal_id,cycle_id,level,description,created_at)
      SELECT 'week:'||w.id,w.commitment,w.status,w.priority,w.long_project_id,'task:'||w.task_id,t.annual_goal_id,t.cycle_goal_id,w.cycle_id,'week',w.review,w.created_at FROM week_assignments w LEFT JOIN tasks t ON t.id=w.task_id;
      INSERT INTO items(id,title,status,priority,project_id,parent_id,annual_goal_id,cycle_id,level,created_at)
      SELECT 'day:'||d.id,d.title,d.status,d.priority,d.long_project_id,CASE WHEN d.assignment_id IS NOT NULL THEN 'week:'||d.assignment_id WHEN d.task_id IS NOT NULL THEN 'task:'||d.task_id END,t.annual_goal_id,t.cycle_id,'day',d.created_at FROM day_tasks d LEFT JOIN tasks t ON t.id=d.task_id;
      INSERT INTO items(id,title,status,description,project_id,level,milestone_id,created_at) SELECT 'plan:'||id,title,CASE WHEN completed=1 THEN 'done' ELSE 'todo' END,description,project_id,'backlog',stage_id,created_at FROM project_plans;
      INSERT INTO item_legacy_map SELECT 'task',id,'task:'||id FROM tasks;
      INSERT INTO item_legacy_map SELECT 'assignment',id,'week:'||id FROM week_assignments;
      INSERT INTO item_legacy_map SELECT 'dayTask',id,'day:'||id FROM day_tasks;
      INSERT INTO item_legacy_map SELECT 'projectPlan',id,'plan:'||id FROM project_plans;
      INSERT INTO placements(id,item_id,kind,date,created_at) SELECT 'day:'||id,'day:'||id,'day',date,created_at FROM day_tasks;
      INSERT INTO placements(id,item_id,kind,week_start,cycle_id,week_no,created_at)
      SELECT 'week:'||w.id||':'||a.week_no,'week:'||w.id,'week',date(c.start_date,'+'||((a.week_no-1)*7)||' days'),w.cycle_id,a.week_no,w.created_at FROM week_assignments w JOIN assignment_weeks a ON a.assignment_id=w.id JOIN cycles c ON c.id=w.cycle_id;
      UPDATE time_entries SET item_id=CASE WHEN day_task_id IS NOT NULL THEN 'day:'||day_task_id WHEN task_id IS NOT NULL THEN 'task:'||task_id END,
        primary_goal_id=(SELECT annual_goal_id FROM tasks WHERE id=time_entries.task_id),
        work_kind=CASE WHEN category_id IN(SELECT id FROM categories WHERE kind='goal') THEN 'work' WHEN category_id IN(SELECT id FROM categories WHERE kind='fixed') THEN 'life' ELSE 'legacy' END,
        life_category=(SELECT name FROM categories WHERE id=time_entries.category_id),category_manual=1;
      UPDATE plan_blocks SET item_id='day:'||day_task_id WHERE day_task_id IS NOT NULL;
      INSERT INTO plan_blocks(id,date,start_min,end_min,title,day_task_id,item_id,created_at)
      SELECT 'scheduled:'||id,date,CAST(strftime('%H',scheduled_start_at,'+8 hours') AS INTEGER)*60+CAST(strftime('%M',scheduled_start_at,'+8 hours') AS INTEGER),CASE WHEN date(scheduled_end_at,'+8 hours')>date THEN 1440 ELSE CAST(strftime('%H',scheduled_end_at,'+8 hours') AS INTEGER)*60+CAST(strftime('%M',scheduled_end_at,'+8 hours') AS INTEGER) END,title,id,'day:'||id,created_at FROM day_tasks WHERE scheduled_start_at IS NOT NULL AND NOT EXISTS(SELECT 1 FROM plan_blocks p WHERE p.day_task_id=day_tasks.id);").map_err(err)?;
    tx.execute_batch("UPDATE time_entries SET source_plan_id='scheduled:'||day_task_id WHERE id=day_task_id||':time' AND EXISTS(SELECT 1 FROM plan_blocks WHERE id='scheduled:'||time_entries.day_task_id);").map_err(err)?;
    tx.execute_batch("INSERT OR IGNORE INTO item_projects SELECT 'task:'||task_id,project_id FROM task_projects; INSERT OR IGNORE INTO item_projects SELECT id,project_id FROM items WHERE project_id IS NOT NULL;").map_err(err)?;
    tx.execute_batch("WITH RECURSIVE slots(id,day,finish,start_at,end_at,title) AS (
      SELECT id,date(date,'+1 day'),date(scheduled_end_at,'+8 hours'),scheduled_start_at,scheduled_end_at,title FROM day_tasks WHERE scheduled_start_at IS NOT NULL AND date(scheduled_end_at,'+8 hours')>date
      UNION ALL SELECT id,date(day,'+1 day'),finish,start_at,end_at,title FROM slots WHERE day<finish)
      INSERT INTO plan_blocks(id,date,start_min,end_min,title,day_task_id,item_id,created_at)
      SELECT 'scheduled:'||id||':'||day,day,0,CASE WHEN day<finish THEN 1440 ELSE CAST(strftime('%H',end_at,'+8 hours') AS INTEGER)*60+CAST(strftime('%M',end_at,'+8 hours') AS INTEGER) END,title,id,'day:'||id,'' FROM slots WHERE day<finish OR strftime('%H:%M',end_at,'+8 hours')>'00:00';").map_err(err)?;
    // Compatibility writes from old assistants stay visible; identity edits synchronize without parent completion.
    for (table,kind,prefix,title) in [("tasks","task","task","title"),("week_assignments","assignment","week","commitment"),("day_tasks","dayTask","day","title"),("project_plans","projectPlan","plan","title")] {
        let status=if table=="project_plans" {"CASE WHEN NEW.completed=1 THEN 'done' ELSE 'todo' END"}else{"NEW.status"};
        let fields=if table=="project_plans" {"completed"}else{"status"};
        let update=if table=="project_plans" {"completed=CASE WHEN NEW.status='done' THEN 1 ELSE 0 END"}else{"status=NEW.status"};
        tx.execute_batch(&format!("CREATE TRIGGER unified_{prefix}_insert AFTER INSERT ON {table} BEGIN INSERT OR IGNORE INTO items(id,title,status,created_at) VALUES('{prefix}:'||NEW.id,NEW.{title},{status},NEW.created_at); INSERT OR IGNORE INTO item_legacy_map VALUES('{kind}',NEW.id,'{prefix}:'||NEW.id); END;
          CREATE TRIGGER unified_{prefix}_update AFTER UPDATE OF {title},{fields} ON {table} BEGIN UPDATE items SET title=NEW.{title},status={status} WHERE id='{prefix}:'||NEW.id AND (title IS NOT NEW.{title} OR status IS NOT {status}); END;
          CREATE TRIGGER unified_{prefix}_reverse AFTER UPDATE OF title,status ON items WHEN NEW.id LIKE '{prefix}:%' BEGIN UPDATE {table} SET {title}=NEW.title,{update} WHERE id=substr(NEW.id,{}) AND ({title} IS NOT NEW.title OR {} IS NOT {}); END;",prefix.len()+2,fields,if table=="project_plans" {"CASE WHEN NEW.status='done' THEN 1 ELSE 0 END"}else{"NEW.status"})).map_err(err)?;
    }
    for (table,prefix,extra) in [("tasks","task","project_id=(SELECT MIN(project_id) FROM task_projects WHERE task_id=NEW.id HAVING COUNT(*)=1),annual_goal_id=NEW.annual_goal_id,cycle_goal_id=NEW.cycle_goal_id,cycle_id=NEW.cycle_id,level='cycle'"),
      ("week_assignments","week","priority=NEW.priority,project_id=NEW.long_project_id,parent_id='task:'||NEW.task_id,annual_goal_id=(SELECT annual_goal_id FROM tasks WHERE id=NEW.task_id),cycle_id=NEW.cycle_id,level='week'"),
      ("day_tasks","day","priority=NEW.priority,project_id=NEW.long_project_id,parent_id=CASE WHEN NEW.assignment_id IS NOT NULL THEN 'week:'||NEW.assignment_id WHEN NEW.task_id IS NOT NULL THEN 'task:'||NEW.task_id END,annual_goal_id=(SELECT annual_goal_id FROM tasks WHERE id=NEW.task_id),level='day'"),
      ("project_plans","plan","project_id=NEW.project_id,description=NEW.description,milestone_id=NEW.stage_id")]
    { for event in ["INSERT","UPDATE"] {let event_sql=if event=="INSERT" {"INSERT".to_string()} else {format!("UPDATE OF {}",match table {"tasks"=>"annual_goal_id,cycle_goal_id,cycle_id","week_assignments"=>"priority,long_project_id,task_id,cycle_id","day_tasks"=>"priority,long_project_id,assignment_id,task_id",_=>"project_id,description,stage_id"})};tx.execute_batch(&format!("CREATE TRIGGER unified_meta_{prefix}_{event} AFTER {event_sql} ON {table} BEGIN UPDATE items SET {extra} WHERE id='{prefix}:'||NEW.id; END;")).map_err(err)?;}}
    tx.execute_batch("CREATE TRIGGER unified_map_inherit AFTER INSERT ON item_legacy_map BEGIN INSERT OR IGNORE INTO placements(id,item_id,kind,date,created_at) SELECT 'day:'||id,NEW.item_id,'day',date,created_at FROM day_tasks WHERE id=NEW.old_id AND NEW.kind='dayTask';
      UPDATE items SET level='cycle',cycle_id=(SELECT cycle_id FROM tasks WHERE id=NEW.old_id),annual_goal_id=(SELECT annual_goal_id FROM tasks WHERE id=NEW.old_id) WHERE id=NEW.item_id AND NEW.kind='task';
      UPDATE items SET level='week',priority=(SELECT priority FROM week_assignments WHERE id=NEW.old_id),project_id=(SELECT long_project_id FROM week_assignments WHERE id=NEW.old_id),parent_id=(SELECT 'task:'||task_id FROM week_assignments WHERE id=NEW.old_id),cycle_id=(SELECT cycle_id FROM week_assignments WHERE id=NEW.old_id) WHERE id=NEW.item_id AND NEW.kind='assignment';
      UPDATE items SET level='day',priority=(SELECT priority FROM day_tasks WHERE id=NEW.old_id),project_id=(SELECT long_project_id FROM day_tasks WHERE id=NEW.old_id),parent_id=(SELECT CASE WHEN assignment_id IS NOT NULL THEN 'week:'||assignment_id WHEN task_id IS NOT NULL THEN 'task:'||task_id END FROM day_tasks WHERE id=NEW.old_id) WHERE id=NEW.item_id AND NEW.kind='dayTask';
      UPDATE items SET project_id=(SELECT project_id FROM project_plans WHERE id=NEW.old_id),description=(SELECT description FROM project_plans WHERE id=NEW.old_id) WHERE id=NEW.item_id AND NEW.kind='projectPlan'; END;").map_err(err)?;
    tx.execute_batch("
      CREATE TRIGGER unified_item_project_insert AFTER INSERT ON items WHEN NEW.project_id IS NOT NULL BEGIN INSERT OR IGNORE INTO item_projects VALUES(NEW.id,NEW.project_id); END;
      CREATE TRIGGER unified_item_project_update AFTER UPDATE OF project_id ON items WHEN NEW.project_id IS NOT OLD.project_id BEGIN DELETE FROM item_projects WHERE item_id=NEW.id; INSERT OR IGNORE INTO item_projects SELECT NEW.id,NEW.project_id WHERE NEW.project_id IS NOT NULL; END;
      CREATE TRIGGER unified_day_date AFTER UPDATE OF date ON day_tasks BEGIN UPDATE placements SET date=NEW.date WHERE id='day:'||NEW.id; END;
      CREATE TRIGGER unified_day_date_reverse AFTER UPDATE OF date ON placements WHEN NEW.kind='day' AND NEW.id LIKE 'day:%' BEGIN INSERT OR IGNORE INTO recurrence_exceptions SELECT recurrence_id,date FROM day_tasks WHERE id=substr(NEW.id,5) AND recurrence_id IS NOT NULL AND date IS NOT NEW.date; UPDATE day_tasks SET date=NEW.date,recurrence_id=NULL WHERE id=substr(NEW.id,5) AND date IS NOT NEW.date; END;
      CREATE TRIGGER unified_week_placement AFTER INSERT ON assignment_weeks BEGIN INSERT OR IGNORE INTO placements(id,item_id,kind,week_start,cycle_id,week_no,created_at) SELECT 'week:'||w.id||':'||NEW.week_no,'week:'||w.id,'week',date(c.start_date,'+'||((NEW.week_no-1)*7)||' days'),w.cycle_id,NEW.week_no,w.created_at FROM week_assignments w JOIN cycles c ON c.id=w.cycle_id WHERE w.id=NEW.assignment_id; END;
      CREATE TRIGGER unified_week_unschedule AFTER DELETE ON assignment_weeks BEGIN DELETE FROM placements WHERE id='week:'||OLD.assignment_id||':'||OLD.week_no; END;
      CREATE UNIQUE INDEX IF NOT EXISTS unique_actual_plan_source ON time_entries(source_plan_id) WHERE source_plan_id IS NOT NULL;
      INSERT INTO unified_migrations VALUES('items-v1');").map_err(err)?;
    for (table,prefix) in [("tasks","task"),("week_assignments","week"),("day_tasks","day"),("project_plans","plan")] {tx.execute_batch(&format!("CREATE TRIGGER unified_{prefix}_delete AFTER DELETE ON {table} BEGIN DELETE FROM items WHERE id='{prefix}:'||OLD.id; END;")).map_err(err)?;}
    for event in ["INSERT","DELETE"] {let r=if event=="INSERT"{"NEW"}else{"OLD"};tx.execute_batch(&format!("CREATE TRIGGER unified_task_projects_{event} AFTER {event} ON task_projects BEGIN
      UPDATE items SET project_id=(SELECT MIN(project_id) FROM task_projects WHERE task_id={r}.task_id HAVING COUNT(*)=1) WHERE id='task:'||{r}.task_id;
      DELETE FROM item_projects WHERE item_id='task:'||{r}.task_id;
      INSERT OR IGNORE INTO item_projects SELECT 'task:'||task_id,project_id FROM task_projects WHERE task_id={r}.task_id; END;")).map_err(err)?;}
    for (prefix,table) in [("week","week_assignments"),("day","day_tasks")] {tx.execute_batch(&format!("CREATE TRIGGER unified_{prefix}_priority_reverse AFTER UPDATE OF priority ON items WHEN NEW.id LIKE '{prefix}:%' BEGIN UPDATE {table} SET priority=NEW.priority WHERE id=substr(NEW.id,{}) AND priority IS NOT NEW.priority; END;",prefix.len()+2)).map_err(err)?;}
    let new_count:i64=tx.query_row("SELECT COUNT(*) FROM item_legacy_map",[],|r|r.get(0)).map_err(err)?;
    let new_time:(i64,f64)=tx.query_row("SELECT COUNT(*),COALESCE(SUM(julianday(end_at)-julianday(start_at)),0) FROM time_entries",[],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
    if old_count!=new_count || old_time!=new_time {return Err("事项迁移数量或实际时间校验失败，已回滚".into());}
    let bad:i64=tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get(0)).map_err(err)?;
    if bad>0 {return Err("事项迁移存在无效关联，已回滚".into());}
    tx.commit().map_err(err)?;
    repair_cycle_hierarchy(c)?;
    restore_scheduled_week_items(c)?;
    migrate_time_editor(c)
}
// Legacy phase goals retain their backing identities. Scheduled assignments stay weekly tasks.
// Keep the backing identities for legacy links and historical time attribution.
fn repair_cycle_hierarchy(c:&mut Connection)->Result<(),String>{
    let done:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM unified_migrations WHERE name='cycle-hierarchy-v2')",[],|r|r.get(0)).map_err(err)?;
    if done{return Ok(());}
    let tx=c.transaction().map_err(err)?;
    let identities=super::ai_service::rows(&tx,"SELECT id,title,status,priority FROM items ORDER BY id",&[])?;
    let placements=super::ai_service::rows(&tx,"SELECT * FROM placements ORDER BY id",&[])?;
    let times=super::ai_service::rows(&tx,"SELECT * FROM time_entries ORDER BY id",&[])?;
    if !has(&tx,"cycle_goals","source_item_id")? {tx.execute_batch("ALTER TABLE cycle_goals ADD COLUMN source_item_id TEXT REFERENCES items(id) ON DELETE CASCADE;").map_err(err)?;}
    tx.execute_batch("CREATE UNIQUE INDEX IF NOT EXISTS cycle_goal_source ON cycle_goals(source_item_id) WHERE source_item_id IS NOT NULL;
      INSERT INTO cycle_goals(id,cycle_id,annual_goal_id,title,created_at,source_item_id)
      SELECT 'legacy-goal:'||m.old_id,i.cycle_id,i.annual_goal_id,i.title,i.created_at,i.id FROM items i JOIN item_legacy_map m ON m.item_id=i.id AND m.kind='task'
      WHERE i.level='cycle' AND i.cycle_goal_id IS NULL AND i.cycle_id IS NOT NULL AND i.annual_goal_id IS NOT NULL;
      UPDATE items SET cycle_goal_id=(SELECT id FROM cycle_goals WHERE source_item_id=items.id) WHERE id IN(SELECT source_item_id FROM cycle_goals WHERE source_item_id IS NOT NULL);
      UPDATE items SET level=CASE WHEN EXISTS(SELECT 1 FROM placements p WHERE p.item_id=items.id AND p.kind='week') THEN 'week' ELSE 'cycle' END,cycle_goal_id=(SELECT id FROM cycle_goals WHERE source_item_id=items.parent_id),annual_goal_id=(SELECT annual_goal_id FROM cycle_goals WHERE source_item_id=items.parent_id)
      WHERE id IN(SELECT item_id FROM item_legacy_map WHERE kind='assignment') AND parent_id IN(SELECT source_item_id FROM cycle_goals WHERE source_item_id IS NOT NULL);
      CREATE TRIGGER cycle_goal_source_title AFTER UPDATE OF title ON items WHEN EXISTS(SELECT 1 FROM cycle_goals WHERE source_item_id=NEW.id AND title IS NOT NEW.title) BEGIN UPDATE cycle_goals SET title=NEW.title WHERE source_item_id=NEW.id; END;
      CREATE TRIGGER cycle_goal_title_source AFTER UPDATE OF title ON cycle_goals WHEN NEW.source_item_id IS NOT NULL BEGIN UPDATE items SET title=NEW.title WHERE id=NEW.source_item_id AND title IS NOT NEW.title; END;
      CREATE TRIGGER cycle_goal_source_delete AFTER DELETE ON items BEGIN DELETE FROM cycle_goals WHERE source_item_id=OLD.id; END;
      CREATE TRIGGER cycle_goal_delete_source AFTER DELETE ON cycle_goals WHEN OLD.source_item_id IS NOT NULL BEGIN DELETE FROM items WHERE id=OLD.source_item_id; END;").map_err(err)?;
    let goal="COALESCE((SELECT id FROM cycle_goals WHERE source_item_id='task:'||NEW.task_id),(SELECT cycle_goal_id FROM tasks WHERE id=NEW.task_id))";
    let level="'week'";
    tx.execute_batch(&format!("DROP TRIGGER IF EXISTS unified_meta_week_INSERT; CREATE TRIGGER unified_meta_week_INSERT AFTER INSERT ON week_assignments BEGIN UPDATE items SET priority=NEW.priority,project_id=NEW.long_project_id,parent_id='task:'||NEW.task_id,annual_goal_id=(SELECT annual_goal_id FROM tasks WHERE id=NEW.task_id),cycle_id=NEW.cycle_id,cycle_goal_id={goal},level={level} WHERE id='week:'||NEW.id; END;
      DROP TRIGGER IF EXISTS unified_meta_week_UPDATE; CREATE TRIGGER unified_meta_week_UPDATE AFTER UPDATE OF priority,long_project_id,task_id,cycle_id ON week_assignments BEGIN UPDATE items SET priority=NEW.priority,
      project_id=CASE WHEN NEW.long_project_id IS NOT OLD.long_project_id THEN NEW.long_project_id ELSE project_id END,
      parent_id=CASE WHEN NEW.task_id IS NOT OLD.task_id THEN 'task:'||NEW.task_id ELSE parent_id END,
      annual_goal_id=CASE WHEN NEW.task_id IS NOT OLD.task_id THEN (SELECT annual_goal_id FROM tasks WHERE id=NEW.task_id) ELSE annual_goal_id END,
      cycle_id=CASE WHEN NEW.cycle_id IS NOT OLD.cycle_id THEN NEW.cycle_id ELSE cycle_id END,
      cycle_goal_id=CASE WHEN NEW.task_id IS NOT OLD.task_id THEN {goal} ELSE cycle_goal_id END,
      level=CASE WHEN NEW.task_id IS NOT OLD.task_id THEN {level} ELSE level END WHERE id='week:'||NEW.id; END;")).map_err(err)?;
    let trigger:String=tx.query_row("SELECT sql FROM sqlite_master WHERE type='trigger' AND name='unified_map_inherit'",[],|r|r.get(0)).map_err(err)?;
    let trigger=trigger.replace("level='week',priority=", "level='week',cycle_goal_id=COALESCE((SELECT id FROM cycle_goals WHERE source_item_id=(SELECT 'task:'||task_id FROM week_assignments WHERE id=NEW.old_id)),(SELECT t.cycle_goal_id FROM tasks t JOIN week_assignments w ON w.task_id=t.id WHERE w.id=NEW.old_id)),priority=");
    tx.execute_batch("DROP TRIGGER unified_map_inherit;").map_err(err)?;tx.execute_batch(&trigger).map_err(err)?;
    if identities!=super::ai_service::rows(&tx,"SELECT id,title,status,priority FROM items ORDER BY id",&[])? || placements!=super::ai_service::rows(&tx,"SELECT * FROM placements ORDER BY id",&[])? || times!=super::ai_service::rows(&tx,"SELECT * FROM time_entries ORDER BY id",&[])? {return Err("规划层级修正校验失败，已回滚".into());}
    if tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).map_err(err)?>0 {return Err("规划层级修正关联校验失败，已回滚".into());}
    tx.execute("INSERT INTO unified_migrations VALUES('cycle-hierarchy-v2')",[]).map_err(err)?;
    tx.commit().map_err(err)
}
// Repair only migrated, scheduled assignments. Unscheduled cycle todos and user edits remain intact.
fn restore_scheduled_week_items(c:&mut Connection)->Result<(),String>{
    let done:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM unified_migrations WHERE name='scheduled-week-level-v3')",[],|r|r.get(0)).map_err(err)?;
    if done{return Ok(());}
    let tx=c.transaction().map_err(err)?;
    let preserved=super::ai_service::rows(&tx,"SELECT id,title,type,status,priority,description,project_id,parent_id,annual_goal_id,cycle_goal_id,cycle_id,version_id,milestone_id,created_at FROM items ORDER BY id",&[])?;
    let placements=super::ai_service::rows(&tx,"SELECT * FROM placements ORDER BY id",&[])?;
    let times=super::ai_service::rows(&tx,"SELECT * FROM time_entries ORDER BY id",&[])?;
    tx.execute_batch("UPDATE items SET level='week' WHERE level='cycle'
      AND id IN(SELECT item_id FROM item_legacy_map WHERE kind='assignment')
      AND cycle_goal_id IN(SELECT id FROM cycle_goals WHERE source_item_id IS NOT NULL)
      AND EXISTS(SELECT 1 FROM placements p WHERE p.item_id=items.id AND p.kind='week');").map_err(err)?;
    // Old installations may still classify legacy weekly writes as cycle todos.
    for name in ["unified_meta_week_INSERT","unified_meta_week_UPDATE","unified_map_inherit"] {
        let sql:String=tx.query_row("SELECT sql FROM sqlite_master WHERE type='trigger' AND name=?",[name],|r|r.get(0)).map_err(err)?;
        let fixed=sql.replace("CASE WHEN EXISTS(SELECT 1 FROM cycle_goals WHERE source_item_id='task:'||NEW.task_id) THEN 'cycle' ELSE 'week' END", "'week'")
            .replace("CASE WHEN EXISTS(SELECT 1 FROM cycle_goals WHERE source_item_id=(SELECT 'task:'||task_id FROM week_assignments WHERE id=NEW.old_id)) THEN 'cycle' ELSE 'week' END", "'week'");
        if fixed!=sql {tx.execute_batch(&format!("DROP TRIGGER {name};")).map_err(err)?;tx.execute_batch(&fixed).map_err(err)?;}
    }
    if preserved!=super::ai_service::rows(&tx,"SELECT id,title,type,status,priority,description,project_id,parent_id,annual_goal_id,cycle_goal_id,cycle_id,version_id,milestone_id,created_at FROM items ORDER BY id",&[])? || placements!=super::ai_service::rows(&tx,"SELECT * FROM placements ORDER BY id",&[])? || times!=super::ai_service::rows(&tx,"SELECT * FROM time_entries ORDER BY id",&[])? {return Err("周任务恢复校验失败，已回滚".into());}
    if tx.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).map_err(err)?>0 {return Err("周任务恢复关联校验失败，已回滚".into());}
    tx.execute("INSERT INTO unified_migrations VALUES('scheduled-week-level-v3')",[]).map_err(err)?;
    tx.commit().map_err(err)
}
// Additive time metadata; existing identities, categories and durations are untouched.
fn migrate_time_editor(c:&mut Connection)->Result<(),String>{
    let done:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM unified_migrations WHERE name='time-editor-v1')",[],|r|r.get(0)).map_err(err)?;
    if done{return Ok(());}
    let tx=c.transaction().map_err(err)?;
    for (table,fields) in [("plan_blocks",vec![("category_id","TEXT REFERENCES categories(id)"),("work_kind","TEXT"),("life_category","TEXT"),("category_manual","INTEGER NOT NULL DEFAULT 0"),("long_project_id","TEXT REFERENCES long_projects(id) ON DELETE SET NULL"),("primary_goal_id","TEXT REFERENCES annual_goals(id) ON DELETE SET NULL"),("direction_id","TEXT REFERENCES directions(id) ON DELETE SET NULL")]),("time_entries",vec![("direction_id","TEXT REFERENCES directions(id) ON DELETE SET NULL")])] {
        for (field,definition) in fields {if !has(&tx,table,field)?{tx.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {field} {definition};")).map_err(err)?;}}
    }
    tx.execute_batch("INSERT OR IGNORE INTO categories(id,name,color,kind,active,created_at) VALUES('system:work','工作','#67D649','fixed',1,'');
      UPDATE time_entries SET direction_id=(SELECT direction_id FROM annual_goals WHERE id=primary_goal_id) WHERE direction_id IS NULL;
      INSERT INTO unified_migrations VALUES('time-editor-v1');").map_err(err)?;
    tx.commit().map_err(err)
}
#[tauri::command]
pub fn init_unified(app:tauri::AppHandle)->Result<(),String>{
    let dir=super::config_dir(&app)?;let path=dir.join("workbench.db");let mut c=Connection::open(&path).map_err(err)?;
    c.execute_batch("PRAGMA foreign_keys=ON").map_err(err)?;c.busy_timeout(std::time::Duration::from_secs(5)).map_err(err)?;
    let has_links:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='task_project_items')",[],|r|r.get(0)).map_err(err)?;
    if !has_links {super::online_backup(&path,&dir.join(format!("before-project-item-links-{}.db",chrono::Utc::now().timestamp_millis())))?;}
    schema(&c)?;
    let done:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM unified_migrations WHERE name='items-v1') AND EXISTS(SELECT 1 FROM unified_migrations WHERE name='cycle-hierarchy-v2') AND EXISTS(SELECT 1 FROM unified_migrations WHERE name='scheduled-week-level-v3') AND EXISTS(SELECT 1 FROM unified_migrations WHERE name='time-editor-v1')",[],|r|r.get(0)).map_err(err)?;
    if !done {super::online_backup(&path,&dir.join(format!("before-workbench-upgrade-{}.db",chrono::Utc::now().timestamp_millis())))?;}
    migrate(&mut c)
}

// Apply current planning defaults only when creating or changing a day's associations.
// Completing or renaming a historical day must preserve its original hierarchy.
pub fn normalize_week_item(c:&Connection,v:&mut Value,id:&str)->Result<(),String>{
    if text(v,"level")!="week" {return Ok(());}
    let existing=super::ai_service::rows(c,"SELECT * FROM items WHERE id=?",&[&id])?.into_iter().next();
    if existing.as_ref().is_some_and(|old|["level","parent_id","cycle_id","annual_goal_id","cycle_goal_id"].iter().all(|key|old[*key]==v[*key])) {return Ok(());}

    if let Some(parent)=v["parent_id"].as_str().filter(|s|!s.is_empty()).map(str::to_owned) {
        let rows=super::ai_service::rows(c,"SELECT i.* FROM items i WHERE id=? AND level='cycle' AND NOT EXISTS(SELECT 1 FROM cycle_goals g WHERE g.source_item_id=i.id)",&[&parent])?;
        rows.first().ok_or("周任务只能关联本轮待办")?;
        let chain=super::ai_service::rows(c,"WITH RECURSIVE parents AS (SELECT *,0 depth FROM items WHERE id=? UNION ALL SELECT i.*,p.depth+1 FROM items i JOIN parents p ON i.id=p.parent_id WHERE p.depth<100) SELECT * FROM parents ORDER BY depth",&[&parent])?;
        for key in ["annual_goal_id","cycle_goal_id","cycle_id"] {v[key]=json!(chain.iter().find_map(|i|i[key].as_str()));}
    }else{v["parent_id"]=Value::Null;v["annual_goal_id"]=Value::Null;v["cycle_goal_id"]=Value::Null;}
    Ok(())
}
pub fn normalize_day_item(c:&Connection,v:&mut Value,id:&str,today:chrono::NaiveDate)->Result<(),String>{
    use chrono::{Datelike,Duration};
    use rusqlite::OptionalExtension;
    if text(v,"level")!="day" {return Ok(());}
    let existing=super::ai_service::rows(c,"SELECT * FROM items WHERE id=?",&[&id])?.into_iter().next();
    if existing.as_ref().is_some_and(|old|["level","parent_id","cycle_id","project_id","annual_goal_id","cycle_goal_id"].iter().all(|key|old[*key]==v[*key])) {return Ok(());}
    let cycle:Option<String>=c.query_row("SELECT id FROM cycles WHERE start_date<=?1 AND date(start_date,'+83 days')>=?1 ORDER BY start_date DESC,id LIMIT 1",[today.to_string()],|r|r.get(0)).optional().map_err(err)?;
    v["cycle_id"]=json!(cycle);
    let parent=v["parent_id"].as_str().filter(|p|!p.is_empty()).map(str::to_owned);
    if let Some(parent)=parent {
        let start=today-Duration::days(today.weekday().num_days_from_monday() as i64);
        let valid:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM placements p JOIN items i ON i.id=p.item_id WHERE p.item_id=?1 AND p.kind='week' AND i.level!='day' AND NOT(i.level='backlog' AND (i.project_id IS NOT NULL OR EXISTS(SELECT 1 FROM item_projects ip WHERE ip.item_id=i.id))) AND p.week_start<=?2 AND date(p.week_start,'+6 days')>=?3)",params![parent,(start+Duration::days(6)).to_string(),start.to_string()],|r|r.get(0)).map_err(err)?;
        if !valid {return Err("日任务只能关联本周已安排的周任务".into());}
        let chain=super::ai_service::rows(c,"WITH RECURSIVE parents AS (SELECT *,0 depth FROM items WHERE id=? UNION ALL SELECT i.*,p.depth+1 FROM items i JOIN parents p ON i.id=p.parent_id WHERE p.depth<100) SELECT * FROM parents ORDER BY depth",&[&parent])?;
        let nearest=|key:&str|chain.iter().find_map(|i|i[key].as_str().map(str::to_owned));
        for key in ["project_id","annual_goal_id","cycle_goal_id"] {v[key]=json!(nearest(key));}
        let inherited_cycle=if let Some(goal)=v["cycle_goal_id"].as_str(){
            let (goal_cycle,annual):(String,String)=c.query_row("SELECT cycle_id,annual_goal_id FROM cycle_goals WHERE id=?",[goal],|r|Ok((r.get(0)?,r.get(1)?))).map_err(err)?;
            v["annual_goal_id"]=json!(annual);Some(goal_cycle)
        }else{nearest("cycle_id")};
        if cycle.is_some() && inherited_cycle.is_some() && cycle!=inherited_cycle {return Err("本周周任务的上级不属于当前12周计划".into());}
        // Associations supplied for a different project cannot survive parent selection.
        for (field,table) in [("version_id","project_versions"),("milestone_id","project_stages")] {if let Some(s)=v[field].as_str(){let project:Option<String>=c.query_row(&format!("SELECT project_id FROM {table} WHERE id=?"),[s],|r|r.get(0)).map_err(err)?;if project.as_deref()!=v["project_id"].as_str(){v[field]=Value::Null;}}}
    }else{v["parent_id"]=Value::Null;v["annual_goal_id"]=Value::Null;v["cycle_goal_id"]=Value::Null;}
    Ok(())
}

pub fn is_project_item(c:&Connection,v:&Value)->Result<bool,String>{
    Ok(text(v,"level")=="backlog" && (v["project_id"].as_str().is_some_and(|p|!p.is_empty()) || c.query_row("SELECT EXISTS(SELECT 1 FROM item_projects WHERE item_id=?)",[text(v,"id")],|r|r.get::<_,bool>(0)).map_err(err)?))
}
pub fn validate_project_boundary(c:&Connection,kind:&str,v:&Value,id:&str)->Result<(),String>{
    if kind=="item" {
        let old=super::ai_service::rows(c,"SELECT * FROM items WHERE id=?",&[&id])?.pop();
        let old_pool=old.as_ref().map(|o|is_project_item(c,o)).transpose()?.unwrap_or(false);
        if old_pool && (old.as_ref().is_some_and(|o|o["project_id"].as_str().is_some()) && v["project_id"].as_str().is_none() || !is_project_item(c,v)?) {return Err("项目事项须保留项目归属及事项池粒度".into());}
        if old_pool && text(v,"level")!="backlog" {return Err("项目事项不能转换为执行任务，请从任务端关联项目事项".into());}
        if v["parent_id"]!=old.as_ref().map(|o|o["parent_id"].clone()).unwrap_or(Value::Null) {
            if let Some(parent)=v["parent_id"].as_str() {
                if let Some(p)=super::ai_service::rows(c,"SELECT * FROM items WHERE id=?",&[&parent])?.pop() {
                    if is_project_item(c,&p)? {return Err("项目事项请通过project_item_ids引用，不作为父任务".into());}
                }
            }
        }
        if old_pool || is_project_item(c,v)? {
            for key in ["parent_id","cycle_id","annual_goal_id","cycle_goal_id"] {
                let previous=old.as_ref().map(|o|&o[key]).unwrap_or(&Value::Null);
                if &v[key]!=previous {return Err("项目事项不设置任务层级；已有历史关联保持原值".into());}
            }
        }
    }
    if kind=="placement" {
        let item=super::ai_service::rows(c,"SELECT * FROM items WHERE id=?",&[&text(v,"item_id")])?.pop().ok_or("事项不存在")?;
        if is_project_item(c,&item)? {return Err("项目事项不能直接安排时间，请在执行任务中引用它；历史安排只读保留".into());}
    }
    Ok(())
}
pub fn validate_project_links(c:&Connection)->Result<(),String>{
    let invalid:bool=c.query_row("SELECT EXISTS(SELECT 1 FROM task_project_items l JOIN items t ON t.id=l.task_id JOIN items p ON p.id=l.project_item_id WHERE t.level NOT IN('day','week','cycle') OR p.level<>'backlog' OR (p.project_id IS NULL AND NOT EXISTS(SELECT 1 FROM item_projects ip WHERE ip.item_id=p.id)))",[],|r|r.get(0)).map_err(err)?;
    let source_goal=has(c,"cycle_goals","source_item_id")? && c.query_row("SELECT EXISTS(SELECT 1 FROM task_project_items l JOIN cycle_goals g ON g.source_item_id=l.task_id)",[],|r|r.get::<_,bool>(0)).map_err(err)?;
    if invalid || source_goal {return Err("引用必须由执行任务指向有项目归属的事项池记录".into());}Ok(())
}
pub fn validate(c:&Connection,kind:&str,v:&mut Value,id:&str)->Result<(),String>{
    let date=|s:&str|chrono::NaiveDate::parse_from_str(s,"%Y-%m-%d").map_err(err);
    if kind=="placement" {
        let item=text(v,"item_id");if item.is_empty(){return Err("请选择事项".into());}
        if text(v,"kind")=="day" {date(text(v,"date"))?;v["week_start"]=Value::Null;v["cycle_id"]=Value::Null;v["week_no"]=Value::Null;}
        else if text(v,"kind")=="week" {
            if !v["cycle_id"].is_null() {
                let start:String=c.query_row("SELECT start_date FROM cycles WHERE id=?",[text(v,"cycle_id")],|r|r.get(0)).map_err(err)?;
                let week=v["week_no"].as_i64().filter(|w|(1..=12).contains(w)).ok_or("周次须为1—12")?;
                v["week_start"]=json!((date(&start)?+chrono::Duration::days((week-1)*7)).to_string());
            } else {let d=date(text(v,"week_start"))?;use chrono::Datelike;if d.weekday()!=chrono::Weekday::Mon{return Err("独立周安排从周一开始".into());}}
            v["date"]=Value::Null;
        }else{return Err("安排类型无效".into());}
    }
    if kind=="item" {
        if let Some(p)=v["parent_id"].as_str(){let cyclic:bool=c.query_row("WITH RECURSIVE parents(id,parent_id) AS (SELECT id,parent_id FROM items WHERE id=? UNION SELECT i.id,i.parent_id FROM items i JOIN parents p ON i.id=p.parent_id) SELECT EXISTS(SELECT 1 FROM parents WHERE id=?)",params![p,id],|r|r.get(0)).map_err(err)?;if p==id||cyclic{return Err("父事项不能形成循环".into());}}
        for (field,table) in [("version_id","project_versions"),("milestone_id","project_stages")] {if let Some(s)=v[field].as_str(){let project:Option<String>=c.query_row(&format!("SELECT project_id FROM {table} WHERE id=?"),[s],|r|r.get(0)).map_err(err)?;if project.as_deref()!=v["project_id"].as_str(){return Err("版本和里程碑必须属于同一项目".into());}}}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
 use super::*;
 pub fn fixture()->Connection {
  let mut c=Connection::open_in_memory().unwrap();c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA recursive_triggers=ON;").unwrap();
  let src=include_str!("../../src/db.ts");
  for line in src.lines().map(str::trim).filter(|s|s.starts_with("'CREATE TABLE")||s.starts_with("'CREATE INDEX")){c.execute_batch(&line.trim_start_matches('\'').trim_end_matches(',').trim_end_matches('\'').replace("\\'","'")).unwrap();}
  super::super::long_projects::migrate_in(&mut c).unwrap();super::super::task_management::upgrade(&mut c).unwrap();super::super::time_categories::upgrade(&mut c).unwrap();
  c.execute_batch("INSERT INTO directions VALUES('dir','开发',''); INSERT INTO annual_goals(id,title,year,direction_id,created_at) VALUES('annual','产品',2026,'dir',''); INSERT INTO cycles VALUES('cycle','跨年周期','2026-11-02','',''); INSERT INTO cycle_goals VALUES('goal','cycle','annual','MVP','');
   INSERT INTO long_projects(id,title,created_at,updated_at) VALUES('project','产品','','');
   INSERT INTO tasks(id,title,status,cycle_id,annual_goal_id,cycle_goal_id,created_at) VALUES('todo','开发','doing','cycle','annual','goal',''); INSERT INTO task_projects VALUES('todo','project');
   INSERT INTO week_assignments(id,task_id,cycle_id,commitment,status,priority,long_project_id,created_at) VALUES('week','todo','cycle','开发','todo','P1','project',''); INSERT INTO assignment_weeks VALUES('week',1),('week',2);
   INSERT INTO day_tasks(id,date,title,status,assignment_id,task_id,created_at) VALUES('day','2026-11-03','开发','todo','week','todo','');
   INSERT INTO project_plans(id,project_id,title,created_at) VALUES('plan','project','以后开发','');
   INSERT INTO time_entries(id,title,start_at,end_at,category_id,day_task_id,task_id,created_at) SELECT 'entry','历史投入','2026-11-03T09:00:00+08:00','2026-11-03T10:00:00+08:00',id,'day','todo','' FROM categories WHERE annual_goal_id='annual';").unwrap();c
 }
 #[test] fn legacy_hierarchy_keeps_scheduled_assignments_weekly(){
  let mut c=fixture();c.execute("UPDATE tasks SET cycle_goal_id=NULL",[]).unwrap();migrate(&mut c).unwrap();
  let goal:String=c.query_row("SELECT id FROM cycle_goals WHERE source_item_id='task:todo'",[],|r|r.get(0)).unwrap();
  assert_eq!(c.query_row("SELECT level FROM items WHERE id='week:week'",[],|r|r.get::<_,String>(0)).unwrap(),"week");
  assert_eq!(c.query_row("SELECT cycle_goal_id FROM items WHERE id='week:week'",[],|r|r.get::<_,String>(0)).unwrap(),goal);
  assert_eq!(c.query_row("SELECT parent_id FROM items WHERE id='day:day'",[],|r|r.get::<_,String>(0)).unwrap(),"week:week");
  let snapshot=super::super::ai_service::rows(&c,"SELECT * FROM items ORDER BY id",&[]).unwrap();migrate(&mut c).unwrap();assert_eq!(snapshot,super::super::ai_service::rows(&c,"SELECT * FROM items ORDER BY id",&[]).unwrap());
  c.execute("UPDATE items SET cycle_goal_id='goal',priority='P0',title='修正待办名称' WHERE id='week:week'",[]).unwrap();assert_eq!(c.query_row("SELECT level FROM items WHERE id='week:week'",[],|r|r.get::<_,String>(0)).unwrap(),"week");assert_eq!(c.query_row("SELECT cycle_goal_id FROM items WHERE id='week:week'",[],|r|r.get::<_,String>(0)).unwrap(),"goal");c.execute("UPDATE items SET cycle_goal_id=? WHERE id='week:week'",[&goal]).unwrap();
  c.execute("INSERT INTO week_assignments(id,task_id,cycle_id,commitment,status,created_at) VALUES('later','todo','cycle','兼容新增','todo','')",[]).unwrap();assert_eq!(c.query_row("SELECT level FROM items WHERE id='week:later'",[],|r|r.get::<_,String>(0)).unwrap(),"week");
  c.execute("UPDATE cycle_goals SET title='修正目标名称' WHERE id=?",[&goal]).unwrap();assert_eq!(c.query_row("SELECT title FROM tasks WHERE id='todo'",[],|r|r.get::<_,String>(0)).unwrap(),"修正目标名称");
  c.execute("INSERT INTO items(id,title,level,cycle_id,annual_goal_id,cycle_goal_id,created_at) VALUES('cycle-todo','真正的本轮待办','cycle','cycle','annual',?,'')",[&goal]).unwrap();
  let mut week=json!({"level":"week","parent_id":"cycle-todo","annual_goal_id":"wrong","cycle_goal_id":null});normalize_week_item(&c,&mut week,"new").unwrap();assert_eq!(week["annual_goal_id"],"annual");assert_eq!(week["cycle_goal_id"],goal);
  let mut wrong=json!({"level":"week","parent_id":"task:todo"});assert!(normalize_week_item(&c,&mut wrong,"new").is_err());
  let counts:i64=c.query_row("SELECT COUNT(*) FROM placements",[],|r|r.get(0)).unwrap();assert_eq!(counts,3);assert_eq!(c.query_row("SELECT COUNT(*) FROM time_entries",[],|r|r.get::<_,i64>(0)).unwrap(),1);
 }
 #[test] fn failed_hierarchy_repair_rolls_back_and_can_retry(){
  let mut c=fixture();c.execute("UPDATE tasks SET cycle_goal_id=NULL",[]).unwrap();schema(&c).unwrap();
  c.execute_batch("CREATE TRIGGER cycle_goal_source_title AFTER UPDATE OF title ON items BEGIN SELECT 1;END;").unwrap();assert!(migrate(&mut c).is_err());assert!(!has(&c,"cycle_goals","source_item_id").unwrap());
  assert_eq!(c.query_row("SELECT level FROM items WHERE id='week:week'",[],|r|r.get::<_,String>(0)).unwrap(),"week");c.execute_batch("DROP TRIGGER cycle_goal_source_title;").unwrap();migrate(&mut c).unwrap();assert_eq!(c.query_row("SELECT level FROM items WHERE id='week:week'",[],|r|r.get::<_,String>(0)).unwrap(),"week");assert_eq!(c.query_row("SELECT COUNT(*) FROM items",[],|r|r.get::<_,i64>(0)).unwrap(),4);
 }
 #[test] fn hierarchy_repair_preserves_real_database_copy(){
  let Ok(path)=std::env::var("WORKBENCH_HIERARCHY_FIXTURE") else{return;};
  let mut c=Connection::open(path).unwrap();c.execute_batch("PRAGMA foreign_keys=ON;PRAGMA recursive_triggers=ON;").unwrap();
  let rows=|c:&Connection,sql:&str|super::super::ai_service::rows(c,sql,&[]).unwrap();
  let items=rows(&c,"SELECT id,title,status,priority FROM items ORDER BY id");let placements=rows(&c,"SELECT * FROM placements ORDER BY id");let times=rows(&c,"SELECT * FROM time_entries ORDER BY id");
  migrate(&mut c).unwrap();migrate(&mut c).unwrap();
  assert_eq!(items,rows(&c,"SELECT id,title,status,priority FROM items ORDER BY id"));assert_eq!(placements,rows(&c,"SELECT * FROM placements ORDER BY id"));assert_eq!(times,rows(&c,"SELECT * FROM time_entries ORDER BY id"));
  assert_eq!(c.query_row("SELECT COUNT(*) FROM items i JOIN cycle_goals g ON i.parent_id=g.source_item_id JOIN item_legacy_map m ON m.item_id=i.id AND m.kind='assignment' WHERE EXISTS(SELECT 1 FROM placements p WHERE p.item_id=i.id AND p.kind='week') AND i.level!='week'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
  assert_eq!(c.query_row("SELECT COUNT(*) FROM pragma_foreign_key_check",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test] fn scheduled_week_recovery_is_targeted_restartable_and_preserves_manual_edits(){
  let mut c=fixture();c.execute("UPDATE tasks SET cycle_goal_id=NULL",[]).unwrap();migrate(&mut c).unwrap();
  // Reproduce the installed v2 state, including one user-restored task and one real cycle todo.
  c.execute_batch("DELETE FROM unified_migrations WHERE name='scheduled-week-level-v3';
    UPDATE items SET level='cycle',title='用户改过的名称',status='doing',priority='P0' WHERE id='week:week';
    INSERT INTO week_assignments(id,task_id,cycle_id,commitment,status,created_at) VALUES('manual','todo','cycle','已手动恢复','done',''),('pool','todo','cycle','未安排的本轮待办','todo','');
    INSERT INTO assignment_weeks VALUES('manual',2);
    UPDATE items SET level='cycle' WHERE id='week:pool';").unwrap();
  let trigger:String=c.query_row("SELECT sql FROM sqlite_master WHERE name='unified_meta_week_UPDATE'",[],|r|r.get(0)).unwrap();
  let faulty=trigger.replace("THEN 'week' ELSE level", "THEN CASE WHEN EXISTS(SELECT 1 FROM cycle_goals WHERE source_item_id='task:'||NEW.task_id) THEN 'cycle' ELSE 'week' END ELSE level");
  c.execute_batch("DROP TRIGGER unified_meta_week_UPDATE;").unwrap();c.execute_batch(&faulty).unwrap();
  let manual=super::super::ai_service::rows(&c,"SELECT * FROM items WHERE id='week:manual'",&[]).unwrap();
  c.execute_batch("CREATE TRIGGER reject_week_restore BEFORE UPDATE OF level ON items WHEN NEW.id='week:week' BEGIN SELECT RAISE(ABORT,'injected restore failure');END;").unwrap();
  assert!(migrate(&mut c).is_err());
  assert_eq!(c.query_row("SELECT COUNT(*) FROM unified_migrations WHERE name='scheduled-week-level-v3'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
  assert_eq!(c.query_row("SELECT level FROM items WHERE id='week:week'",[],|r|r.get::<_,String>(0)).unwrap(),"cycle");
  c.execute_batch("DROP TRIGGER reject_week_restore;").unwrap();migrate(&mut c).unwrap();
  let mut restored=super::super::ai_service::rows(&c,"SELECT * FROM items WHERE id='week:week'",&[]).unwrap().remove(0);
  assert_eq!(restored["level"],"week");assert_eq!(restored["title"],"用户改过的名称");assert_eq!(restored["status"],"doing");assert_eq!(restored["priority"],"P0");
  assert_eq!(manual,super::super::ai_service::rows(&c,"SELECT * FROM items WHERE id='week:manual'",&[]).unwrap());
  assert_eq!(c.query_row("SELECT level FROM items WHERE id='week:pool'",[],|r|r.get::<_,String>(0)).unwrap(),"cycle");
  restored["status"]=json!("done");let before=restored.clone();normalize_week_item(&c,&mut restored,"week:week").unwrap();assert_eq!(restored,before);
  let snapshot=super::super::ai_service::rows(&c,"SELECT * FROM items ORDER BY id",&[]).unwrap();migrate(&mut c).unwrap();assert_eq!(snapshot,super::super::ai_service::rows(&c,"SELECT * FROM items ORDER BY id",&[]).unwrap());
  let sql:String=c.query_row("SELECT sql FROM sqlite_master WHERE name='unified_meta_week_UPDATE'",[],|r|r.get(0)).unwrap();assert!(!sql.contains("THEN 'cycle' ELSE 'week'"));
 }
 #[test] fn day_defaults_enforce_current_week_and_inherit_without_rewriting_history(){
  let mut c=fixture();migrate(&mut c).unwrap();
  let today=chrono::NaiveDate::from_ymd_opt(2026,11,3).unwrap();
  c.execute("UPDATE items SET cycle_goal_id='goal' WHERE id='task:todo'",[]).unwrap();
  let mut day=json!({"level":"day","parent_id":"week:week","priority":"P0","status":"done"});normalize_day_item(&c,&mut day,"new",today).unwrap();
  assert_eq!(day["cycle_id"],"cycle");assert_eq!(day["project_id"],"project");assert_eq!(day["annual_goal_id"],"annual");assert_eq!(day["cycle_goal_id"],"goal");assert_eq!(day["priority"],"P0");
  assert_eq!(c.query_row("SELECT status FROM items WHERE id='week:week'",[],|r|r.get::<_,String>(0)).unwrap(),"todo");
  let mut bad=json!({"level":"day","parent_id":"task:todo"});assert!(normalize_day_item(&c,&mut bad,"new",today).unwrap_err().contains("本周"));
  let mut late=json!({"level":"day","parent_id":"week:week"});assert!(normalize_day_item(&c,&mut late,"new",chrono::NaiveDate::from_ymd_opt(2026,11,17).unwrap()).is_err());
  let mut independent=json!({"level":"day","cycle_id":null});normalize_day_item(&c,&mut independent,"new",today).unwrap();assert_eq!(independent["cycle_id"],"cycle");assert!(independent["parent_id"].is_null());
  let mut historical=super::super::ai_service::rows(&c,"SELECT * FROM items WHERE id='day:day'",&[]).unwrap().remove(0);historical["status"]=json!("done");let original=historical.clone();normalize_day_item(&c,&mut historical,"day:day",chrono::NaiveDate::from_ymd_opt(2027,2,1).unwrap()).unwrap();assert_eq!(historical,original);
 }
 #[test] fn migration_keeps_distinct_identities_relations_times_and_is_restartable(){
  let mut c=fixture();migrate(&mut c).unwrap();migrate(&mut c).unwrap();
  assert_eq!(c.query_row("SELECT COUNT(*) FROM items",[],|r|r.get::<_,i64>(0)).unwrap(),4);
  assert_eq!(c.query_row("SELECT COUNT(*) FROM placements",[],|r|r.get::<_,i64>(0)).unwrap(),3);
  assert_eq!(c.query_row("SELECT parent_id FROM items WHERE id='day:day'",[],|r|r.get::<_,String>(0)).unwrap(),"week:week");
  assert_eq!(c.query_row("SELECT COUNT(*) FROM time_entries",[],|r|r.get::<_,i64>(0)).unwrap(),1);
  assert_eq!(c.query_row("SELECT project_id FROM items WHERE id='task:todo'",[],|r|r.get::<_,String>(0)).unwrap(),"project");
  c.execute("UPDATE items SET status='done' WHERE id='day:day'",[]).unwrap();assert_eq!(c.query_row("SELECT status FROM day_tasks WHERE id='day'",[],|r|r.get::<_,String>(0)).unwrap(),"done");
  assert_eq!(c.query_row("SELECT status FROM items WHERE id='week:week'",[],|r|r.get::<_,String>(0)).unwrap(),"todo");
  c.execute("UPDATE week_assignments SET commitment='接口',priority='P0' WHERE id='week'",[]).unwrap();assert_eq!(c.query_row("SELECT title FROM items WHERE id='week:week'",[],|r|r.get::<_,String>(0)).unwrap(),"接口");
  c.execute("INSERT INTO day_tasks(id,date,title,status,created_at) VALUES('new','2026-11-04','临时','todo','')",[]).unwrap();
  assert_eq!(c.query_row("SELECT date FROM placements WHERE item_id='day:new'",[],|r|r.get::<_,String>(0)).unwrap(),"2026-11-04");
 }
 #[test] fn invalid_migration_rolls_back_and_timed_plan_never_creates_actual(){
  let mut c=fixture();c.execute_batch("PRAGMA foreign_keys=OFF; UPDATE day_tasks SET task_id='missing';").unwrap();assert!(migrate(&mut c).is_err());assert_eq!(c.query_row("SELECT COUNT(*) FROM items",[],|r|r.get::<_,i64>(0)).unwrap(),0);assert!(!has(&c,"time_entries","item_id").unwrap());
  let mut c=fixture();migrate(&mut c).unwrap();
  c.execute("INSERT INTO day_tasks(id,date,title,status,scheduled_start_at,scheduled_end_at,scheduled_category_id,created_at) SELECT 'timed','2026-11-04','计划','todo','2026-11-04T09:00:00+08:00','2026-11-04T10:00:00+08:00',id,'' FROM categories WHERE name='学习'",[]).unwrap();
  assert_eq!(c.query_row("SELECT COUNT(*) FROM time_entries",[],|r|r.get::<_,i64>(0)).unwrap(),1);
  c.execute("UPDATE day_tasks SET long_project_id=NULL WHERE id='day'",[]).unwrap();assert_eq!(c.query_row("SELECT long_project_id FROM time_entries WHERE id='entry'",[],|r|r.get::<_,String>(0)).unwrap(),"project");
 }
 #[test] fn management_api_applies_day_defaults_and_rejects_non_week_parent(){
  let mut c=fixture();migrate(&mut c).unwrap();super::super::ai_service::schema(&c).unwrap();super::super::ai_workbench::schema(&c).unwrap();
  let today=chrono::Utc::now().with_timezone(&chrono::FixedOffset::east_opt(28800).unwrap()).date_naive();use chrono::Datelike;let start=today-chrono::Duration::days(today.weekday().num_days_from_monday() as i64);
  let args=json!({"request_id":"day-hierarchy","intent":"explicit","operations":[
   {"kind":"cycle","action":"create","ref":"cycle","values":{"title":"当前计划","start_date":start.to_string()}},
   {"kind":"cycleGoal","action":"create","ref":"goal","values":{"title":"本轮目标","cycle_id":"$cycle","annual_goal_id":"annual"}},
   {"kind":"item","action":"create","ref":"todo","values":{"title":"本轮待办","level":"cycle","cycle_goal_id":"$goal","project_id":"project"}},
   {"kind":"item","action":"create","ref":"week","values":{"title":"本周任务","level":"week","parent_id":"$todo"}},
   {"kind":"placement","action":"create","values":{"item_id":"$week","kind":"week","week_start":start.to_string()}},
   {"kind":"item","action":"create","ref":"day","values":{"title":"日任务","level":"day","parent_id":"$week","priority":"P3"}}
  ]});
  let receipt=super::super::ai_workbench::execute(&mut c,&args,"workbench-ui").unwrap();let day=&receipt["records"][5];assert_eq!(day["cycle_id"],receipt["records"][0]["id"]);assert_eq!(day["cycle_goal_id"],receipt["records"][1]["id"]);assert_eq!(day["annual_goal_id"],"annual");assert_eq!(day["project_id"],"project");assert_eq!(day["priority"],"P3");
  let bad=json!({"request_id":"wrong-parent","intent":"explicit","operation":{"kind":"item","action":"create","values":{"title":"不得保存","level":"day","parent_id":receipt["records"][2]["id"]}}});assert!(super::super::ai_workbench::execute(&mut c,&bad,"workbench-ui").unwrap_err().contains("本周"));assert_eq!(c.query_row("SELECT COUNT(*) FROM items WHERE title='不得保存'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
 #[test] fn ui_and_assistant_share_atomic_identities_and_manual_time_attribution(){
  let mut c=fixture();migrate(&mut c).unwrap();super::super::ai_service::schema(&c).unwrap();super::super::ai_workbench::schema(&c).unwrap();
  let args=json!({"request_id":"new-independent","intent":"explicit","operations":[{"kind":"item","action":"create","ref":"i","values":{"title":"生活事务"}},{"kind":"placement","action":"create","values":{"item_id":"$i","kind":"week","week_start":"2026-11-09"}},{"kind":"placement","action":"create","values":{"item_id":"$i","kind":"day","date":"2026-11-10"}}]});
  let result=super::super::ai_workbench::execute(&mut c,&args,"workbench-ui").unwrap();let item=text(&result["records"][0],"id");
  let repeated=super::super::ai_workbench::execute(&mut c,&args,"workbench-ui").unwrap();assert_eq!(result,repeated);
  let cat:String=c.query_row("SELECT id FROM categories WHERE name='学习'",[],|r|r.get(0)).unwrap();
  let time=json!({"request_id":"new-time","intent":"explicit","operation":{"kind":"time","action":"create","values":{"start_at":"2026-11-10T23:00:00+08:00","end_at":"2026-11-11T01:00:00+08:00","category_id":cat,"item_id":item,"work_kind":"life","life_category":"杂事","category_manual":1}}});
  let saved=super::super::ai_workbench::execute(&mut c,&time,"workbench-ui").unwrap();assert_eq!(saved["records"][0]["work_kind"],"life");
  let q=super::super::ai_workbench::query(&c,&json!({"kind":"item","id":item})).unwrap();assert_eq!(q["items"][0]["title"],"生活事务");
  let automatic=json!({"request_id":"automatic-time","intent":"explicit","operation":{"kind":"time","action":"create","values":{"start_at":"2026-11-12T09:00:00+08:00","end_at":"2026-11-12T10:00:00+08:00","category_id":cat,"item_id":item}}});
  let saved=super::super::ai_workbench::execute(&mut c,&automatic,"workbench-ui").unwrap();assert_eq!(saved["records"][0]["work_kind"],"work");
  let bad=json!({"request_id":"rollback","intent":"explicit","operations":[{"kind":"item","action":"create","ref":"i","values":{"title":"不保存"}},{"kind":"placement","action":"create","values":{"item_id":"$i","kind":"week","week_start":"2026-11-10"}}]});assert!(super::super::ai_workbench::execute(&mut c,&bad,"workbench-ui").is_err());assert_eq!(c.query_row("SELECT COUNT(*) FROM items WHERE title='不保存'",[],|r|r.get::<_,i64>(0)).unwrap(),0);
 }
}

pub fn statistics(c:&Connection,a:&Value)->Result<Value,String>{
 use std::collections::BTreeMap;
 use chrono::{Duration,NaiveDate};
 let from=NaiveDate::parse_from_str(text(a,"date_from"),"%Y-%m-%d").map_err(err)?;
 let to=NaiveDate::parse_from_str(text(a,"date_to"),"%Y-%m-%d").map_err(err)?;
 let lo=format!("{from}T00:00:00+08:00");let hi=format!("{}T00:00:00+08:00",to+Duration::days(1));let now=chrono::Utc::now().to_rfc3339();
 let duration:f64=c.query_row("SELECT max(0,(min(julianday(?2),julianday(?3))-julianday(?1))*1440)",params![lo,hi,now],|r|r.get(0)).map_err(err)?;
 let rows=super::ai_service::rows(c,"SELECT e.*,c.name category_name,g.title goal_title,d.title direction_title,p.title project_title,max(0,(min(julianday(e.end_at),julianday(?2),julianday(?3))-max(julianday(e.start_at),julianday(?1)))*1440) minutes FROM time_entries e LEFT JOIN categories c ON c.id=e.category_id LEFT JOIN annual_goals g ON g.id=e.primary_goal_id LEFT JOIN directions d ON d.id=e.direction_id LEFT JOIN long_projects p ON p.id=e.long_project_id WHERE julianday(e.end_at)>julianday(?1) AND julianday(e.start_at)<min(julianday(?2),julianday(?3))",&[&lo,&hi,&now])?;
 let mut life=BTreeMap::<String,f64>::new();let mut projects=BTreeMap::<String,f64>::new();let mut goals=BTreeMap::<String,f64>::new();let mut directions=BTreeMap::<String,f64>::new();let mut recorded=0.;let mut work=0.;
 for e in rows {let n=e["minutes"].as_f64().unwrap_or(0.);recorded+=n;let key=match text(&e,"work_kind"){"work"=>"工作".to_string(),"life"=>e["life_category"].as_str().filter(|s|!s.is_empty()).unwrap_or(text(&e,"category_name")).to_string(),_=>format!("历史分类：{}",text(&e,"category_name"))};*life.entry(key).or_default()+=n;
  if text(&e,"work_kind")=="work" {work+=n;for (map,k) in [(&mut projects,"project_title"),(&mut goals,"goal_title"),(&mut directions,"direction_title")] {*map.entry(e[k].as_str().filter(|s|!s.is_empty()).unwrap_or("未归属工作").to_string()).or_default()+=n;}}
 }
 life.insert("未记录".into(),(duration-recorded).max(0.));
 let mut q="SELECT DISTINCT i.* FROM items i LEFT JOIN placements p ON p.item_id=i.id WHERE NOT EXISTS(SELECT 1 FROM cycle_goals g WHERE g.source_item_id=i.id)".to_string();let mut values=vec![];
 if let Some(cycle)=a["cycle_id"].as_str(){q.push_str(" AND (i.cycle_id=? OR p.cycle_id=?)");values.push(cycle.to_string());values.push(cycle.to_string());}
 if let Some(project)=a["project_id"].as_str(){q.push_str(" AND (i.project_id=? OR i.id IN(SELECT item_id FROM item_projects WHERE project_id=?))");values.push(project.to_string());values.push(project.to_string());}
 q.push_str(" AND (i.level='cycle' OR (p.kind='day' AND p.date>=? AND p.date<=?) OR (p.kind='week' AND p.week_start<=? AND date(p.week_start,'+6 days')>=?))");values.extend([from.to_string(),to.to_string(),to.to_string(),from.to_string()]);
 let refs=values.iter().map(|s|s as &dyn rusqlite::ToSql).collect::<Vec<_>>();let items=super::ai_service::rows(c,&q,&refs)?;
 let mut counts=BTreeMap::<String,i64>::new();for i in &items {*counts.entry(text(i,"status").into()).or_default()+=1;}
 Ok(json!({"items":items,"status_counts":counts,"total_minutes":duration,"recorded_minutes":recorded,"work_minutes":work,"life":life,"projects":projects,"goals":goals,"directions":directions}))
}
