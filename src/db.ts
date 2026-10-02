import Database from '@tauri-apps/plugin-sql';
import { invoke } from '@tauri-apps/api/core';
import { addDays, EMPTY, id, localDate, type Data, type Recurrence } from './model';

let connection: Database | null = null;
let initialization: Promise<void> | null = null;
// Historical tables and columns remain so existing local databases can be migrated without losing linked records.
const schema = [
  'CREATE TABLE IF NOT EXISTS cycles (id TEXT PRIMARY KEY, title TEXT NOT NULL, start_date TEXT NOT NULL, review TEXT NOT NULL DEFAULT \'\', created_at TEXT NOT NULL)',
  'CREATE TABLE IF NOT EXISTS directions (id TEXT PRIMARY KEY, title TEXT NOT NULL, created_at TEXT NOT NULL)',
  'CREATE TABLE IF NOT EXISTS annual_goals (id TEXT PRIMARY KEY, direction_id TEXT REFERENCES directions(id), title TEXT NOT NULL, year INTEGER NOT NULL, completed INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL)',
  'CREATE TABLE IF NOT EXISTS cycle_goals (id TEXT PRIMARY KEY, cycle_id TEXT NOT NULL REFERENCES cycles(id), annual_goal_id TEXT NOT NULL REFERENCES annual_goals(id), title TEXT NOT NULL, created_at TEXT NOT NULL)',
  'CREATE TABLE IF NOT EXISTS projects (id TEXT PRIMARY KEY, cycle_goal_id TEXT REFERENCES cycle_goals(id), title TEXT NOT NULL, status TEXT NOT NULL, created_at TEXT NOT NULL)',
  'CREATE TABLE IF NOT EXISTS tasks (id TEXT PRIMARY KEY, cycle_goal_id TEXT REFERENCES cycle_goals(id), project_id TEXT REFERENCES projects(id), cycle_id TEXT REFERENCES cycles(id), annual_goal_id TEXT REFERENCES annual_goals(id), title TEXT NOT NULL, status TEXT NOT NULL, due_date TEXT, created_at TEXT NOT NULL)',
  'CREATE TABLE IF NOT EXISTS week_assignments (id TEXT PRIMARY KEY, task_id TEXT NOT NULL REFERENCES tasks(id), cycle_id TEXT REFERENCES cycles(id), commitment TEXT NOT NULL, status TEXT NOT NULL, review TEXT NOT NULL DEFAULT \'\', created_at TEXT NOT NULL)',
  'CREATE TABLE IF NOT EXISTS assignment_weeks (assignment_id TEXT NOT NULL REFERENCES week_assignments(id) ON DELETE CASCADE, week_no INTEGER NOT NULL CHECK(week_no BETWEEN 1 AND 12), PRIMARY KEY(assignment_id,week_no))',
  'CREATE TABLE IF NOT EXISTS day_tasks (id TEXT PRIMARY KEY, date TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL, assignment_id TEXT REFERENCES week_assignments(id), project_id TEXT REFERENCES projects(id), task_id TEXT REFERENCES tasks(id), recurrence_id TEXT, scheduled_start_at TEXT, scheduled_end_at TEXT, scheduled_category_id TEXT REFERENCES categories(id), created_at TEXT NOT NULL, UNIQUE(recurrence_id,date))',
  'CREATE TABLE IF NOT EXISTS categories (id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, color TEXT NOT NULL, created_at TEXT NOT NULL)',
  'CREATE TABLE IF NOT EXISTS plan_blocks (id TEXT PRIMARY KEY, date TEXT NOT NULL, start_min INTEGER NOT NULL, end_min INTEGER NOT NULL, title TEXT NOT NULL, day_task_id TEXT REFERENCES day_tasks(id), recurrence_id TEXT, created_at TEXT NOT NULL, CHECK(start_min>=0 AND end_min<=1440 AND end_min>start_min), UNIQUE(recurrence_id,date))',
  'CREATE TABLE IF NOT EXISTS daily_plan_templates (id TEXT PRIMARY KEY, start_min INTEGER NOT NULL, end_min INTEGER NOT NULL, title TEXT NOT NULL, created_at TEXT NOT NULL, CHECK(start_min>=0 AND end_min<=1440 AND end_min>start_min))',
  'CREATE TABLE IF NOT EXISTS time_entries (id TEXT PRIMARY KEY, start_at TEXT NOT NULL, end_at TEXT NOT NULL, title TEXT NOT NULL, category_id TEXT NOT NULL REFERENCES categories(id), day_task_id TEXT REFERENCES day_tasks(id), project_id TEXT REFERENCES projects(id), task_id TEXT REFERENCES tasks(id), created_at TEXT NOT NULL)',
  'CREATE TABLE IF NOT EXISTS recurrences (id TEXT PRIMARY KEY, kind TEXT NOT NULL, frequency TEXT NOT NULL, weekdays TEXT NOT NULL, start_date TEXT NOT NULL, end_date TEXT, title TEXT NOT NULL, start_min INTEGER, end_min INTEGER, project_id TEXT REFERENCES projects(id), task_id TEXT REFERENCES tasks(id), category_id TEXT REFERENCES categories(id), active INTEGER NOT NULL, created_at TEXT NOT NULL)',
  'CREATE TABLE IF NOT EXISTS recurrence_exceptions (recurrence_id TEXT NOT NULL, date TEXT NOT NULL, PRIMARY KEY(recurrence_id,date))',
  'CREATE INDEX IF NOT EXISTS idx_day_tasks_date ON day_tasks(date)',
  'CREATE INDEX IF NOT EXISTS idx_time_entries_start ON time_entries(start_at)',
  'CREATE INDEX IF NOT EXISTS idx_plan_blocks_date ON plan_blocks(date)',
  'CREATE INDEX IF NOT EXISTS idx_assignment_weeks_week ON assignment_weeks(week_no)'
];
const tables: Record<keyof Data,string> = { cycles:'cycles', directions:'directions', annualGoals:'annual_goals', tasks:'tasks', assignments:'week_assignments', assignmentWeeks:'assignment_weeks', dayTasks:'day_tasks', categories:'categories', planBlocks:'plan_blocks', dailyPlanTemplates:'daily_plan_templates', timeEntries:'time_entries', recurrences:'recurrences', longProjects:'long_projects', projectTypes:'project_types', taskProjects:'task_projects', projectStages:'project_stages', projectPlans:'project_plans', projectUpdates:'project_updates', planTasks:'plan_tasks', planWeeks:'plan_weeks' };
const allowed = new Set(Object.values(tables));
async function db() { if (!connection) connection = await Database.load('sqlite:workbench.db'); return connection; }
export function initDb(): Promise<void> {
  if(!initialization) initialization=initializeDb().catch(error=>{initialization=null;throw error;});
  return initialization;
}
async function initializeDb() {
  const c = await db(); await c.execute('PRAGMA foreign_keys = ON');
  for (const statement of schema) await c.execute(statement);
  const columns=await c.select<{name:string}[]>('PRAGMA table_info(tasks)');
  if(!columns.some(x=>x.name==='cycle_id')) await c.execute('ALTER TABLE tasks ADD COLUMN cycle_id TEXT REFERENCES cycles(id)');
  if(!columns.some(x=>x.name==='annual_goal_id')) await c.execute('ALTER TABLE tasks ADD COLUMN annual_goal_id TEXT REFERENCES annual_goals(id)');
  const annualColumns=await c.select<{name:string}[]>('PRAGMA table_info(annual_goals)');
  if(!annualColumns.some(x=>x.name==='completed')) await c.execute('ALTER TABLE annual_goals ADD COLUMN completed INTEGER NOT NULL DEFAULT 0');
  const cycleColumns=await c.select<{name:string}[]>('PRAGMA table_info(cycles)');
  if(!cycleColumns.some(x=>x.name==='review')) await c.execute("ALTER TABLE cycles ADD COLUMN review TEXT NOT NULL DEFAULT ''");
  for(const table of ['day_tasks','time_entries','recurrences']) {
    const fields=await c.select<{name:string}[]>(`PRAGMA table_info(${table})`);
    if(!fields.some(x=>x.name==='task_id')) await c.execute(`ALTER TABLE ${table} ADD COLUMN task_id TEXT REFERENCES tasks(id)`);
  }
  const dayTaskColumns=await c.select<{name:string}[]>('PRAGMA table_info(day_tasks)');
  if(!dayTaskColumns.some(x=>x.name==='scheduled_start_at')) await c.execute('ALTER TABLE day_tasks ADD COLUMN scheduled_start_at TEXT');
  if(!dayTaskColumns.some(x=>x.name==='scheduled_end_at')) await c.execute('ALTER TABLE day_tasks ADD COLUMN scheduled_end_at TEXT');
  if(!dayTaskColumns.some(x=>x.name==='scheduled_category_id')) await c.execute('ALTER TABLE day_tasks ADD COLUMN scheduled_category_id TEXT REFERENCES categories(id)');
  await c.execute("CREATE TRIGGER IF NOT EXISTS create_time_entry_for_timed_day_task AFTER INSERT ON day_tasks WHEN NEW.scheduled_start_at IS NOT NULL BEGIN SELECT RAISE(ABORT, '这段时间与已有实际记录重叠') WHERE EXISTS (SELECT 1 FROM time_entries WHERE start_at < NEW.scheduled_end_at AND end_at > NEW.scheduled_start_at); INSERT INTO time_entries (id,start_at,end_at,title,category_id,day_task_id,project_id,task_id,created_at) VALUES (NEW.id || ':time',NEW.scheduled_start_at,NEW.scheduled_end_at,NEW.title,NEW.scheduled_category_id,NEW.id,NEW.project_id,NEW.task_id,NEW.created_at); END");
  await c.execute('CREATE TRIGGER IF NOT EXISTS sync_time_entry_parent_on_day_task_change AFTER UPDATE OF assignment_id,project_id,task_id ON day_tasks BEGIN UPDATE time_entries SET project_id=NEW.project_id,task_id=NEW.task_id WHERE day_task_id=NEW.id; END');
  await c.execute('CREATE TRIGGER IF NOT EXISTS sync_day_task_parent_on_week_task_change AFTER UPDATE OF task_id ON week_assignments BEGIN UPDATE day_tasks SET task_id=NEW.task_id,project_id=(SELECT project_id FROM tasks WHERE id=NEW.task_id) WHERE assignment_id=NEW.id; END');
  await c.execute('CREATE TRIGGER IF NOT EXISTS sync_week_cycle_on_task_change AFTER UPDATE OF cycle_id ON tasks BEGIN UPDATE week_assignments SET cycle_id=NEW.cycle_id WHERE task_id=NEW.id; END');
  await c.execute('CREATE TABLE IF NOT EXISTS app_migrations (name TEXT PRIMARY KEY)');
  await c.execute('UPDATE tasks SET cycle_id=(SELECT cycle_id FROM cycle_goals WHERE id=COALESCE(tasks.cycle_goal_id,(SELECT cycle_goal_id FROM projects WHERE id=tasks.project_id))) WHERE cycle_id IS NULL');
  await c.execute('UPDATE tasks SET annual_goal_id=(SELECT annual_goal_id FROM cycle_goals WHERE id=COALESCE(tasks.cycle_goal_id,(SELECT cycle_goal_id FROM projects WHERE id=tasks.project_id))) WHERE annual_goal_id IS NULL');
  const migrated=await c.select<{n:number}[]>("SELECT COUNT(*) AS n FROM app_migrations WHERE name='unified_tasks'");
  if(!migrated[0].n) {
    await c.execute('INSERT INTO tasks (id,cycle_goal_id,project_id,cycle_id,annual_goal_id,title,status,due_date,created_at) SELECT g.id,NULL,NULL,g.cycle_id,g.annual_goal_id,g.title,\'todo\',NULL,g.created_at FROM cycle_goals g WHERE NOT EXISTS (SELECT 1 FROM tasks t WHERE t.cycle_goal_id=g.id) AND NOT EXISTS (SELECT 1 FROM projects p WHERE p.cycle_goal_id=g.id) AND NOT EXISTS (SELECT 1 FROM tasks t WHERE t.id=g.id)');
    await c.execute('INSERT INTO tasks (id,cycle_goal_id,project_id,cycle_id,annual_goal_id,title,status,due_date,created_at) SELECT p.id,NULL,p.id,g.cycle_id,g.annual_goal_id,p.title,p.status,NULL,p.created_at FROM projects p LEFT JOIN cycle_goals g ON g.id=p.cycle_goal_id WHERE NOT EXISTS (SELECT 1 FROM tasks t WHERE t.project_id=p.id OR t.id=p.id)');
    await c.execute('UPDATE day_tasks SET task_id=(SELECT task_id FROM week_assignments WHERE id=day_tasks.assignment_id) WHERE task_id IS NULL AND assignment_id IS NOT NULL');
    await c.execute('UPDATE day_tasks SET task_id=(SELECT id FROM tasks WHERE project_id=day_tasks.project_id LIMIT 1) WHERE task_id IS NULL AND project_id IS NOT NULL AND (SELECT COUNT(*) FROM tasks WHERE project_id=day_tasks.project_id)=1');
    await c.execute('UPDATE time_entries SET task_id=(SELECT task_id FROM day_tasks WHERE id=time_entries.day_task_id) WHERE task_id IS NULL AND day_task_id IS NOT NULL');
    await c.execute('UPDATE time_entries SET task_id=(SELECT id FROM tasks WHERE project_id=time_entries.project_id LIMIT 1) WHERE task_id IS NULL AND project_id IS NOT NULL AND (SELECT COUNT(*) FROM tasks WHERE project_id=time_entries.project_id)=1');
    await c.execute('UPDATE recurrences SET task_id=(SELECT id FROM tasks WHERE project_id=recurrences.project_id LIMIT 1) WHERE task_id IS NULL AND project_id IS NOT NULL AND (SELECT COUNT(*) FROM tasks WHERE project_id=recurrences.project_id)=1');
    await c.execute("INSERT INTO app_migrations (name) VALUES ('unified_tasks')");
  }
  await invoke('init_long_projects');
  await invoke('init_task_management');
  await invoke('init_time_categories');
}
export async function readData(): Promise<Data> {
  const c = await db(); const result = { ...EMPTY };
  for (const [key,table] of Object.entries(tables)) (result as unknown as Record<string,unknown>)[key] = await c.select(`SELECT * FROM ${table}`);
  return result;
}
export async function insert(table:string, values:Record<string,unknown>) {
  if (!allowed.has(table)) throw Error('不支持的数据表');
  const c=await db(); const keys=Object.keys(values); const params=keys.map(k=>values[k]);
  await c.execute(`INSERT INTO ${table} (${keys.join(',')}) VALUES (${keys.map((_,i)=>`$${i+1}`).join(',')})`,params);
  await backupAfterChange();
}
export async function update(table:string, rowId:string, values:Record<string,unknown>) {
  if (!allowed.has(table)) throw Error('不支持的数据表');
  const c=await db(); const keys=Object.keys(values);
  await c.execute(`UPDATE ${table} SET ${keys.map((k,i)=>`${k}=$${i+1}`).join(',')} WHERE id=$${keys.length+1}`,[...keys.map(k=>values[k]),rowId]);
  await backupAfterChange();
}
export async function addAssignmentWeek(assignmentId:string, weekNo:number) {
  if(!Number.isInteger(weekNo)||weekNo<1||weekNo>12) throw Error('请选择第 1–12 周');
  const parent=await (await db()).select<{cycle_id:string|null}[]>('SELECT cycle_id FROM week_assignments WHERE id=$1',[assignmentId]);if(!parent[0]?.cycle_id)throw Error('请先指定周期再安排周次');
  await (await db()).execute('INSERT INTO assignment_weeks (assignment_id,week_no) VALUES ($1,$2)',[assignmentId,weekNo]);
  await backupAfterChange();
}
export async function removeAssignmentWeek(assignmentId:string, weekNo:number) {
  await (await db()).execute('DELETE FROM assignment_weeks WHERE assignment_id=$1 AND week_no=$2',[assignmentId,weekNo]);
  await backupAfterChange();
}
export async function updateDayTask(rowId:string, values:{priority:import('./model').Priority|null;date:string;title:string;assignment_id:string|null;project_id:string|null;task_id:string|null;scheduled_start_at:string|null;scheduled_end_at:string|null;scheduled_category_id:string|null}) {
  const c=await db();
  const rows=await c.select<{date:string;recurrence_id:string|null}[]>('SELECT date,recurrence_id FROM day_tasks WHERE id=$1',[rowId]);
  if(!rows.length) throw Error('日任务不存在');
  if(rows[0].recurrence_id && rows[0].date!==values.date) {
    await c.execute('INSERT OR IGNORE INTO recurrence_exceptions (recurrence_id,date) VALUES ($1,$2)',[rows[0].recurrence_id,rows[0].date]);
    await c.execute('UPDATE day_tasks SET date=$1,title=$2,assignment_id=$3,project_id=$4,task_id=$5,scheduled_start_at=$6,scheduled_end_at=$7,scheduled_category_id=$8,priority=$10,long_project_id=CASE WHEN assignment_id IS $3 THEN long_project_id ELSE (SELECT long_project_id FROM week_assignments WHERE id=$3) END,recurrence_id=NULL WHERE id=$9',[values.date,values.title,values.assignment_id,values.project_id,values.task_id,values.scheduled_start_at,values.scheduled_end_at,values.scheduled_category_id,rowId,values.priority]);
  } else {
    await c.execute('UPDATE day_tasks SET date=$1,title=$2,assignment_id=$3,project_id=$4,task_id=$5,scheduled_start_at=$6,scheduled_end_at=$7,scheduled_category_id=$8,priority=$10,long_project_id=CASE WHEN assignment_id IS $3 THEN long_project_id ELSE (SELECT long_project_id FROM week_assignments WHERE id=$3) END WHERE id=$9',[values.date,values.title,values.assignment_id,values.project_id,values.task_id,values.scheduled_start_at,values.scheduled_end_at,values.scheduled_category_id,rowId,values.priority]);
  }
  await backupAfterChange();
}
export async function remove(table:string, rowId:string) {
  if (!allowed.has(table)) throw Error('不支持的数据表');
  await (await db()).execute(`DELETE FROM ${table} WHERE id=$1`,[rowId]); await backupAfterChange();
}
export async function removeAnnualGoal(rowId:string) {
  const c=await db();
  await c.execute('UPDATE projects SET cycle_goal_id=NULL WHERE cycle_goal_id IN (SELECT id FROM cycle_goals WHERE annual_goal_id=$1)',[rowId]);
  await c.execute('UPDATE tasks SET cycle_goal_id=NULL WHERE cycle_goal_id IN (SELECT id FROM cycle_goals WHERE annual_goal_id=$1)',[rowId]);
  await c.execute('UPDATE tasks SET annual_goal_id=NULL WHERE annual_goal_id=$1',[rowId]);
  await c.execute('DELETE FROM cycle_goals WHERE annual_goal_id=$1',[rowId]);
  await c.execute('DELETE FROM annual_goals WHERE id=$1',[rowId]);
  await backupAfterChange();
}
export async function removePlanBlock(rowId:string) {
  const c=await db();
  await c.execute('INSERT OR IGNORE INTO recurrence_exceptions (recurrence_id,date) SELECT recurrence_id,date FROM plan_blocks WHERE id=$1 AND recurrence_id IS NOT NULL',[rowId]);
  await c.execute('DELETE FROM plan_blocks WHERE id=$1',[rowId]);
  await backupAfterChange();
}
export async function removePlanningItem(kind:'task'|'assignment'|'dayTask', rowId:string) {
  const c=await db();
  if(kind==='task') {
    await c.execute('UPDATE day_tasks SET assignment_id=NULL,task_id=NULL,project_id=NULL WHERE assignment_id IN (SELECT id FROM week_assignments WHERE task_id=$1)',[rowId]);
    await c.execute('UPDATE day_tasks SET task_id=NULL,project_id=NULL WHERE task_id=$1',[rowId]);
    await c.execute('UPDATE time_entries SET task_id=NULL WHERE task_id=$1',[rowId]);
    await c.execute('UPDATE recurrences SET task_id=NULL WHERE task_id=$1',[rowId]);
    await c.execute('DELETE FROM assignment_weeks WHERE assignment_id IN (SELECT id FROM week_assignments WHERE task_id=$1)',[rowId]);
    await c.execute('DELETE FROM week_assignments WHERE task_id=$1',[rowId]);
    await c.execute('DELETE FROM tasks WHERE id=$1',[rowId]);
  } else if(kind==='assignment') {
    await c.execute('UPDATE day_tasks SET assignment_id=NULL,task_id=NULL,project_id=NULL WHERE assignment_id=$1',[rowId]);
    await c.execute('DELETE FROM assignment_weeks WHERE assignment_id=$1',[rowId]);
    await c.execute('DELETE FROM week_assignments WHERE id=$1',[rowId]);
  } else {
    await c.execute('INSERT OR IGNORE INTO recurrence_exceptions (recurrence_id,date) SELECT recurrence_id,date FROM day_tasks WHERE id=$1 AND recurrence_id IS NOT NULL',[rowId]);
    await c.execute('UPDATE plan_blocks SET day_task_id=NULL WHERE day_task_id=$1',[rowId]);
    await c.execute('UPDATE time_entries SET day_task_id=NULL WHERE day_task_id=$1',[rowId]);
    await c.execute('DELETE FROM day_tasks WHERE id=$1',[rowId]);
  }
  await backupAfterChange();
}
async function backupAfterChange() { try { await invoke('backup_database',{ manual:false }); } catch (e) { console.error('自动备份失败',e); } }
export async function manualBackup(destination:string) { return invoke<string>('backup_database',{manual:true,destination}); }
export async function listBackups() { return invoke<string[]>('list_backups'); }
export async function stageRestore(path:string) { return invoke('stage_restore',{path}); }
export async function generateRecurrences(data:Data, focusDate=localDate()) {
  const c=await db(); const today=localDate(); let changed=false;
  const exceptions=new Set((await c.select<{recurrence_id:string;date:string}[]>('SELECT recurrence_id,date FROM recurrence_exceptions')).map(x=>`${x.recurrence_id}:${x.date}`));
  const ranges:[[string,string],[string,string]]=[[addDays(today,-7),addDays(today,28)],[addDays(focusDate,-7),addDays(focusDate,7)]];
  for (const rule of data.recurrences.filter(r=>r.active)) {
    for (const [min,max] of ranges) {
      for (let d: string = rule.start_date>min ? rule.start_date : min; d<=max && (!rule.end_date || d<=rule.end_date); d=addDays(d,1)) {
        if (!matches(rule,d) || exceptions.has(`${rule.id}:${d}`)) continue;
        if (rule.kind==='day_task') { const result=await c.execute('INSERT OR IGNORE INTO day_tasks (id,date,title,status,assignment_id,project_id,task_id,recurrence_id,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)',[id(),d,rule.title,'todo',null,rule.project_id,rule.task_id,rule.id,new Date().toISOString()]); changed=changed||result.rowsAffected>0; }
        else if (rule.start_min!==null && rule.end_min!==null) { const result=await c.execute('INSERT OR IGNORE INTO plan_blocks (id,date,start_min,end_min,title,day_task_id,recurrence_id,created_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)',[id(),d,rule.start_min,rule.end_min,rule.title,null,rule.id,new Date().toISOString()]); changed=changed||result.rowsAffected>0; }
      }
    }
  }
  if (changed) await backupAfterChange();
}
function matches(rule:Recurrence,date:string) { return rule.frequency==='daily' || rule.weekdays.split(',').includes(String(new Date(`${date}T12:00:00`).getDay())); }

export async function deleteDirectionRecord(rowId:string) { await invoke('delete_direction',{rowId}); await backupAfterChange(); }
export async function deleteCycleRecord(rowId:string) { await invoke('delete_cycle',{rowId}); await backupAfterChange(); }
export type BatchInsert = { values:Record<string,unknown>; weeks:number[] };
export async function batchCreate(kind:string,rows:BatchInsert[]) { const count=await invoke<number>('batch_create',{kind,rows}); await backupAfterChange(); return count; }

export async function projectAction(entity:string,action:string,rowId:string,values:Record<string,unknown>={},ids:string[]=[],planId:string|null=null,weeks:number[]=[]) { await invoke('project_action',{entity,action,rowId,values,ids,planId,weeks}); await backupAfterChange(); }
