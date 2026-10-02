import { addDays, assignmentInWeek, cycleWeek, localDate, type Cycle, type Data, type DayTask, type Task, type WeekAssignment } from './model';
import { statusCounts } from './taskQueries';
export type OverdueItem = { kind:'dayTask'|'assignment'|'task'; id:string; title:string; status:DayTask['status']; priority?:DayTask['priority']; date:string; cycleId:string|null; days:number };
export const elapsedDays=(from:string,to:string)=>Math.round((Date.parse(`${to}T00:00:00Z`)-Date.parse(`${from}T00:00:00Z`))/86400000);
export function lastAssignmentDate(data:Data,item:WeekAssignment):string|null {
 const cycle=data.cycles.find(c=>c.id===item.cycle_id),weeks=data.assignmentWeeks.filter(w=>w.assignment_id===item.id&&w.week_no>=1&&w.week_no<=12).map(w=>w.week_no);
 return cycle&&weeks.length?addDays(cycle.start_date,Math.max(...weeks)*7-1):null;
}
export function overdueTasks(data:Data,today=localDate()):OverdueItem[]{
 const days=data.dayTasks.filter(d=>d.date<today&&d.status!=='done').map(d=>({kind:'dayTask' as const,id:d.id,title:d.title,status:d.status,priority:d.priority,date:d.date,cycleId:data.assignments.find(a=>a.id===d.assignment_id)?.cycle_id||data.tasks.find(t=>t.id===d.task_id)?.cycle_id||null,days:elapsedDays(d.date,today)}));
 const weeks=data.assignments.flatMap(w=>{const date=lastAssignmentDate(data,w);return date&&date<today&&w.status!=='done'?[{kind:'assignment' as const,id:w.id,title:w.commitment,status:w.status,priority:w.priority,date,cycleId:w.cycle_id,days:elapsedDays(date,today)}]:[];});
 const tasks=data.tasks.filter(t=>t.due_date&&t.due_date<today&&t.status!=='done').map(t=>({kind:'task' as const,id:t.id,title:t.title,status:t.status,date:t.due_date!,cycleId:t.cycle_id,days:elapsedDays(t.due_date!,today)}));
 return [...days,...weeks,...tasks].sort((a,b)=>b.date.localeCompare(a.date)||a.title.localeCompare(b.title,'zh-CN'));
}
export function homeSummary(data:Data,cycle:Cycle|undefined,today=localDate()) {
 const week=cycle?cycleWeek(cycle,today):0;
 const tasks:Task[]=cycle?data.tasks.filter(t=>t.cycle_id===cycle.id):[];
 const weeks=cycle?data.assignments.filter(w=>w.cycle_id===cycle.id):[];
 const passed=cycle?elapsedDays(cycle.start_date,today):0;
 return {today:data.dayTasks.filter(d=>d.date===today).sort((a,b)=>(a.scheduled_start_at||'9999').localeCompare(b.scheduled_start_at||'9999')||(a.priority||'P4').localeCompare(b.priority||'P4')||a.created_at.localeCompare(b.created_at)),thisWeek:week>=1&&week<=12?weeks.filter(w=>assignmentInWeek(data,w.id,week)).sort((a,b)=>(a.priority||'P4').localeCompare(b.priority||'P4')||b.created_at.localeCompare(a.created_at)):[],tasks,weeks,counts:statusCounts(tasks),weekDone:weeks.filter(w=>w.status==='done').length,unscheduled:weeks.filter(w=>!data.assignmentWeeks.some(a=>a.assignment_id===w.id)).length,week,phase:passed<0?'future':passed>=84?'ended':'active',remaining:Math.max(0,84-Math.max(0,passed)),timePercent:Math.min(100,Math.max(0,passed/84*100))};
}
