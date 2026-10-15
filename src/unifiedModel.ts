import { addDays, cycleEndDate, mondayOf, weekStart, type Data, type Priority, type Status, type TimeEntry } from './model';
export type Item = {id:string;title:string;type:'requirement'|'bug'|'todo'|'idea';status:Status;priority:Priority|null;description:string;project_id:string|null;parent_id:string|null;annual_goal_id:string|null;cycle_goal_id:string|null;cycle_id:string|null;level:'backlog'|'cycle'|'week'|'day';version_id:string|null;milestone_id:string|null;created_at:string};
export type Placement = {id:string;item_id:string;kind:'day'|'week';date:string|null;week_start:string|null;cycle_id:string|null;week_no:number|null;created_at:string};
export type CycleGoal = {id:string;title:string;cycle_id:string;annual_goal_id:string;source_item_id?:string|null;created_at:string};
export type ProjectVersion = {id:string;project_id:string;title:string;description:string;created_at:string};
export type UnifiedData = {taskProjectItems:{task_id:string;project_item_id:string}[];items:Item[];placements:Placement[];cycleGoals:CycleGoal[];projectVersions:ProjectVersion[];itemProjects:{item_id:string;project_id:string}[]};
export const EMPTY_UNIFIED:UnifiedData={taskProjectItems:[],items:[],placements:[],cycleGoals:[],projectVersions:[],itemProjects:[]};
export const ITEM_TYPES={requirement:'需求',bug:'Bug',todo:'待办',idea:'想法'};
const order:Record<Status,number>={doing:0,todo:1,missed:2,done:3};
export const sortItems=(items:Item[])=>[...items].sort((a,b)=>order[a.status]-order[b.status]||(a.priority||'P2').localeCompare(b.priority||'P2')||a.created_at.localeCompare(b.created_at)||a.id.localeCompare(b.id));
export function taskItems(data:Data){const goals=new Set(data.cycleGoals.map(g=>g.source_item_id).filter(Boolean));return data.items.filter(i=>!goals.has(i.id));}
export function cycleTodos(data:Data,cycleId?:string,excludeId?:string){return sortItems(taskItems(data).filter(i=>i.level==='cycle'&&i.id!==excludeId&&(!cycleId||i.cycle_id===cycleId)));}
export function weekTaskValues(data:Data,values:Record<string,unknown>){const parent=cycleTodos(data).find(i=>i.id===values.parent_id);if(!parent)return {...values,parent_id:null,annual_goal_id:null,cycle_goal_id:null};const inherited=itemHierarchy(data,parent.id);return {...values,parent_id:parent.id,cycle_id:inherited.cycle_id,annual_goal_id:inherited.annual_goal_id,cycle_goal_id:inherited.cycle_goal_id};}
export function itemsOnDay(data:Data,date:string){const ids=new Set(data.placements.filter(p=>p.kind==='day'&&p.date===date).map(p=>p.item_id));return sortItems(taskItems(data).filter(i=>ids.has(i.id)));}
export function itemsInWeek(data:Data,date:string){const start=mondayOf(date),end=addDays(start,7);const ids=new Set(data.placements.filter(p=>p.kind==='week'&&p.week_start!<end&&addDays(p.week_start!,7)>start).map(p=>p.item_id));return sortItems(taskItems(data).filter(i=>ids.has(i.id)));}
// Keep the execution track aligned with the cycle's own week boundaries, including older midweek starts.
export function executionWeeks(data:Data,today:string){
 const cycle=activeCycle(data,today),start=cycle?.start_date||addDays(mondayOf(today),-28);
 return Array.from({length:12},(_,n)=>{
  const from=cycle?weekStart(cycle,n+1):addDays(start,n*7),to=addDays(from,6);
  const ids=new Set(data.placements.filter(p=>p.kind==='week'&&(cycle&&p.cycle_id?p.cycle_id===cycle.id&&p.week_no===n+1:p.week_start!<=to&&addDays(p.week_start!,6)>=from)).map(p=>p.item_id));
  return {id:`${cycle?.id||'independent'}:${from}`,week:cycle?n+1:null,cycle_id:cycle?.id||null,start:from,end:to,current:from<=today&&to>=today,items:sortItems(taskItems(data).filter(i=>ids.has(i.id)))};
 });
}
export function activeCycle(data:Data,today:string){return [...data.cycles].filter(c=>c.start_date<=today&&cycleEndDate(c)>=today).sort((a,b)=>b.start_date.localeCompare(a.start_date)||a.id.localeCompare(b.id))[0];}
export function itemHierarchy(data:Data,parentId:string|null){const chain:Item[]=[];const seen=new Set<string>();let id=parentId;while(id&&!seen.has(id)){seen.add(id);const item=data.items.find(i=>i.id===id);if(!item)break;chain.push(item);id=item.parent_id;}const nearest=(key:'project_id'|'annual_goal_id'|'cycle_goal_id'|'cycle_id')=>chain.find(i=>i[key])?.[key]||null;const goal=data.cycleGoals.find(g=>g.id===nearest('cycle_goal_id'));return {project_id:nearest('project_id'),annual_goal_id:goal?.annual_goal_id||nearest('annual_goal_id'),cycle_goal_id:goal?.id||null,cycle_id:goal?.cycle_id||nearest('cycle_id'),todo:chain.find(i=>i.level==='cycle')};}
export function dayTaskParents(data:Data,today:string,excludeId?:string){const cycle=activeCycle(data,today);return itemsInWeek(data,today).filter(i=>i.id!==excludeId&&i.level!=='day'&&!isProjectItem(data,i)&&(!cycle||!itemHierarchy(data,i.id).cycle_id||itemHierarchy(data,i.id).cycle_id===cycle.id));}
export function dayTaskValues(data:Data,values:Record<string,unknown>,today:string){const parent=dayTaskParents(data,today).find(i=>i.id===values.parent_id);const inherited=parent?itemHierarchy(data,parent.id):undefined;return {...values,cycle_id:activeCycle(data,today)?.id||null,parent_id:parent?.id||null,...(inherited?{project_id:inherited.project_id,annual_goal_id:inherited.annual_goal_id,cycle_goal_id:inherited.cycle_goal_id,...(values.project_id!==inherited.project_id?{version_id:null,milestone_id:null}:{})}:{annual_goal_id:null,cycle_goal_id:null})};}
export type OverdueItem={item:Item}&({kind:'day';dates:string[]}|{kind:'week';weekStart:string;weekEnd:string});
export function overdueItems(data:Data,today:string):OverdueItem[]{return sortItems([...new Map(taskItems(data).filter(i=>i.status==='todo'||i.status==='doing').map(i=>[i.id,i])).values()]).flatMap<OverdueItem>(item=>{
 const placements=data.placements.filter(p=>p.item_id===item.id);
 if(item.level==='day'){
  const dates=[...new Set(placements.filter(p=>p.kind==='day'&&p.date&&p.date<today).map(p=>p.date!))].sort();
  return dates.length?[{item,kind:'day',dates}]:[];
 }
 if(item.level==='week'){
  const last=placements.filter(p=>p.kind==='week'&&p.week_start).map(p=>p.week_start!).sort().at(-1);
  if(last&&addDays(last,6)<today)return [{item,kind:'week',weekStart:last,weekEnd:addDays(last,6)}];
 }
 return [];
});}
export function completion(items:Item[]){const unique=[...new Map(items.map(i=>[i.id,i])).values()];return {done:unique.filter(i=>i.status==='done').length,total:unique.length};}
// Clamp one real column between the tree edge and viewport right edge; no cloned content.
export function frozenOffset(columnLeft:number,columnWidth:number,scrollLeft:number,viewportWidth:number,treeWidth:number){return Math.max(scrollLeft+treeWidth,Math.min(columnLeft,scrollLeft+viewportWidth-columnWidth))-columnLeft;}
export function timeDistribution(data:Data,from:string,to:string,now=new Date()){
 const lo=new Date(`${from}T00:00:00+08:00`).getTime(),hi=Math.min(new Date(`${addDays(to,1)}T00:00:00+08:00`).getTime(),now.getTime());
 const total=Math.max(0,(hi-lo)/60000);const life=new Map<string,number>(),projects=new Map<string,number>(),goals=new Map<string,number>(),directions=new Map<string,number>();let recorded=0,work=0;
 const add=(map:Map<string,number>,k:string,n:number)=>map.set(k,(map.get(k)||0)+n);
 const entries:TimeEntry[]=data.timeEntries;
 for(const e of entries){const minutes=Math.max(0,(Math.min(Date.parse(e.end_at),hi)-Math.max(Date.parse(e.start_at),lo))/60000);if(!minutes)continue;recorded+=minutes;
 const category=data.categories.find(c=>c.id===e.category_id);const isWork=e.work_kind==='work';add(life,isWork?'工作':e.work_kind==='life'?(e.life_category||category?.name||'生活事务'):`历史分类：${category?.name||'未分类'}`,minutes);
 if(isWork){work+=minutes;add(projects,data.longProjects.find(p=>p.id===e.long_project_id)?.title||'未归属工作',minutes);const goal=data.annualGoals.find(g=>g.id===e.primary_goal_id);add(goals,goal?.title||'未归属工作',minutes);add(directions,data.directions.find(d=>d.id===(e.direction_id===undefined?goal?.direction_id:e.direction_id))?.title||'未归属工作',minutes);}
 }
 add(life,'未记录',Math.max(0,total-recorded));return {total,recorded,work,life,projects,goals,directions};
}

export function isProjectItem(data:Data,item:Pick<Item,'id'|'level'|'project_id'>){return item.level==='backlog'&&!!(item.project_id||data.itemProjects.some(p=>p.item_id===item.id));}
