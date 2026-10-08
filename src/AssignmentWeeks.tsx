import Select from './Select';
import { weekDateRange, type Cycle } from './model';

type Props={cycle?:Cycle;weeks:number[];disabled?:boolean;onSchedule:(week:number)=>void;onUnschedule:(week:number)=>void;label?:string};
export function WeekLabel({cycle,week}:{cycle?:Cycle;week:number}) {
  return <>第{week}周{cycle&&<small className="week-date-range">（{weekDateRange(cycle,week)}）</small>}</>;
}

export default function AssignmentWeeks({cycle,weeks,disabled,onSchedule,onUnschedule,label='安排到周'}:Props) {
  const ordered=[...new Set(weeks)].sort((a,b)=>a-b);
  const available=Array.from({length:12},(_,i)=>i+1).filter(week=>!ordered.includes(week));
  return <div className="assignment-weeks">
    {ordered.map(week=><button type="button" key={week} disabled={disabled} aria-label={`移除第${week}周安排`} onClick={()=>onUnschedule(week)}><span><WeekLabel cycle={cycle} week={week}/></span><span aria-hidden="true">×</span></button>)}
    <Select value="" aria-label={label} disabled={disabled||!cycle||!available.length} renderOption={({value,label:optionLabel})=>value?<WeekLabel cycle={cycle} week={Number(value)}/>:optionLabel} onChange={event=>{if(event.target.value)onSchedule(Number(event.target.value));}}>
      <option value="">{available.length?'＋ 安排到周':'已安排全部周次'}</option>
      {available.map(week=><option key={week} value={week}>第{week}周{cycle?`（${weekDateRange(cycle,week)}）`:''}</option>)}
    </Select>
  </div>;
}
