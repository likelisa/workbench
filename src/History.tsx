import PriorityBadge from './PriorityBadge';
import { annualStatus, type AnnualStatus } from './model';
import Select from './Select';
import { useMemo, useState } from 'react';
import { ChevronDown, ChevronRight } from 'lucide-react';
import { addDays, assignmentInWeek, cycleWeek, cycleEndDate, dayTaskTimeLabel, historyYearGroups, progress, STATUS, weekStart, type AnnualGoal, type Data, type DayTask, type Status } from './model';

type Props = {
  data: Data;
  onToggleAnnual: (goal: AnnualGoal,status:AnnualStatus) => void;
  onSetDayTaskStatus: (task: DayTask, status: Status) => void;
  onEditDayTask: (task: DayTask) => void;
  onDeleteDayTask: (task: DayTask) => void;
  onSaveCycleReview: (cycle: Data['cycles'][number], review: string) => void;
};

const completion = (items: {status: Status}[]) => `${items.filter(item=>item.status==='done').length}/${items.length} 已完成`;

export default function History({data,onToggleAnnual,onSetDayTaskStatus,onEditDayTask,onDeleteDayTask,onSaveCycleReview}:Props) {
  const groups=useMemo(()=>historyYearGroups(data),[data]);
  const [expandedYears,setExpandedYears]=useState<Set<number>>(()=>new Set(groups.length?[groups[0].year]:[]));
  const [expandedCycles,setExpandedCycles]=useState<Set<string>>(()=>new Set());
  const toggleYear=(year:number)=>setExpandedYears(previous=>{const next=new Set(previous);next.has(year)?next.delete(year):next.add(year);return next});
  const toggleCycle=(id:string)=>setExpandedCycles(previous=>{const next=new Set(previous);next.has(id)?next.delete(id):next.add(id);return next});
  const dayTaskRow=(task:DayTask)=><div className="history-day-task" key={task.id}>
    <span className="history-day-date">{task.date}</span>
    <strong>{dayTaskTimeLabel(task)&&<small className="task-clock">{dayTaskTimeLabel(task)} · </small>}{task.title}<PriorityBadge value={task.priority}/></strong>
    <Select aria-label={`设置“${task.title}”的状态`} className={`status ${task.status}`} value={task.status} onChange={e=>onSetDayTaskStatus(task,e.target.value as Status)}>{Object.entries(STATUS).map(([value,label])=><option key={value} value={value}>{label}</option>)}</Select>
    <div className="record-actions"><button onClick={()=>onEditDayTask(task)}>编辑</button><button onClick={()=>onDeleteDayTask(task)}>删除</button></div>
  </div>;

  return <>
    <div className="page-heading"><div><div className="kicker">回顾与归档</div><h1>历史情况</h1><p>按年份查看年度目标、已结束的 12 周计划、周任务和日任务。</p></div></div>
    <div className="history-years">{groups.map(group=>{
      const yearOpen=expandedYears.has(group.year);
      const annualDone=group.annualGoals.filter(goal=>annualStatus(goal)==='done').length;
      return <section className="panel history-year" key={group.year}>
        <button className="history-toggle history-year-toggle" aria-expanded={yearOpen} onClick={()=>toggleYear(group.year)}>
          <span className="history-toggle-title">{yearOpen?<ChevronDown size={19}/>:<ChevronRight size={19}/>}<strong>{group.year} 年</strong></span>
          <span className="history-toggle-summary">年度目标 {annualDone}/{group.annualGoals.length} 已完成 · 已结束周期 {group.cycles.length} 个</span>
        </button>
        {yearOpen&&<div className="history-year-body">
          <section className="history-annual"><h2>年度目标完成情况</h2>{group.annualGoals.length?<div className="history-annual-list">{group.annualGoals.map(goal=>{
            const linkedProgress=progress(data.tasks.filter(task=>task.annual_goal_id===goal.id));
            return <div className="history-annual-item" key={goal.id}><div><small>{data.directions.find(direction=>direction.id===goal.direction_id)?.title||'未关联年度方向'}</small><strong>{goal.title}</strong><span>关联待办进度：{linkedProgress?`${linkedProgress.done}/${linkedProgress.total} 已完成`:'暂无关联待办'}</span></div><Select className={`status ${annualStatus(goal)}`} aria-label={`${goal.title}完成状态`} value={annualStatus(goal)} onChange={e=>onToggleAnnual(goal,e.target.value as AnnualStatus)}>{['todo','done','missed'].map(v=><option key={v} value={v}>{STATUS[v as AnnualStatus]}</option>)}</Select></div>;
          })}</div>:<p className="history-empty">这一年没有年度目标</p>}</section>
          <section className="history-cycles"><h2>已结束的 12 周计划</h2>{group.cycles.length?<div className="history-cycle-list">{group.cycles.map(cycle=>{
            const cycleOpen=expandedCycles.has(cycle.id);
            const assignments=data.assignments.filter(assignment=>assignment.cycle_id===cycle.id);
            const scheduledAssignments=assignments.filter(assignment=>data.assignmentWeeks.some(row=>row.assignment_id===assignment.id));
            const scheduledAssignmentIds=new Set(scheduledAssignments.map(assignment=>assignment.id));
            const dayTasks=data.dayTasks.filter(task=>task.assignment_id!==null&&scheduledAssignmentIds.has(task.assignment_id));
            const unmatchedDays=dayTasks.filter(task=>!assignmentInWeek(data,task.assignment_id!,cycleWeek(cycle,task.date)));
            return <div className="history-cycle" key={cycle.id}>
              <button className="history-toggle history-cycle-toggle" aria-expanded={cycleOpen} onClick={()=>toggleCycle(cycle.id)}><span className="history-toggle-title">{cycleOpen?<ChevronDown size={17}/>:<ChevronRight size={17}/>}<strong>{cycle.title}</strong></span><span className="history-toggle-summary">{cycle.start_date} — {cycleEndDate(cycle)} · 周任务 {completion(assignments)} · 日任务 {completion(dayTasks)}</span></button>
              {cycleOpen&&<div className="history-weeks"><section className="history-cycle-review"><div className="history-review-summary"><span>{cycle.start_date} — {cycleEndDate(cycle)}</span><small>已安排周任务 {completion(scheduledAssignments)}</small><small>关联日任务 {completion(dayTasks)}</small></div><label className="history-review-label" htmlFor={`cycle-review-${cycle.id}`}>综合复盘</label><textarea id={`cycle-review-${cycle.id}`} className="history-review-input" defaultValue={cycle.review} placeholder="记录整体结果、经验与下一步方向…" onBlur={event=>onSaveCycleReview(cycle,event.target.value.trim())}/>{!cycle.review&&<p className="history-empty">暂无综合复盘</p>}</section>{Array.from({length:12},(_,index)=>{
                const weekNo=index+1, weekAssignments=assignments.filter(assignment=>assignmentInWeek(data,assignment.id,weekNo));
                return <div className="history-week" key={weekNo}><div className="history-week-head"><strong>第 {weekNo} 周</strong><span>{weekStart(cycle,weekNo)} — {addDays(weekStart(cycle,weekNo),6)}</span><small>周任务 {completion(weekAssignments)}</small></div>{weekAssignments.length?weekAssignments.map(assignment=>{
                  const linkedDays=data.dayTasks.filter(task=>task.assignment_id===assignment.id&&cycleWeek(cycle,task.date)===weekNo).sort((a,b)=>a.date.localeCompare(b.date));
                  return <div className="history-assignment" key={assignment.id}><div className="history-assignment-head"><strong>{assignment.commitment}<PriorityBadge value={assignment.priority}/></strong><span className={`pill ${assignment.status}`}>{STATUS[assignment.status]}</span><small>{data.tasks.find(task=>task.id===assignment.task_id)?.title||'本轮待办已删除'}</small></div>{assignment.review&&<p className="history-review">复盘：{assignment.review}</p>}{linkedDays.length?<div className="history-day-list">{linkedDays.map(dayTaskRow)}</div>:<p className="history-empty">暂无关联日任务</p>}</div>;
                }):<p className="history-empty">本周无安排</p>}</div>;
              })}{assignments.some(item=>!data.assignmentWeeks.some(row=>row.assignment_id===item.id))&&<div className="history-week"><strong>未安排周次的周任务</strong>{assignments.filter(item=>!data.assignmentWeeks.some(row=>row.assignment_id===item.id)).map(item=><div className="history-assignment" key={item.id}><strong>{item.commitment}<PriorityBadge value={item.priority}/></strong><span className={`pill ${item.status}`}>{STATUS[item.status]}</span>{item.review&&<p className="history-review">复盘：{item.review}</p>}</div>)}</div>}{unmatchedDays.length>0&&<div className="history-week"><strong>未匹配周次的关联日任务</strong><div className="history-day-list">{unmatchedDays.sort((a,b)=>a.date.localeCompare(b.date)).map(dayTaskRow)}</div></div>}</div>}
            </div>;
          })}</div>:<p className="history-empty">这一年暂无已结束的 12 周计划</p>}</section>
          {group.unlinkedDayTasks.length>0&&<section className="history-unlinked"><h2>未归属日任务</h2><div className="history-day-list">{group.unlinkedDayTasks.map(dayTaskRow)}</div></section>}
        </div>}
      </section>;
    })}</div>
  </>;
}
