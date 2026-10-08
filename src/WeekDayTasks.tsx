import { STATUS_ICONS } from './StatusIcon';
import { addDays, calendarRange, dayTaskTimeLabel, STATUS, type DayTask } from './model';

type Props={tasks:DayTask[];date:string;onEditDayTask:(task:DayTask)=>void;onOpenDay:(date:string)=>void};
const weekdays=['周一','周二','周三','周四','周五','周六','周日'];
const countLabel=(items:DayTask[])=>`${items.length} 项 · ${items.filter(item=>item.status==='done').length} 项完成`;
const taskCaption=(task:DayTask)=>`${dayTaskTimeLabel(task)?`${dayTaskTimeLabel(task)} · `:''}${task.title}`;
export default function WeekDayTasks({tasks:allTasks,date,onEditDayTask,onOpenDay}:Props){
 const [from]=calendarRange(date,'week');
 const days=Array.from({length:7},(_,index)=>addDays(from,index));
 return <div className="calendar-week-grid">{days.map((day,index)=>{const tasks=allTasks.filter(task=>task.date===day);return <section className="panel calendar-week-day" key={day}><div className="calendar-week-day-head"><strong>{weekdays[index]} · {day.slice(5)}</strong><span>{countLabel(tasks)}</span></div>{tasks.length?<div className="calendar-week-task-list">{tasks.map(task=><button type="button" className="calendar-week-task" key={task.id} onClick={()=>onEditDayTask(task)} aria-label={`编辑日任务：${task.title}`}><span className="calendar-week-status-icon" role="img" aria-label={STATUS[task.status]} title={STATUS[task.status]}>{STATUS_ICONS[task.status]}</span><strong>{taskCaption(task)}</strong></button>)}</div>:<p className="calendar-overview-empty">暂无日任务</p>}<button className="text-button" onClick={()=>onOpenDay(day)}>查看这一天</button></section>})}</div>;
}
