import { localDate, type Data, type Status, type Task, type WeekAssignment } from './model';
export type WeekMultiKey='status'|'parent'|'project'|'priority'|'schedule'|'week';
export type Filters={multi?:Partial<Record<WeekMultiKey,string[]>>;cycle:string;status:string;direction:string;annual:string;project:string;overdue:string;parent:string;priority:string;schedule:string;week:string;group:string;sort:string};
export const defaultFilters=(kind:'task'|'assignment',cycle:string):Filters=>({cycle,status:'',direction:'',annual:'',project:'',overdue:'',parent:'',priority:'',schedule:'',week:'',group:kind==='task'?'annual':'parent',sort:kind==='task'?'created':'priority'});
export function filterTasks(data:Data,f:Filters,query='',today=localDate()):Task[]{
 return data.tasks.filter(t=>(f.cycle==='__all'||(t.cycle_id||'__none')===f.cycle)&&(!f.status||t.status===f.status)&&(!f.direction||data.annualGoals.find(a=>a.id===t.annual_goal_id)?.direction_id===f.direction)&&(!f.annual||t.annual_goal_id===f.annual)&&(!f.project||data.taskProjects.some(p=>p.task_id===t.id&&p.project_id===f.project))&&(!f.overdue||(f.overdue==='yes'?!!t.due_date&&t.due_date<today&&t.status!=='done':!t.due_date||t.due_date>=today||t.status==='done'))&&t.title.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())).sort((a,b)=>f.sort==='name'?a.title.localeCompare(b.title,'zh-CN'):f.sort==='due'?(a.due_date||'9999').localeCompare(b.due_date||'9999')||b.created_at.localeCompare(a.created_at):b.created_at.localeCompare(a.created_at));
}
export function filterWeeks(data:Data,f:Filters,query=''):WeekAssignment[]{
 const weeks=(id:string)=>data.assignmentWeeks.filter(w=>w.assignment_id===id);
 const accepts=(key:WeekMultiKey,value:string)=>{const values=f.multi?.[key]??(f[key]?[f[key]]:[]);return !values.length||values.includes(value);};
 const rank=(w:WeekAssignment)=>w.priority?Number(w.priority.slice(1)):4;
 return data.assignments.filter(w=>(w.cycle_id||'__none')===f.cycle&&accepts('status',w.status)&&accepts('parent',w.task_id||'')&&accepts('project',w.long_project_id||'')&&accepts('priority',w.priority||'__none')&&accepts('schedule',weeks(w.id).length?'scheduled':'none')&&(!(f.multi?.week??(f.week?[f.week]:[])).length||weeks(w.id).some(a=>accepts('week',String(a.week_no))))&&w.commitment.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())).sort((a,b)=>f.sort==='name'?a.commitment.localeCompare(b.commitment,'zh-CN'):f.sort==='priority'?rank(a)-rank(b)||b.created_at.localeCompare(a.created_at):b.created_at.localeCompare(a.created_at));
}
export const statusCounts=(items:{status:Status}[])=>Object.fromEntries(['todo','doing','done','missed'].map(status=>[status,items.filter(i=>i.status===status).length]));
export const taskPage=(items:{id:string}[],id:string)=>Math.max(1,Math.floor(items.findIndex(t=>t.id===id)/50)+1);
