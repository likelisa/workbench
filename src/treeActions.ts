import { cycleWeek, localDate, weekStart, type Data } from './model';
import type { BatchKind } from './batch';
export const childKind=(kind:BatchKind):BatchKind|undefined=>({direction:'annual',annual:'task',task:'assignment',assignment:'dayTask'} as Partial<Record<BatchKind,BatchKind>>)[kind];
export function childDefaults(data:Data,cycleId:string|null,kind:BatchKind,id:string):Record<string,string>{
 if(kind==='direction')return {direction_id:id,year:String(new Date().getFullYear())};
 if(kind==='annual')return {annual_goal_id:id,cycle_id:cycleId||''};
 if(kind==='task')return {task_id:id,cycle_id:cycleId||''};
 const a=data.assignments.find(w=>w.id===id),c=data.cycles.find(c=>c.id===a?.cycle_id),weeks=data.assignmentWeeks.filter(w=>w.assignment_id===id).map(w=>w.week_no).sort((a,b)=>a-b);
 return {assignment_id:id,date:c&&weeks.length?(weeks.includes(cycleWeek(c,localDate()))?localDate():weekStart(c,weeks[0])):localDate()};
}
export function hasTreeChildren(data:Data,cycleId:string|null,kind:BatchKind,id:string):boolean{
 if(kind==='direction')return data.annualGoals.some(g=>g.direction_id===id);
 if(kind==='annual')return data.tasks.some(t=>t.annual_goal_id===id&&t.cycle_id===cycleId);
 if(kind==='task')return data.assignments.some(w=>w.task_id===id);
 if(kind==='assignment')return data.dayTasks.some(d=>d.assignment_id===id);
 return false;
}
