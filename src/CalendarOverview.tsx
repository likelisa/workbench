import { addDays, assignmentsInRange, calendarRange, dayTaskTimeLabel, localDate, mondayOf, STATUS, type CalendarMode, type Data, type DayTask, type WeekAssignment } from './model';
import WeekTimeline from './WeekTimeline';
import Select from './Select';
import type { Status } from './model';

type Props = { data:Data; date:string; mode:Exclude<CalendarMode,'day'>; onOpenDay:(date:string)=>void; onOpenWeek:(date:string)=>void; onOpenMonth:(date:string)=>void; onCreateTemplate:(start:string,end:string)=>void; onEditTemplate:(plan:Data['dailyPlanTemplates'][number])=>void; onDeleteTemplate:(plan:Data['dailyPlanTemplates'][number])=>void; onCreateEntry:(date:string,start:string,end:string)=>void; onEditEntry:(entry:Data['timeEntries'][number])=>void; onDeleteEntry:(entry:Data['timeEntries'][number])=>void; onCreateAssignment:(date:string)=>void; onEditAssignment:(assignment:WeekAssignment)=>void; onDeleteAssignment:(assignment:WeekAssignment)=>void; onAssignmentStatus:(assignment:WeekAssignment,status:Status)=>void };
const weekdays=['周一','周二','周三','周四','周五','周六','周日'];
const countLabel=(items:{status:string}[])=>`${items.length} 项 · ${items.filter(item=>item.status==='done').length} 项完成`;
const taskCaption=(task:DayTask)=>`${dayTaskTimeLabel(task)?`${dayTaskTimeLabel(task)} · `:''}${task.title}`;

function AssignmentList({data,items,onEdit,onDelete,onStatus}:{data:Data;items:WeekAssignment[];onEdit:(item:WeekAssignment)=>void;onDelete:(item:WeekAssignment)=>void;onStatus:(item:WeekAssignment,status:Status)=>void}) {
  return items.length?<div className="calendar-assignment-list">{items.map(assignment=><div className="calendar-assignment" key={assignment.id}><Select className={`status ${assignment.status}`} aria-label={`${assignment.commitment}状态`} value={assignment.status} onChange={event=>onStatus(assignment,event.target.value as Status)}>{Object.entries(STATUS).map(([value,label])=><option key={value} value={value}>{label}</option>)}</Select><strong>{assignment.commitment}</strong><small>{data.tasks.find(task=>task.id===assignment.task_id)?.title||'本轮待办已删除'} · {data.cycles.find(cycle=>cycle.id===assignment.cycle_id)?.title||'周期已删除'}</small><div className="record-actions"><button onClick={()=>onEdit(assignment)} aria-label={`编辑周任务：${assignment.commitment}`}>编辑</button><button onClick={()=>onDelete(assignment)} aria-label={`删除周任务：${assignment.commitment}`}>删除</button></div></div>)}</div>:<p className="calendar-overview-empty">暂无周任务</p>;
}

function DayCell({date,tasks,onOpenDay,muted=false}:{date:string;tasks:DayTask[];onOpenDay:(date:string)=>void;muted?:boolean}) {
  const shown=muted?[]:tasks.slice(0,3);
  return <button className={`calendar-day-cell ${muted?'outside':''} ${date===localDate()?'today':''}`} onClick={()=>onOpenDay(date)} aria-label={`${date}，${tasks.length} 个日任务`}><span className="calendar-day-number">{Number(date.slice(8))}</span>{shown.map(task=><span className={`calendar-task-line ${task.status}`} key={task.id}><i/>{taskCaption(task)}</span>)}{!muted&&tasks.length>shown.length&&<small>还有 {tasks.length-shown.length} 项</small>}</button>;
}

