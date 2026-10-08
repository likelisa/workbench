import { Plus, Trash2 } from 'lucide-react';
import { minuteLabel, type Data, type DailyPlanTemplate, type PlanBlock } from './model';

type Props={data:Data;date:string;onCreate:()=>void;onEdit:(plan:DailyPlanTemplate)=>void;onDelete:(plan:DailyPlanTemplate)=>void;onEditLegacy:(plan:PlanBlock)=>void;onDeleteLegacy:(plan:PlanBlock)=>void};
export default function DayPlans({data,date,onCreate,onEdit,onDelete,onEditLegacy,onDeleteLegacy}:Props){
  const plans=[...data.dailyPlanTemplates].sort((a,b)=>a.start_min-b.start_min);
  const legacy=data.planBlocks.filter(plan=>plan.date===date).sort((a,b)=>a.start_min-b.start_min);
  return <section className="panel day-legacy-plans">
    <div className="section-head"><h2>单日计划时段</h2><button className="outline" onClick={onCreate}><Plus size={15}/> 每日规划</button></div>
    <p className="muted">与周视图的每日规划共用，修改后同步到所有日期。</p>
    {plans.length?plans.map(plan=><div className="day-plan-row" key={plan.id}><span>{minuteLabel(plan.start_min)}—{minuteLabel(plan.end_min)}</span><button className="task-title-button" onClick={()=>onEdit(plan)}>{plan.title}</button><button className="icon-button" aria-label={`删除每日规划：${plan.title}`} onClick={()=>onDelete(plan)}><Trash2 size={16}/></button></div>):<p className="calendar-overview-empty">暂无每日规划，可在日视图或周视图中添加。</p>}
    {legacy.length>0&&<details className="legacy-day-plans"><summary>历史单日记录 · {legacy.length} 项</summary><p className="muted">旧版按日期保存的记录保留在此，不作为每日共用规划。</p>{legacy.map(plan=><div className="day-plan-row" key={plan.id}><span>{minuteLabel(plan.start_min)}—{minuteLabel(plan.end_min)}</span><button className="task-title-button" onClick={()=>onEditLegacy(plan)}>{plan.title}</button><button className="icon-button" aria-label={`删除历史单日记录：${plan.title}`} onClick={()=>onDeleteLegacy(plan)}><Trash2 size={16}/></button></div>)}</details>}
  </section>;
}
