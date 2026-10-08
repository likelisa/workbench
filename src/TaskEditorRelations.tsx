import AssignmentWeeks from './AssignmentWeeks';
import PriorityBadge from './PriorityBadge';
import StatusIcon from './StatusIcon';
import { assignmentsForDate, dayTaskTimeLabel, type Data } from './model';

type ChildKind='assignment'|'dayTask';
type Props={kind:'task'|'assignment';recordId:string;data:Data;disabled:boolean;onCreate:()=>void;onEdit:(kind:ChildKind,id:string)=>void;onDelete:(kind:ChildKind,id:string,title:string)=>void;onSchedule:(week:number)=>void;onUnschedule:(week:number)=>void};

export default function TaskEditorRelations({kind,recordId,data,disabled,onCreate,onEdit,onDelete,onSchedule,onUnschedule}:Props) {
  const assignment=kind==='assignment'?data.assignments.find(item=>item.id===recordId):undefined;
  const cycle=data.cycles.find(item=>item.id===assignment?.cycle_id);
  const weeks=data.assignmentWeeks.filter(item=>item.assignment_id===recordId).map(item=>item.week_no);
  const childKind=kind==='task'?'assignment':'dayTask';
  const children=kind==='task'?data.assignments.filter(item=>item.task_id===recordId):data.dayTasks.filter(item=>item.assignment_id===recordId);
  const canCreate=kind==='task'?!!data.tasks.find(item=>item.id===recordId)?.cycle_id:!!cycle&&weeks.length>0;
  return <div className="task-editor-relations" data-no-autosave>
    {kind==='assignment'&&<section><h3>周次管理</h3><AssignmentWeeks cycle={cycle} weeks={weeks} disabled={disabled} onSchedule={onSchedule} onUnschedule={onUnschedule}/>{!cycle&&<p className="muted">请先为所属待办指定周期，再安排周次。</p>}</section>}
    <section>
      <div className="section-head"><h3>{kind==='task'?'下层周任务':'关联日任务'}</h3><button type="button" className="outline" disabled={disabled||!canCreate} title={!canCreate?(kind==='task'?'请先指定周期':'请先安排周次'):undefined} onClick={onCreate}>＋ {kind==='task'?'周任务':'日任务'}</button></div>
      {children.map(item=>{
        const title='commitment'in item?item.commitment:item.title;
        return <div className="task-editor-child" key={item.id}>
          <button type="button" className="task-editor-child-title" disabled={disabled} onClick={()=>onEdit(childKind,item.id)}>{title}{'date'in item&&<small>{item.date} {dayTaskTimeLabel(item)}{!assignmentsForDate(data,item.date).some(week=>week.id===recordId)&&' · 日期与周次不匹配'}</small>}</button>
          <PriorityBadge value={item.priority}/><span className={`pill ${item.status}`}><StatusIcon value={item.status}/></span>
          <button type="button" className="text-button task-editor-child-delete" disabled={disabled} aria-label={`删除${title}`} onClick={()=>onDelete(childKind,item.id,title)}>删除</button>
        </div>;
      })}
      {!children.length&&<p className="muted">暂无{kind==='task'?'下层周任务':'关联日任务'}。</p>}
    </section>
  </div>;
}
