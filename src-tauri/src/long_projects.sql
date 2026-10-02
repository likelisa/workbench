CREATE TABLE IF NOT EXISTS project_types(id TEXT PRIMARY KEY,title TEXT NOT NULL UNIQUE,sort_order INTEGER NOT NULL DEFAULT 0);
CREATE TABLE IF NOT EXISTS long_projects(id TEXT PRIMARY KEY,title TEXT NOT NULL,type_id TEXT REFERENCES project_types(id) ON DELETE SET NULL,description TEXT NOT NULL DEFAULT '',focus TEXT NOT NULL DEFAULT '',links TEXT NOT NULL DEFAULT '',status TEXT NOT NULL DEFAULT 'preparing' CHECK(status IN ('preparing','active','paused','archived')),favorite INTEGER NOT NULL DEFAULT 0,created_at TEXT NOT NULL,updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS task_projects(task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,project_id TEXT NOT NULL REFERENCES long_projects(id) ON DELETE CASCADE,PRIMARY KEY(task_id,project_id));
CREATE TABLE IF NOT EXISTS project_stages(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES long_projects(id) ON DELETE CASCADE,title TEXT NOT NULL,description TEXT NOT NULL DEFAULT '',start_date TEXT,end_date TEXT,status TEXT NOT NULL DEFAULT 'planned' CHECK(status IN ('planned','active','done')),created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS project_plans(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES long_projects(id) ON DELETE CASCADE,stage_id TEXT REFERENCES project_stages(id) ON DELETE SET NULL,title TEXT NOT NULL,description TEXT NOT NULL DEFAULT '',horizon TEXT NOT NULL DEFAULT 'later' CHECK(horizon IN ('now','next','later')),completed INTEGER NOT NULL DEFAULT 0,created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS project_updates(id TEXT PRIMARY KEY,project_id TEXT NOT NULL REFERENCES long_projects(id) ON DELETE CASCADE,date TEXT NOT NULL,content TEXT NOT NULL,created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS plan_tasks(plan_id TEXT NOT NULL REFERENCES project_plans(id) ON DELETE CASCADE,task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,PRIMARY KEY(plan_id,task_id));
CREATE TABLE IF NOT EXISTS plan_weeks(plan_id TEXT NOT NULL REFERENCES project_plans(id) ON DELETE CASCADE,assignment_id TEXT NOT NULL REFERENCES week_assignments(id) ON DELETE CASCADE,PRIMARY KEY(plan_id,assignment_id));
CREATE TABLE IF NOT EXISTS app_migrations(name TEXT PRIMARY KEY);
INSERT OR IGNORE INTO project_types SELECT 'product','产品',0 WHERE NOT EXISTS(SELECT 1 FROM app_migrations WHERE name='long_projects_seed');
INSERT OR IGNORE INTO project_types SELECT 'media','自媒体',1 WHERE NOT EXISTS(SELECT 1 FROM app_migrations WHERE name='long_projects_seed');
INSERT OR IGNORE INTO project_types SELECT 'course','课程与培训',2 WHERE NOT EXISTS(SELECT 1 FROM app_migrations WHERE name='long_projects_seed');
INSERT OR IGNORE INTO app_migrations VALUES('long_projects_seed');
CREATE INDEX IF NOT EXISTS idx_task_projects_project ON task_projects(project_id);
CREATE INDEX IF NOT EXISTS idx_project_plans_project ON project_plans(project_id);
CREATE INDEX IF NOT EXISTS idx_week_long_project ON week_assignments(long_project_id);
CREATE INDEX IF NOT EXISTS idx_entry_long_project ON time_entries(long_project_id);
-- Assignment projects are explicitly chosen, never inferred from a task's projects.
CREATE TRIGGER IF NOT EXISTS validate_week_project_insert BEFORE INSERT ON week_assignments WHEN NEW.long_project_id IS NOT NULL BEGIN
 SELECT RAISE(ABORT,'所选项目不属于该本轮待办') WHERE NOT EXISTS(SELECT 1 FROM task_projects WHERE task_id=NEW.task_id AND project_id=NEW.long_project_id);
END;
CREATE TRIGGER IF NOT EXISTS validate_week_project_update BEFORE UPDATE OF task_id,long_project_id ON week_assignments WHEN NEW.long_project_id IS NOT NULL AND (NEW.task_id IS NOT OLD.task_id OR NEW.long_project_id IS NOT OLD.long_project_id) BEGIN
 SELECT RAISE(ABORT,'所选项目不属于该本轮待办') WHERE NOT EXISTS(SELECT 1 FROM task_projects WHERE task_id=NEW.task_id AND project_id=NEW.long_project_id);
END;
CREATE TRIGGER IF NOT EXISTS inherit_day_project_insert AFTER INSERT ON day_tasks BEGIN
 UPDATE day_tasks SET long_project_id=(SELECT long_project_id FROM week_assignments WHERE id=NEW.assignment_id) WHERE id=NEW.id;
 UPDATE time_entries SET long_project_id=(SELECT long_project_id FROM week_assignments WHERE id=NEW.assignment_id) WHERE day_task_id=NEW.id;
END;
-- Unlinking deleted tasks preserves historical project attribution.
CREATE TRIGGER IF NOT EXISTS inherit_day_project_update AFTER UPDATE OF assignment_id ON day_tasks WHEN NEW.assignment_id IS NOT OLD.assignment_id AND NEW.assignment_id IS NOT NULL BEGIN
 UPDATE day_tasks SET long_project_id=(SELECT long_project_id FROM week_assignments WHERE id=NEW.assignment_id) WHERE id=NEW.id;
 UPDATE time_entries SET long_project_id=(SELECT long_project_id FROM week_assignments WHERE id=NEW.assignment_id) WHERE day_task_id=NEW.id;
END;
CREATE TRIGGER IF NOT EXISTS inherit_week_project_update AFTER UPDATE OF long_project_id ON week_assignments WHEN NEW.long_project_id IS NOT OLD.long_project_id BEGIN
 UPDATE day_tasks SET long_project_id=NEW.long_project_id WHERE assignment_id=NEW.id;
 UPDATE time_entries SET long_project_id=NEW.long_project_id WHERE day_task_id IN(SELECT id FROM day_tasks WHERE assignment_id=NEW.id);
 -- A plan-week link must always stay within its project.
 DELETE FROM plan_weeks WHERE assignment_id=NEW.id AND plan_id IN(SELECT id FROM project_plans WHERE project_id IS NOT NEW.long_project_id);
END;
CREATE TRIGGER IF NOT EXISTS inherit_entry_project_insert AFTER INSERT ON time_entries WHEN NEW.day_task_id IS NOT NULL BEGIN
 UPDATE time_entries SET long_project_id=(SELECT long_project_id FROM day_tasks WHERE id=NEW.day_task_id) WHERE id=NEW.id;
END;
CREATE TRIGGER IF NOT EXISTS inherit_entry_project_update AFTER UPDATE OF day_task_id ON time_entries WHEN NEW.day_task_id IS NOT OLD.day_task_id AND NEW.day_task_id IS NOT NULL BEGIN
 UPDATE time_entries SET long_project_id=(SELECT long_project_id FROM day_tasks WHERE id=NEW.day_task_id) WHERE id=NEW.id;
END;
CREATE TRIGGER IF NOT EXISTS sync_explicit_day_project AFTER UPDATE OF long_project_id ON day_tasks WHEN NEW.long_project_id IS NOT OLD.long_project_id BEGIN
 UPDATE time_entries SET long_project_id=NEW.long_project_id WHERE day_task_id=NEW.id;
END;
