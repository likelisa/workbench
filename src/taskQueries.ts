import { localDate, type Data, type Status, type Task, type WeekAssignment } from './model';
export type Filters={cycle:string;status:string;direction:string;annual:string;project:string;overdue:string;parent:string;priority:string;schedule:string;week:string;group:string;sort:string};
export const defaultFilters=(kind:'task'|'assignment',cycle:string):Filters=>({cycle,status:'',direction:'',annual:'',project:'',overdue:'',parent:'',priority:'',schedule:'',week:'',group:kind==='task'?'annual':'parent',sort:kind==='task'?'created':'priority'});
export function filterTasks(data:Data,f:Filters,query='',today=localDate()):Task[]{
 return data.tasks.filter(t=>(f.cycle==='__all'||(t.cycle_id||'__none')===f.cycle)&&(!f.status||t.status===f.status)&&(!f.direction||data.annualGoals.find(a=>a.id===t.annual_goal_id)?.direction_id===f.direction)&&(!f.annual||t.annual_goal_id===f.annual)&&(!f.project||data.taskProjects.some(p=>p.task_id===t.id&&p.project_id===f.project))&&(!f.overdue||(f.overdue==='yes'?!!t.due_date&&t.due_date<today&&t.status!=='done':!t.due_date||t.due_date>=today||t.status==='done'))&&t.title.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())).sort((a,b)=>f.sort==='name'?a.title.localeCompare(b.title,'zh-CN'):f.sort==='due'?(a.due_date||'9999').localeCompare(b.due_date||'9999')||b.created_at.localeCompare(a.created_at):b.created_at.localeCompare(a.created_at));
}
export function filterWeeks(data:Data,f:Filters,query=''):WeekAssignment[]{
 const weeks=(id:string)=>data.assignmentWeeks.filter(w=>w.assignment_id===id);
 const rank=(w:WeekAssignment)=>w.priority?Number(w.priority.slice(1)):4;
 return data.assignments.filter(w=>(w.cycle_id||'__none')===f.cycle&&(!f.status||w.status===f.status)&&(!f.parent||w.task_id===f.parent)&&(!f.project||w.long_project_id===f.project)&&(!f.priority||(w.priority||'__none')===f.priority)&&(!f.schedule||(f.schedule==='none'?weeks(w.id).length===0:weeks(w.id).length>0))&&(!f.week||weeks(w.id).some(a=>a.week_no===Number(f.week)))&&w.commitment.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())).sort((a,b)=>f.sort==='name'?a.commitment.localeCompare(b.commitment,'zh-CN'):f.sort==='priority'?rank(a)-rank(b)||b.created_at.localeCompare(a.created_at):b.created_at.localeCompare(a.created_at));
}
export const statusCounts=(items:{status:Status}[])=>Object.fromEntries(['todo','doing','done','missed'].map(status=>[status,items.filter(i=>i.status===status).length]));
export const taskPage=(items:{id:string}[],id:string)=>Math.max(1,Math.floor(items.findIndex(t=>t.id===id)/50)+1);
