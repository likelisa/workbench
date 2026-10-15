import { minuteLabel, type DailyPlanTemplate, type TimeEntry } from './model';
import { parseClock, stamp } from './timeEditorModel';
import type { Operation } from './unifiedApi';
export const TIMELINE={gutter:64,reference:240,day:250,height:1440,startMinute:420};
export const timelineHourLabels=Array.from({length:25},(_,hour)=>minuteLabel((TIMELINE.startMinute+hour*60)%1440));
// Rotate only the display: both segments retain the original record and calendar date.
export function timelineSegments<T extends {start:number;end:number}>(spans:T[]){
 return spans.flatMap(span=>[[TIMELINE.startMinute,1440],[0,TIMELINE.startMinute]].flatMap(([lo,hi])=>{
  const start=Math.max(lo,span.start),end=Math.min(hi,span.end);
  if(end<=start)return [];
  const displayStart=start-TIMELINE.startMinute+(lo===0?1440:0);
  return [{...span,start,end,displayStart,displayEnd:displayStart+end-start}];
 }));
}
export function actualSpan(entry:TimeEntry,date:string){const lo=Date.parse(stamp(date,0));return {id:entry.id,start:Math.max(0,(Date.parse(entry.start_at)-lo)/60000),end:Math.min(1440,(Date.parse(entry.end_at)-lo)/60000)};}
export function spanPosition(start:number,end:number){return {top:Math.max(0,start),height:Math.max(1,Math.min(1440,end)-Math.max(0,start))};}
// Preserve overlapping historical records: use parallel lanes instead of covering one with another.
export function timelineLanes<T extends {id:string;start:number;end:number}>(spans:T[]){
 const result:(T&{lane:number;lanes:number})[]=[];let group:typeof result=[],ends:number[]=[],groupEnd=-1;
 const finish=()=>{for(const row of group)row.lanes=ends.length;result.push(...group);group=[];ends=[];};
 for(const row of [...spans].filter(r=>r.end>r.start).sort((a,b)=>a.start-b.start||a.end-b.end)){
  if(row.start>=groupEnd){finish();groupEnd=-1;}let lane=ends.findIndex(end=>end<=row.start);if(lane<0)lane=ends.length;ends[lane]=row.end;groupEnd=Math.max(groupEnd,row.end);group.push({...row,lane,lanes:1});
 }finish();return result;
}
export type ScheduleRow={key:string;start:string;end:string;title:string;deleted?:boolean;original?:DailyPlanTemplate&{version:string}};
export const scheduleRow=(p:DailyPlanTemplate&{version:string}):ScheduleRow=>({key:p.id,start:minuteLabel(p.start_min),end:minuteLabel(p.end_min),title:p.title,original:p});
export const scheduleChanged=(r:ScheduleRow)=>r.original?!!r.deleted||r.start!==minuteLabel(r.original.start_min)||r.end!==minuteLabel(r.original.end_min)||r.title!==r.original.title:!r.deleted;
export function scheduleOperations(rows:ScheduleRow[]):{operations:Operation[];errors:Record<string,string>}{
 const errors:Record<string,string>={},operations:Operation[]=[];
 for(const row of rows){if(!scheduleChanged(row))continue;if(row.deleted){if(row.original)operations.push({kind:'dailyPlan',action:'delete',id:row.original.id,expected_version:row.original.version});continue;}
 const start=parseClock(row.start),end=parseClock(row.end);if(!Number.isFinite(start)||!Number.isFinite(end)||start>=1440||end<=start)errors[row.key]='请填写有效的当天时间段，跨午夜请拆成两段';if(!row.title.trim())errors[row.key]='请填写作息内容，例如睡觉、工作或看书';
 operations.push({kind:'dailyPlan',action:row.original?'update':'create',id:row.original?.id,expected_version:row.original?.version,values:{start_min:start,end_min:end,title:row.title.trim()}});
 }return {operations,errors};
}
