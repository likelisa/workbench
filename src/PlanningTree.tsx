import { useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import { ChevronDown, ChevronRight, Plus, Trash2, TriangleAlert } from 'lucide-react';
import Select from './Select';
import { batchNames } from './BatchCreate';
import type { BatchKind } from './batch';
import { addDays, assignmentsForDate, cycleWeek, dayTaskTimeLabel, localDate, STATUS, weekStart, type Data, type Status } from './model';
type Kind=BatchKind|'group';
type Node={id:string;kind:Kind;title:string;detail?:string;status?:Status;children:Node[];mismatch?:boolean};
type Props={data:Data;cycleId:string|null;onCreate:(kind:BatchKind,defaults?:Record<string,string>)=>void;onBatch:(kind:BatchKind,defaults?:Record<string,string>)=>void;onEdit:(kind:BatchKind,id:string)=>void;onDelete:(kind:BatchKind,id:string,title:string)=>void;onStatus:(kind:BatchKind,id:string,status:Status)=>void;onAnnual:(id:string)=>void};
export default function PlanningTree({data,cycleId,onCreate,onBatch,onEdit,onDelete,onStatus,onAnnual}:Props){
 const [collapsed,setCollapsed]=useState(new Set<string>()),[from,setFrom]=useState(''),[to,setTo]=useState('');
 const known=useRef<Set<string>|null>(null),canvas=useRef<HTMLDivElement>(null);
 const [lines,setLines]=useState<string[]>([]);
 const nodes=useMemo(()=>{
  const cycle=data.cycles.find(item=>item.id===cycleId),tasks=data.tasks.filter(item=>item.cycle_id===cycleId);
  const day=(item:Data['dayTasks'][number]):Node=>({id:item.id,kind:'dayTask',title:item.title,detail:`${item.date} ${dayTaskTimeLabel(item)}`,status:item.status,children:[],mismatch:!!item.assignment_id&&!assignmentsForDate(data,item.date).some(a=>a.id===item.assignment_id)});
  const task=(item:Data['tasks'][number]):Node=>({id:item.id,kind:'task',title:item.title,status:item.status,children:data.assignments.filter(a=>a.task_id===item.id).map(a=>({id:a.id,kind:'assignment',title:a.commitment,status:a.status,detail:data.assignmentWeeks.filter(w=>w.assignment_id===a.id).map(w=>w.week_no).sort((a,b)=>a-b).map(week=>`第 ${week} 周`).join('、')||'待安排',children:data.dayTasks.filter(d=>d.assignment_id===a.id).map(day)}))});
  const goal=(item:Data['annualGoals'][number]):Node=>({id:item.id,kind:'annual',title:item.title,detail:`${item.year} 年`,status:item.completed===1?'done':'missed',children:tasks.filter(t=>t.annual_goal_id===item.id).map(task)});
  const roots:Node[]=data.directions.map(item=>({id:item.id,kind:'direction',title:item.title,children:data.annualGoals.filter(a=>a.direction_id===item.id).map(goal)}));
  const missingGoals=data.annualGoals.filter(item=>!item.direction_id||!data.directions.some(d=>d.id===item.direction_id));
  if(missingGoals.length)roots.push({id:'orphan-direction',kind:'group',title:'未归属年度方向',children:missingGoals.map(goal)});
  const missingTasks=tasks.filter(item=>!item.annual_goal_id||!data.annualGoals.some(a=>a.id===item.annual_goal_id));
  if(missingTasks.length)roots.push({id:'orphan-annual',kind:'group',title:'未归属年度目标',children:missingTasks.map(task)});
  const unlinked=data.dayTasks.filter(item=>(!item.assignment_id||!data.assignments.some(a=>a.id===item.assignment_id))&&(cycle?item.date>=cycle.start_date&&item.date<=addDays(cycle.start_date,83):(!from||item.date>=from)&&(!to||item.date<=to)));
  if(unlinked.length)roots.push({id:'unlinked-days',kind:'group',title:'未关联日任务',children:unlinked.sort((a,b)=>a.date.localeCompare(b.date)).map(day)});
  return roots;
 },[data,cycleId,from,to]);
 const defaults=(node:Node,child:BatchKind):Record<string,string>=>{
  if(child==='annual')return {direction_id:node.id,year:String(new Date().getFullYear())};
  if(child==='task')return {annual_goal_id:node.id,cycle_id:cycleId||''};
  if(child==='assignment')return {task_id:node.id,cycle_id:cycleId||''};
  const assignment=data.assignments.find(item=>item.id===node.id),cycle=data.cycles.find(item=>item.id===assignment?.cycle_id),weeks=data.assignmentWeeks.filter(item=>item.assignment_id===node.id).map(item=>item.week_no).sort((a,b)=>a-b);
  return {assignment_id:node.id,date:cycle&&weeks.length?(weeks.includes(cycleWeek(cycle,localDate()))?localDate():weekStart(cycle,weeks[0])):localDate()};
 };
 const key=(node:Node)=>`${node.kind}:${node.id}`;
 const columns:Record<Kind,number>={direction:0,annual:1,task:2,assignment:3,dayTask:4,group:0};
 const column=(node:Node)=>node.kind==='group'?(node.id==='orphan-annual'?1:node.id==='unlinked-days'?3:0):columns[node.kind];
 const walk=(items:Node[],ancestors:string[]=[]):{node:Node;ancestors:string[]}[]=>items.flatMap(node=>[{node,ancestors},...walk(node.children,[...ancestors,key(node)])]);
 useEffect(()=>{
  const entries=walk(nodes),ids=new Set(entries.map(({node})=>key(node)));
  if(known.current){const previous=known.current;const reveal=entries.filter(({node})=>!previous.has(key(node))).flatMap(({ancestors})=>ancestors);if(reveal.length)setCollapsed(current=>new Set([...current].filter(id=>!reveal.includes(id))));}
  known.current=ids;
 },[nodes]);
 useLayoutEffect(()=>{
  const element=canvas.current;if(!element)return;
  const measure=()=>{
   const origin=element.getBoundingClientRect(),points=new Map<string,DOMRect>();
   element.querySelectorAll<HTMLElement>('[data-tree-key]').forEach(node=>points.set(node.dataset.treeKey!,node.getBoundingClientRect()));
   const paths:string[]=[];
   for(const {node} of walk(nodes)){const parent=points.get(key(node));if(!parent)continue;for(const child of node.children){const target=points.get(key(child));if(!target)continue;const x=parent.right-origin.left,y=parent.top+parent.height/2-origin.top,end=target.left-origin.left,cy=target.top+target.height/2-origin.top;paths.push(`M ${x} ${y} H ${x+(end-x)/2} V ${cy} H ${end}`);}}
   setLines(previous=>previous.join('|')===paths.join('|')?previous:paths);
  };
  measure();const observer=new ResizeObserver(measure);observer.observe(element);element.querySelectorAll('[data-tree-key]').forEach(node=>observer.observe(node));return()=>observer.disconnect();
 },[nodes,collapsed]);
 const toggle=(node:Node)=>setCollapsed(previous=>{const next=new Set(previous),id=key(node);next.has(id)?next.delete(id):next.add(id);return next;});
 const icons:Record<Status,string>={todo:'⭕',doing:'⏳',done:'✅',missed:'❌'};
 const render=(node:Node,root=false):React.ReactNode=>{
  const open=!collapsed.has(key(node)),child=({direction:'annual',annual:'task',task:'assignment',assignment:'dayTask'} as Partial<Record<Kind,BatchKind>>)[node.kind];
  const canAdd=!!child&&(child!=='dayTask'||data.assignmentWeeks.some(w=>w.assignment_id===node.id));
  const description=[node.detail,node.status&&STATUS[node.status],node.mismatch&&'日期与周次不匹配'].filter(Boolean).join('；');
  return <li className="compact-tree-branch" key={key(node)} style={root?{marginLeft:column(node)*240}:undefined}>
   <div className="compact-tree-slot">
    <div className={`compact-tree-node ${node.status||''} ${node.kind==='group'?'tree-group':''}`}>
     <div className="compact-tree-control" data-tree-key={key(node)} title={description||node.title}>
      {node.status&&<Select className={`tree-state status ${node.status}`} aria-label={`${node.title}状态：${STATUS[node.status]}`} value={node.status} displayLabel={<span aria-hidden="true">{icons[node.status]}</span>} onChange={event=>{const status=event.target.value as Status;if(status===node.status)return;if(node.kind==='annual')onAnnual(node.id);else onStatus(node.kind as BatchKind,node.id,status);}}>
       {Object.entries(STATUS).filter(([value])=>node.kind!=='annual'||value==='done'||value==='missed').map(([value,label])=><option key={value} value={value}>{label}</option>)}
      </Select>}
      {node.kind==='group'?<span className="compact-tree-title" tabIndex={0}>{node.title}</span>:<button type="button" className="compact-tree-title" aria-label={`编辑${batchNames[node.kind]}：${node.title}`} onClick={()=>onEdit(node.kind as BatchKind,node.id)}>{node.title}</button>}
      {node.mismatch&&<TriangleAlert className="tree-warning" size={14} aria-label="日期与周次不匹配"/>}
     </div>
     <div className="tree-drawer" aria-label={`${node.title}操作`}>
      <button type="button" disabled={!canAdd} aria-label={`在${node.title}下新增${child?batchNames[child]:'分支'}`} title={!child?'日任务没有下一级':!canAdd?'先安排周次后才能关联日任务':`新增${batchNames[child]}`} onClick={()=>{if(child)onCreate(child,defaults(node,child));}}><Plus size={16}/></button>
      <button type="button" disabled={node.kind==='group'} aria-label={`删除${node.title}`} title="删除" onClick={()=>onDelete(node.kind as BatchKind,node.id,node.title)}><Trash2 size={14}/></button>
      <button type="button" disabled={!node.children.length} aria-label={`${open?'收起':'展开'}${node.title}`} title={node.children.length?(open?'收起':'展开'):'没有下层分支'} aria-expanded={node.children.length?open:undefined} onClick={()=>toggle(node)}>{open?<ChevronDown size={16}/>:<ChevronRight size={16}/>}</button>
     </div>
    </div>
   </div>
   {open&&node.children.length>0&&<ul>{node.children.map(item=>render(item))}</ul>}
  </li>;
 };
 return <section className="panel planning-tree-panel"><div className="section-head"><h2>计划关系树</h2><div className="tree-toolbar"><button className="outline" onClick={()=>setCollapsed(new Set())}>展开全部</button><button className="outline" onClick={()=>setCollapsed(new Set(walk(nodes).map(({node})=>key(node))))}>收起全部</button><button className="outline" onClick={()=>onCreate('direction')}>＋ 年度方向</button><Select aria-label="批量新增类型" value="" onChange={event=>{if(event.target.value)onBatch(event.target.value as BatchKind,{cycle_id:cycleId||''});}}><option value="">批量新增…</option>{Object.entries(batchNames).map(([value,label])=><option key={value} value={value}>{label}</option>)}</Select></div></div>{!cycleId&&<div className="tree-date-filter"><span>未关联日任务日期筛选</span><input aria-label="筛选开始日期" type="date" value={from} onChange={e=>setFrom(e.target.value)}/><input aria-label="筛选结束日期" type="date" value={to} onChange={e=>setTo(e.target.value)}/></div>}
  <div className="compact-tree-scroll"><div className="compact-tree-header">{['年度方向','年度目标','本轮待办','周任务','日任务'].map(label=><div key={label}>{label}</div>)}</div><div ref={canvas} className="compact-tree-canvas"><div className="compact-tree-lanes" aria-hidden="true">{Array.from({length:5},(_,i)=><div key={i}/>)}</div><svg className="compact-tree-lines" aria-hidden="true">{lines.map((path,index)=><path key={index} d={path}/>)}</svg><ul className="compact-tree-roots">{nodes.map(node=>render(node,true))}</ul>{!nodes.length&&<p className="empty">从创建年度方向开始建立计划关系。</p>}</div></div>
 </section>;
}
