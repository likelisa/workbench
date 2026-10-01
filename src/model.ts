export type Status = 'todo' | 'doing' | 'done' | 'missed';
export type Cycle = { id: string; title: string; start_date: string; created_at: string; review: string };
export type Direction = { id: string; title: string; created_at: string };
export type AnnualGoal = { id: string; direction_id: string | null; title: string; year: number; completed: number; created_at: string };
export type Task = { id: string; cycle_goal_id: string | null; project_id: string | null; cycle_id: string | null; annual_goal_id: string | null; title: string; status: Status; due_date: string | null; created_at: string };
export type WeekAssignment = { id: string; task_id: string; cycle_id: string | null; commitment: string; status: Status; review: string; created_at: string };
export type AssignmentWeek = { assignment_id: string; week_no: number };
export type DayTask = { id: string; date: string; title: string; status: Status; assignment_id: string | null; project_id: string | null; task_id: string | null; recurrence_id: string | null; scheduled_start_at: string | null; scheduled_end_at: string | null; scheduled_category_id: string | null; created_at: string };
export type Category = { id: string; name: string; color: string; created_at: string };
export type PlanBlock = { id: string; date: string; start_min: number; end_min: number; title: string; day_task_id: string | null; recurrence_id: string | null; created_at: string };
export type DailyPlanTemplate = { id: string; start_min: number; end_min: number; title: string; created_at: string };
export type TimeEntry = { id: string; start_at: string; end_at: string; title: string; category_id: string; day_task_id: string | null; project_id: string | null; task_id: string | null; created_at: string };
export type Recurrence = { id: string; kind: 'day_task' | 'plan'; frequency: 'daily' | 'weekly'; weekdays: string; start_date: string; end_date: string | null; title: string; start_min: number | null; end_min: number | null; project_id: string | null; task_id: string | null; category_id: string | null; active: number; created_at: string };
export type Data = { cycles: Cycle[]; directions: Direction[]; annualGoals: AnnualGoal[]; tasks: Task[]; assignments: WeekAssignment[]; assignmentWeeks: AssignmentWeek[]; dayTasks: DayTask[]; categories: Category[]; planBlocks: PlanBlock[]; dailyPlanTemplates: DailyPlanTemplate[]; timeEntries: TimeEntry[]; recurrences: Recurrence[] };
export const EMPTY: Data = { cycles: [], directions: [], annualGoals: [], tasks: [], assignments: [], assignmentWeeks: [], dayTasks: [], categories: [], planBlocks: [], dailyPlanTemplates: [], timeEntries: [], recurrences: [] };
export const STATUS: Record<Status, string> = { todo: '待办', doing: '进行中', done: '已完成', missed: '未完成' };
export const id = () => crypto.randomUUID();
export const localDate = (date = new Date()) => `${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`;
export const addDays = (date: string, days: number) => { const d = new Date(`${date}T12:00:00`); d.setDate(d.getDate()+days); return localDate(d); };
export const minuteLabel = (min: number) => `${String(Math.floor(min/60)).padStart(2,'0')}:${String(min%60).padStart(2,'0')}`;
export const parseMinute = (time: string) => { const [h,m] = time.split(':').map(Number); return h*60+m; };
export const dayTaskTimeLabel = (task: DayTask) => {
  if (!task.scheduled_start_at || !task.scheduled_end_at) return '';
  const start = new Date(task.scheduled_start_at), end = new Date(task.scheduled_end_at);
  const clock = (value: Date) => minuteLabel(value.getHours() * 60 + value.getMinutes());
  return `${clock(start)}—${clock(end)}${localDate(end) !== localDate(start) ? '（次日）' : ''}`;
};
export const cycleWeek = (cycle: Cycle, date: string) => Math.floor((new Date(`${date}T12:00:00`).getTime()-new Date(`${cycle.start_date}T12:00:00`).getTime())/86400000/7)+1;
export const weekStart = (cycle: Cycle, week: number) => addDays(cycle.start_date, (week-1)*7);
export const assignmentInWeek = (data:Data, assignmentId:string, weekNo:number) => data.assignmentWeeks.some(item=>item.assignment_id===assignmentId && item.week_no===weekNo);
export const assignmentsForDate = (data:Data, date:string) => data.assignments.filter(item=>{
  const cycle=data.cycles.find(c=>c.id===item.cycle_id);
  return cycle && assignmentInWeek(data,item.id,cycleWeek(cycle,date));
});
export type CalendarMode = 'day' | 'week' | 'month' | 'year';
export const mondayOf = (date: string) => addDays(date,-((new Date(`${date}T12:00:00`).getDay()+6)%7));
export const calendarRange = (date:string,mode:CalendarMode):[string,string] => {
  if(mode==='day') return [date,addDays(date,1)];
  if(mode==='week') { const from=mondayOf(date); return [from,addDays(from,7)]; }
  if(mode==='month') { const from=`${date.slice(0,7)}-01`; return [from,localDate(new Date(Number(date.slice(0,4)),Number(date.slice(5,7)),1))]; }
  return [`${date.slice(0,4)}-01-01`,`${Number(date.slice(0,4))+1}-01-01`];
};
export const shiftCalendarDate = (date:string,mode:CalendarMode,step:number) => {
  if(mode==='day') return addDays(date,step);
  if(mode==='week') return addDays(date,step*7);
  if(mode==='month') return localDate(new Date(Number(date.slice(0,4)),Number(date.slice(5,7))-1+step,1));
  return `${Number(date.slice(0,4))+step}-01-01`;
};
export const assignmentsInRange = (data:Data,from:string,to:string) => data.assignments.filter(assignment=>{
  const cycle=data.cycles.find(item=>item.id===assignment.cycle_id);
  if(!cycle) return false;
  return data.assignmentWeeks.some(item=>{
    if(item.assignment_id!==assignment.id) return false;
    const start=weekStart(cycle,item.week_no);
    return start<to && addDays(start,7)>from;
  });
});
export const progress = (tasks: Task[]) => tasks.length ? { done: tasks.filter(t=>t.status==='done').length, total: tasks.length, percent: Math.round(tasks.filter(t=>t.status==='done').length/tasks.length*100) } : null;
export const annualGoalTasks = (data: Data, annualGoal: AnnualGoal) => data.tasks.filter(t=>t.annual_goal_id===annualGoal.id);
export const cycleEndDate = (cycle: Cycle) => addDays(cycle.start_date,83);
export const historyYearGroups = (data: Data, today=localDate()) => {
  const currentYear=Number(today.slice(0,4));
  const endedCycles=data.cycles.filter(c=>cycleEndDate(c)<today);
  const assignmentIds=new Set(data.assignments.map(a=>a.id));
  const unlinkedDayTasks=data.dayTasks.filter(t=>t.date<today && (!t.assignment_id || !assignmentIds.has(t.assignment_id)));
  const years=[...new Set([currentYear,...data.annualGoals.filter(g=>g.year<=currentYear).map(g=>g.year),...endedCycles.map(c=>Number(cycleEndDate(c).slice(0,4))),...unlinkedDayTasks.map(t=>Number(t.date.slice(0,4)))])].sort((a,b)=>b-a);
  return years.map(year=>({
    year,
    annualGoals:data.annualGoals.filter(g=>g.year===year),
    cycles:endedCycles.filter(c=>Number(cycleEndDate(c).slice(0,4))===year).sort((a,b)=>cycleEndDate(b).localeCompare(cycleEndDate(a))),
    unlinkedDayTasks:unlinkedDayTasks.filter(t=>Number(t.date.slice(0,4))===year).sort((a,b)=>b.date.localeCompare(a.date))
  }));
};
export const dayTaskProjectId = (data: Data, dayTask: DayTask) => dayTask.project_id || data.tasks.find(t=>t.id===data.assignments.find(a=>a.id===dayTask.assignment_id)?.task_id)?.project_id || null;
export const dayTaskTaskId = (data: Data, dayTask: DayTask) => {
  if(dayTask.task_id) return dayTask.task_id;
  const assignmentTaskId=data.assignments.find(a=>a.id===dayTask.assignment_id)?.task_id;
  if(assignmentTaskId) return assignmentTaskId;
  const legacyMatches=dayTask.project_id ? data.tasks.filter(t=>t.project_id===dayTask.project_id) : [];
  return legacyMatches.length===1 ? legacyMatches[0].id : null;
};
export const minutesForEntryInDay = (entry: TimeEntry, day: string) => { const start = new Date(entry.start_at).getTime(); const end = new Date(entry.end_at).getTime(); const lo = new Date(`${day}T00:00:00`).getTime(); const hi = new Date(`${addDays(day,1)}T00:00:00`).getTime(); return Math.max(0, Math.min(end,hi)-Math.max(start,lo))/60000; };
export const categoryTotals = (entries: TimeEntry[], categories: Category[], from: string, toExclusive: string) => {
  const days: string[] = []; for (let d=from; d<toExclusive; d=addDays(d,1)) days.push(d);
  const totals = categories.map(category => ({ category, minutes: entries.filter(e=>e.category_id===category.id).reduce((sum,e)=>sum+days.reduce((n,d)=>n+minutesForEntryInDay(e,d),0),0) }));
  const total = totals.reduce((sum,item)=>sum+item.minutes,0);
  return { total, rows: totals.filter(x=>x.minutes>0).sort((a,b)=>b.minutes-a.minutes).map(x=>({ ...x, percent: total ? x.minutes/total*100 : 0 })) };
};
export const validateTimeEntry = (entries: TimeEntry[], startAt: string, endAt: string, exceptId?: string) => {
  const a = new Date(startAt).getTime(), b = new Date(endAt).getTime();
  if (!Number.isFinite(a) || !Number.isFinite(b) || b<=a) return '结束时间必须晚于开始时间';
  if (entries.some(e=>e.id!==exceptId && a<new Date(e.end_at).getTime() && b>new Date(e.start_at).getTime())) return '这段时间与已有实际记录重叠';
  return null;
};
export const formatHours = (minutes: number) => `${(minutes/60).toFixed(1)} 小时`;
