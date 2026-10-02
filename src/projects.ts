import { localDate, type Data } from './model';
export type LongProject={id:string;title:string;type_id:string|null;description:string;focus:string;links:string;status:'preparing'|'active'|'paused'|'archived';favorite:number;created_at:string;updated_at:string};
export type ProjectType={id:string;title:string;sort_order:number};
export type ProjectStage={id:string;project_id:string;title:string;description:string;start_date:string|null;end_date:string|null;status:'planned'|'active'|'done';created_at:string};
export type ProjectPlan={id:string;project_id:string;stage_id:string|null;title:string;description:string;horizon:'now'|'next'|'later';completed:number;created_at:string};
export type ProjectUpdate={id:string;project_id:string;date:string;content:string;created_at:string};
export type ProjectData={longProjects:LongProject[];projectTypes:ProjectType[];taskProjects:{task_id:string;project_id:string}[];projectStages:ProjectStage[];projectPlans:ProjectPlan[];projectUpdates:ProjectUpdate[];planTasks:{plan_id:string;task_id:string}[];planWeeks:{plan_id:string;assignment_id:string}[]};
export const EMPTY_PROJECTS:ProjectData={longProjects:[],projectTypes:[],taskProjects:[],projectStages:[],projectPlans:[],projectUpdates:[],planTasks:[],planWeeks:[]};
export const PROJECT_STATUS={preparing:'筹备中',active:'推进中',paused:'暂停',archived:'已归档'};
export const STAGE_STATUS={planned:'计划中',active:'进行中',done:'已完成'};
export const HORIZON={now:'现在',next:'接下来',later:'以后'};
export type ProjectFilters={query:string;types:string[];statuses:string[];directions:string[];goals:string[];cycles:string[];favorite:boolean;doing:boolean;overdue:boolean;sort:'updated'|'name'|'created'};
export const DEFAULT_FILTERS:ProjectFilters={query:'',types:[],statuses:['preparing','active','paused'],directions:[],goals:[],cycles:[],favorite:false,doing:false,overdue:false,sort:'updated'};
export function projectTasks(data:Data,projectId:string){const ids=new Set(data.taskProjects.filter(x=>x.project_id===projectId).map(x=>x.task_id));return data.tasks.filter(t=>ids.has(t.id));}
export function projectWeeks(data:Data,projectId:string){return data.assignments.filter(w=>w.long_project_id===projectId);}
export function filterProjects(data:Data,f:ProjectFilters,today=localDate()){
 return data.longProjects.filter(p=>{
  if(f.query&&!`${p.title}\n${p.description}`.toLocaleLowerCase().includes(f.query.toLocaleLowerCase()))return false;
  if(f.types.length&&!f.types.includes(p.type_id||'__none'))return false;
  if(f.statuses.length&&!f.statuses.includes(p.status))return false;
  if(f.favorite&&!p.favorite)return false;
  const tasks=projectTasks(data,p.id);
  if((f.directions.length||f.goals.length||f.cycles.length)&&!tasks.some(t=>{
   const goal=data.annualGoals.find(g=>g.id===t.annual_goal_id);
   return (!f.directions.length||f.directions.includes(goal?.direction_id||''))&&(!f.goals.length||f.goals.includes(goal?.id||''))&&(!f.cycles.length||f.cycles.includes(t.cycle_id||''));
  }))return false;
  if(f.doing&&!projectWeeks(data,p.id).some(w=>w.status==='doing'))return false;
  if(f.overdue&&!tasks.some(t=>t.due_date&&t.due_date<today&&t.status!=='done'))return false;
  return true;
 }).sort((a,b)=>Number(b.favorite)-Number(a.favorite)||(f.sort==='name'?a.title.localeCompare(b.title,'zh-CN'):f.sort==='created'?b.created_at.localeCompare(a.created_at):Date.parse(b.updated_at)-Date.parse(a.updated_at)));
}
export function projectStats(data:Data,projectId:string){
 const tasks=projectTasks(data,projectId),weeks=projectWeeks(data,projectId),days=data.dayTasks.filter(d=>d.long_project_id===projectId),entries=data.timeEntries.filter(e=>e.long_project_id===projectId);
 return {tasks,weeks,days,entries,minutes:entries.reduce((n,e)=>n+Math.max(0,(Date.parse(e.end_at)-Date.parse(e.start_at))/60000),0)};
}
export function planExecution(data:Data,planId:string){
 const tasks=new Set(data.planTasks.filter(x=>x.plan_id===planId).map(x=>x.task_id)),weeks=new Set(data.planWeeks.filter(x=>x.plan_id===planId).map(x=>x.assignment_id));
 return {tasks:data.tasks.filter(t=>tasks.has(t.id)),weeks:data.assignments.filter(w=>weeks.has(w.id))};
}
