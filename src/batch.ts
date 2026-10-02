import { validCategory } from './timeCategories';
import { assignmentsForDate, id, validateTimeEntry, type Data, type TimeEntry } from './model';
import type { BatchInsert } from './db';
export type BatchKind='direction'|'annual'|'task'|'assignment'|'dayTask';
export type InputRow=Record<string,string>;
export class RowError extends Error{constructor(public field:string,message:string){super(message);}}
export function prepareRow(kind:BatchKind,v:InputRow,data:Data,entries:TimeEntry[]=data.timeEntries):BatchInsert{
 const required=(key:string,label:string)=>{if(!v[key]?.trim())throw new RowError(key,`请填写${label}`);};
 const has=(key:string,items:{id:string}[],label:string)=>{required(key,label);if(!items.some(item=>item.id===v[key]))throw new RowError(key,`所选${label}不存在`);};
 const values:Record<string,unknown>={id:id(),created_at:new Date().toISOString()};let weeks:number[]=[];
 if(kind==='assignment'||kind==='dayTask'){if(v.priority&&!['P0','P1','P2','P3'].includes(v.priority))throw new RowError('priority','紧急程度必须为 P0–P3 或留空');values.priority=v.priority||null;}
 required(kind==='assignment'?'commitment':'title','内容');
 if(kind==='direction')Object.assign(values,{title:v.title.trim()});
 if(kind==='annual'){has('direction_id',data.directions,'年度方向');required('year','年份');const year=Number(v.year);if(!Number.isInteger(year)||year<1||year>9999)throw new RowError('year','年份无效');Object.assign(values,{title:v.title.trim(),year,direction_id:v.direction_id,completed:0,status:'todo'});}
 if(kind==='task'){has('annual_goal_id',data.annualGoals,'年度目标');has('cycle_id',data.cycles,'周期');if(v.due_date&&!/^\d{4}-\d{2}-\d{2}$/.test(v.due_date))throw new RowError('due_date','截止日期无效');Object.assign(values,{title:v.title.trim(),annual_goal_id:v.annual_goal_id,cycle_id:v.cycle_id,due_date:v.due_date||null,status:'todo'});}
 if(kind==='assignment'){has('task_id',data.tasks,'本轮待办');const task=data.tasks.find(item=>item.id===v.task_id)!;if(!task.cycle_id)throw new RowError('task_id','请先为本轮待办指定周期');weeks=v.weeks?.trim()?v.weeks.split(/[,，、\s]+/).filter(Boolean).map(Number):[];if(weeks.some(week=>!Number.isInteger(week)||week<1||week>12)||new Set(weeks).size!==weeks.length)throw new RowError('weeks','周次须为不重复的 1–12');Object.assign(values,{task_id:task.id,cycle_id:task.cycle_id,commitment:v.commitment.trim(),status:'todo',review:''});}
 if(kind==='dayTask'){
  required('date','日期');const date=new Date(`${v.date}T12:00:00`);if(!Number.isFinite(date.getTime())||v.date!==`${date.getFullYear()}-${String(date.getMonth()+1).padStart(2,'0')}-${String(date.getDate()).padStart(2,'0')}`)throw new RowError('date','日期无效');
  const assignment=assignmentsForDate(data,v.date).find(item=>item.id===v.assignment_id);if(v.assignment_id&&!assignment)throw new RowError('assignment_id','日任务日期所在周未安排此周任务');
  const task=data.tasks.find(item=>item.id===assignment?.task_id);let start:string|null=null,end:string|null=null,category:string|null=null;
  if(v.start||v.end||v.category_id){required('start','开始时间');required('end','结束时间');has('category_id',data.categories,'分类');if(!validCategory(data,v.category_id))throw new RowError('category_id','请选择有效的固定类别或年度目标');if(!/^([01]\d|2[0-3]):[0-5]\d$/.test(v.start)||!/^([01]\d|2[0-3]):[0-5]\d$/.test(v.end)||v.start===v.end)throw new RowError('end','请填写有效且不同的开始、结束时间');const a=new Date(`${v.date}T${v.start}:00`),b=new Date(`${v.date}T${v.end}:00`);if(b<a)b.setDate(b.getDate()+1);start=a.toISOString();end=b.toISOString();category=v.category_id;const error=validateTimeEntry(entries,start,end);if(error)throw new RowError('start',error);}
  Object.assign(values,{date:v.date,title:v.title.trim(),status:'todo',assignment_id:assignment?.id||null,task_id:task?.id||null,project_id:task?.project_id||null,scheduled_start_at:start,scheduled_end_at:end,scheduled_category_id:category});
 }
 return {values,weeks};
}