export default function CalendarOverview({data,date,mode,onOpenDay,onOpenWeek,onOpenMonth,onCreateTemplate,onEditTemplate,onDeleteTemplate,onCreateEntry,onEditEntry,onDeleteEntry,onCreateAssignment,onEditAssignment,onDeleteAssignment,onAssignmentStatus}:Props) {
  if(mode==='week') {
    const [from,to]=calendarRange(date,'week');
    const days=Array.from({length:7},(_,index)=>addDays(from,index));
    const assignments=assignmentsInRange(data,from,to);
    return <div className="calendar-overview"><section className="panel calendar-period-assignments"><div className="section-head"><h2>本周周任务</h2><div className="calendar-week-actions"><span className="muted">{countLabel(assignments)}</span><button className="outline" onClick={()=>onCreateAssignment(date)}>＋ 周任务</button></div></div><AssignmentList data={data} items={assignments} onEdit={onEditAssignment} onDelete={onDeleteAssignment} onStatus={onAssignmentStatus}/></section><div className="calendar-week-grid">{days.map((day,index)=>{const tasks=data.dayTasks.filter(task=>task.date===day);return <section className="panel calendar-week-day" key={day}><div className="calendar-week-day-head"><strong>{weekdays[index]} · {day.slice(5)}</strong><span>{countLabel(tasks)}</span></div>{tasks.length?<div className="calendar-week-task-list">{tasks.map(task=><div className="calendar-week-task" key={task.id}><span className={`pill ${task.status}`}>{STATUS[task.status]}</span><strong>{taskCaption(task)}</strong></div>)}</div>:<p className="calendar-overview-empty">暂无日任务</p>}<button className="text-button" onClick={()=>onOpenDay(day)}>查看这一天</button></section>})}</div><WeekTimeline data={data} date={date} onCreateTemplate={onCreateTemplate} onEditTemplate={onEditTemplate} onDeleteTemplate={onDeleteTemplate} onCreateEntry={onCreateEntry} onEditEntry={onEditEntry} onDeleteEntry={onDeleteEntry}/></div>;
  }

  if(mode==='month') {
    const [from,to]=calendarRange(date,'month');
    const rows:string[]=[];
    for(let day=mondayOf(from);day<to;day=addDays(day,7)) rows.push(day);
    return <div className="calendar-overview"><div className="calendar-month-weekdays">{weekdays.map(label=><span key={label}>{label}</span>)}</div><div className="calendar-month-rows">{rows.map(rowStart=>{
      const rowEnd=addDays(rowStart,7), inMonthFrom=rowStart>from?rowStart:from, inMonthTo=rowEnd<to?rowEnd:to;
      const assignments=assignmentsInRange(data,inMonthFrom,inMonthTo);
      return <section className="calendar-month-row" key={rowStart}><div className="calendar-month-assignment-strip"><button onClick={()=>onOpenWeek(rowStart)}>{rowStart.slice(5)} — {addDays(rowStart,6).slice(5)} · 查看该周</button>{assignments.length?<div>{assignments.slice(0,3).map(assignment=><span className={`calendar-assignment-chip ${assignment.status}`} key={assignment.id} title={assignment.commitment}>{assignment.commitment}</span>)}{assignments.length>3&&<small>另有 {assignments.length-3} 项周任务</small>}</div>:<small>无周任务</small>}</div><div className="calendar-month-days">{Array.from({length:7},(_,index)=>{const day=addDays(rowStart,index);return <DayCell key={day} date={day} muted={day<from||day>=to} tasks={data.dayTasks.filter(task=>task.date===day)} onOpenDay={onOpenDay}/>})}</div></section>;
    })}</div></div>;
  }

  const year=Number(date.slice(0,4));
  return <div className="calendar-year-grid">{Array.from({length:12},(_,index)=>{
    const month=`${year}-${String(index+1).padStart(2,'0')}-01`;
    const [from,to]=calendarRange(month,'month');
    const tasks=data.dayTasks.filter(task=>task.date>=from&&task.date<to).sort((a,b)=>a.date.localeCompare(b.date));
    const assignments=assignmentsInRange(data,from,to);
    return <button className="panel calendar-year-month" key={month} onClick={()=>onOpenMonth(month)}><span className="calendar-year-month-head"><strong>{index+1} 月</strong><span>查看月视图 ›</span></span><span className="calendar-year-counts">日任务 {countLabel(tasks)}<br/>周任务 {countLabel(assignments)}</span><span className="calendar-year-preview"><b>日任务</b>{tasks.length?tasks.slice(0,2).map(task=><span key={task.id}>{task.date.slice(5)} · {taskCaption(task)}</span>):<span>暂无</span>}{tasks.length>2&&<small>另有 {tasks.length-2} 项</small>}</span><span className="calendar-year-preview"><b>周任务</b>{assignments.length?assignments.slice(0,2).map(assignment=><span key={assignment.id}>{assignment.commitment}</span>):<span>暂无</span>}{assignments.length>2&&<small>另有 {assignments.length-2} 项</small>}</span></button>;
  })}</div>;
}
