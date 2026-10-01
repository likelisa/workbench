import { useEffect, useMemo, useRef, useState } from 'react';
import Select from './Select';
import { batchNames } from './BatchCreate';
import type { BatchKind } from './batch';
import { addDays, assignmentsForDate, cycleWeek, dayTaskTimeLabel, localDate, STATUS, weekStart, type Data, type Status } from './model';
type Kind=BatchKind|'group';
type Node={id:string;kind:Kind;title:string;detail?:string;status?:Status;children:Node[];mismatch?:boolean};
type Props={data:Data;cycleId:string|null;onCreate:(kind:BatchKind,defaults?:Record<string,string>)=>void;onBatch:(kind:BatchKind,defaults?:Record<string,string>)=>void;onEdit:(kind:BatchKind,id:string)=>void;onDelete:(kind:BatchKind,id:string,title:string)=>void;onStatus:(kind:BatchKind,id:string,status:Status)=>void;onAnnual:(id:string)=>void;onManageWeeks:(id:string)=>void};
export default function PlanningTree({data,cycleId,onCreate,onBatch,onEdit,onDelete,onStatus,onAnnual,onManageWeeks}:Props){
 const [expanded,setExpanded]=useState(()=>new Set([...data.directions.map(item=>item.id),...data.annualGoals.map(item=>item.id),'orphan-direction','orphan-annual','unlinked-days'])),[from,setFrom]=useState(''),[to,setTo]=useState('');
 const known=useRef(new Set([...data.directions.map(item=>item.id),...data.annualGoals.map(item=>item.id)]));
 useEffect(()=>{const ids=[...data.directions.map(item=>item.id),...data.annualGoals.map(item=>item.id)];const added=ids.filter(id=>!known.current.has(id));added.forEach(id=>known.current.add(id));if(added.length)setExpanded(previous=>new Set([...previous,...added]));},[data.directions,data.annualGoals]);
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
 const render=(node:Node):React.ReactNode=>{
  const open=expanded.has(node.id),child=({direction:'annual',annual:'task',task:'assignment',assignment:'dayTask'} as Partial<Record<Kind,BatchKind>>)[node.kind];
  const canAdd=child!=='dayTask'||data.assignmentWeeks.some(w=>w.assignment_id===node.id);
  return <li className="tree-branch" key={node.id}><div className={`tree-node ${node.status||''}`}><div className="tree-node-head"><small>{node.kind==='group'?'未归属区域':batchNames[node.kind]}</small>{node.children.length>0&&<button className="tree-toggle" aria-expanded={open} aria-label={`${open?'收起':'展开'}${node.title}`} onClick={()=>setExpanded(previous=>{const next=new Set(previous);next.has(node.id)?next.delete(node.id):next.add(node.id);return next;})}>{open?'−':'＋'}</button>}</div><strong>{node.title}</strong>{node.detail&&<p>{node.detail}</p>}{node.mismatch&&<span className="mismatch">日期与周次不匹配</span>}
    {node.kind==='annual'?<button className={`history-complete ${node.status==='done'?'done':''}`} onClick={()=>onAnnual(node.id)}>{node.status==='done'?'已完成':'未完成'}</button>:node.status&&node.kind!=='group'&&<Select className={`status ${node.status}`} aria-label={`${node.title}状态`} value={node.status} onChange={event=>onStatus(node.kind as BatchKind,node.id,event.target.value as Status)}>{Object.entries(STATUS).map(([value,label])=><option key={value} value={value}>{label}</option>)}</Select>}
    {node.kind!=='group'&&<div className="record-actions"><button onClick={()=>onEdit(node.kind as BatchKind,node.id)}>编辑</button><button onClick={()=>onDelete(node.kind as BatchKind,node.id,node.title)}>删除</button>{node.kind==='assignment'&&<button onClick={()=>onManageWeeks(node.id)}>管理周次</button>}</div>}
    {child&&<div className="tree-add"><button disabled={!canAdd} title={canAdd?'':'先安排周次后才能关联日任务'} onClick={()=>onCreate(child,defaults(node,child))}>＋ {batchNames[child]}</button><button disabled={!canAdd} onClick={()=>onBatch(child,defaults(node,child))}>批量新增</button></div>}
    {!node.children.length&&child&&<small className="muted">{child==='task'?'当前周期暂无本轮待办':'暂无下层任务'}</small>}
  </div>{open&&node.children.length>0&&<ul>{node.children.map(render)}</ul>}</li>;
 };
 const allIds=(items:Node[]):string[]=>items.flatMap(item=>[item.id,...allIds(item.children)]);
 return <section className="panel planning-tree-panel"><div className="section-head"><h2>计划关系树</h2><div className="tree-toolbar"><button className="outline" onClick={()=>setExpanded(new Set(allIds(nodes)))}>展开全部</button><button className="outline" onClick={()=>setExpanded(new Set())}>收起全部</button><button className="outline" onClick={()=>onCreate('direction')}>＋ 年度方向</button><Select aria-label="批量新增类型" value="" onChange={event=>{if(event.target.value)onBatch(event.target.value as BatchKind,{cycle_id:cycleId||''});}}><option value="">批量新增…</option>{Object.entries(batchNames).map(([value,label])=><option key={value} value={value}>{label}</option>)}</Select></div></div>{!cycleId&&<div className="tree-date-filter"><span>未关联日任务日期筛选</span><input aria-label="筛选开始日期" type="date" value={from} onChange={e=>setFrom(e.target.value)}/><input aria-label="筛选结束日期" type="date" value={to} onChange={e=>setTo(e.target.value)}/></div>}<div className="tree-scroll"><ul className="planning-tree">{nodes.map(render)}</ul>{!nodes.length&&<p className="empty">从创建年度方向开始建立计划关系。</p>}</div></section>;
}
