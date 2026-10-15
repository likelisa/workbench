import { addDays, minuteLabel, type Data, type PlanBlock, type TimeEntry } from './model';
import type { Operation } from './unifiedApi';
export type TimeKind='plan'|'time';
export type SavedTime=(PlanBlock|TimeEntry)&{version?:string};
export type TimeRow={key:string;original?:SavedTime;startDate:string;start:string;end:string;nextDay:boolean;endDate?:string;category:string;project:string;direction:string;goal:string;note:string;schedule:string;itemId?:string|null;sourcePlan?:string|null;deleted?:boolean};
export const stamp=(date:string,minute:number)=>`${minute===1440?addDays(date,1):date}T${minuteLabel(minute===1440?0:minute)}:00+08:00`;
export function parseClock(value:string){if(!/^(?:[01]\d|2[0-3]):[0-5]\d$|^24:00$/.test(value))return NaN;const [h,m]=value.split(':').map(Number);return h*60+m;}
export function monthDates(month:string){if(!/^\d{4}-\d{2}$/.test(month))return [];const result=[];for(let d=`${month}-01`;d.slice(0,7)===month;d=addDays(d,1))result.push(d);return result;}
export function shiftMonth(month:string,delta:number){const d=new Date(`${month}-15T12:00:00`);d.setMonth(d.getMonth()+delta);return `${d.getFullYear()}-${String(d.getMonth()+1).padStart(2,'0')}`;}
export function localParts(value:string){return new Intl.DateTimeFormat('sv-SE',{timeZone:'Asia/Shanghai',year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',hour12:false}).format(new Date(value)).split(' ');}
export function dayEntries(data:Data,date:string){const lo=Date.parse(stamp(date,0)),hi=Date.parse(stamp(date,1440));return data.timeEntries.filter(e=>Date.parse(e.start_at)<hi&&Date.parse(e.end_at)>lo).sort((a,b)=>Date.parse(a.start_at)-Date.parse(b.start_at));}
export function timeCategory(p:Partial<PlanBlock>|Partial<TimeEntry>,data:Data){if(p.work_kind==='work')return '工作';return p.life_category||data.categories.find(c=>c.id===p.category_id)?.name||'待分类';}
export function rowFromRecord(p:SavedTime,kind:TimeKind,data:Data):TimeRow{
 const actual=p as TimeEntry,plan=p as PlanBlock;
 const [startDate,start]=kind==='time'?localParts(actual.start_at):[plan.date,minuteLabel(plan.start_min)];
 const [endDate,end]=kind==='time'?localParts(actual.end_at):[plan.date,minuteLabel(plan.end_min)];
 const category=p.work_kind==='work'?'work':p.category_id||'';
 const label=timeCategory(p,data);
 return {key:p.id,original:p,startDate,start,end,nextDay:endDate>startDate,endDate:endDate>addDays(startDate,1)?endDate:undefined,category,project:p.long_project_id||'',direction:p.direction_id||'',goal:p.primary_goal_id||'',note:p.title===label?'':p.title,schedule:plan.schedule_kind||'flexible',itemId:p.item_id,sourcePlan:actual.source_plan_id};
}
export function blankRow(date:string,previous?:TimeRow):TimeRow{
 // A new row belongs to the day being edited, even when the last record ends tomorrow.
 const endDate=previous?(previous.nextDay?(previous.endDate||addDays(previous.startDate,1)):previous.startDate):undefined;
 const start=endDate===date&&previous?.end!=='24:00'?previous?.end||'':'';
 return {key:crypto.randomUUID(),startDate:date,start,end:'',nextDay:false,category:'',project:'',direction:'',goal:'',note:'',schedule:'flexible'};
}
export function rowRange(row:TimeRow){const s=parseClock(row.start),e=parseClock(row.end);return {start:Date.parse(stamp(row.startDate,s)),end:Date.parse(stamp(row.nextDay?(row.endDate||addDays(row.startDate,1)):row.startDate,e))};}
export function rowChanged(row:TimeRow,kind:TimeKind,data:Data){if(!row.original)return !row.deleted;if(row.deleted)return true;const original=rowFromRecord(row.original,kind,data);return ['startDate','start','end','nextDay','endDate','category','project','direction','goal','note','schedule'].some(k=>row[k as keyof TimeRow]!==original[k as keyof TimeRow]);}
export function draftErrors(rows:TimeRow[],kind:TimeKind,data:Data){
 const errors:Record<string,string>={};const live=rows.filter(r=>!r.deleted);
 for(const r of live){if(!rowChanged(r,kind,data))continue;const {start,end}=rowRange(r);if(!Number.isFinite(start)||!Number.isFinite(end)||parseClock(r.start)>=1440||end<=start)errors[r.key]='请填写有效时间，结束须晚于开始；跨日请选择次日';else if(!r.category)errors[r.key]='请选择时间类别';}
 if(kind==='time')for(let i=0;i<live.length;i++){const a=rowRange(live[i]);for(let j=i+1;j<live.length;j++){const b=rowRange(live[j]);if(a.start<b.end&&a.end>b.start&&(rowChanged(live[i],kind,data)||rowChanged(live[j],kind,data)))errors[live[i].key]=errors[live[j].key]='这两个时段重叠，请调整';}}
 return errors;
}
export function draftSummary(rows:TimeRow[],date:string,kind:TimeKind,now=Date.now()){
 const lo=Date.parse(stamp(date,0)),hi=kind==='time'?Math.min(Date.parse(stamp(date,1440)),now):Date.parse(stamp(date,1440));
 const ranges=rows.filter(r=>!r.deleted).map(rowRange).filter(r=>Number.isFinite(r.start)&&Number.isFinite(r.end)&&r.end>r.start).map(r=>({start:Math.max(lo,r.start),end:Math.min(hi,r.end)})).filter(r=>r.end>r.start).sort((a,b)=>a.start-b.start);
 const gaps:{start:number;end:number}[]=[];let cursor=lo,minutes=0;
 for(const r of ranges){if(r.start>cursor)gaps.push({start:(cursor-lo)/60000,end:(r.start-lo)/60000});minutes+=Math.max(0,r.end-Math.max(cursor,r.start))/60000;cursor=Math.max(cursor,r.end);}
 if(cursor<hi)gaps.push({start:(cursor-lo)/60000,end:(hi-lo)/60000});return {minutes,gaps};
}
export function draftOperations(rows:TimeRow[],kind:TimeKind,data:Data):{operations:Operation[];keys:string[]}{
 const operations:Operation[]=[],keys:string[]=[];
 const push=(op:Operation,key:string)=>{operations.push(op);keys.push(key);};
 for(const r of rows){if(!rowChanged(r,kind,data))continue;if(r.deleted){if(r.original)push({kind,action:'delete',id:r.original.id,expected_version:r.original.version},r.key);continue;}
 const cat=data.categories.find(c=>c.id===r.category),work=r.category==='work';
 const legacy=r.original?.work_kind==='legacy'&&r.category===r.original.category_id;
 const values:Record<string,unknown>={title:r.note.trim()||(work?'工作':cat?.name||r.original?.title||'时间记录'),category_id:work?'system:work':r.category,work_kind:work?'work':legacy?'legacy':'life',life_category:work?null:cat?.name||null,category_manual:1,long_project_id:r.project||null,direction_id:r.direction||null,primary_goal_id:r.goal||null};
 // Omitted links stay untouched on updates. New backfill rows retain their source and task.
 if(!r.original){values.item_id=r.itemId||null;if(kind==='time')values.source_plan_id=r.sourcePlan||null;}
 const op:Operation={kind,action:r.original?'update':'create',id:r.original?.id,expected_version:r.original?.version,values};
 if(kind==='time'){const range=rowRange(r);values.start_at=new Date(range.start).toISOString();values.end_at=new Date(range.end).toISOString();push(op,r.key);}
 else {const start=parseClock(r.start),end=parseClock(r.end);Object.assign(values,{date:r.startDate,start_min:start,end_min:r.nextDay?1440:end,schedule_kind:r.schedule});push(op,r.key);if(r.nextDay&&end>0)push({kind:'plan',action:'create',values:{...values,date:addDays(r.startDate,1),start_min:0,end_min:end,item_id:r.itemId||null}},r.key);}
 }
 return {operations,keys};
}
export function backfillRows(data:Data,plans:PlanBlock[],rows:TimeRow[]){return plans.filter(p=>!rows.some(r=>r.sourcePlan===p.id)&&!data.timeEntries.some(e=>e.source_plan_id===p.id||(p.item_id&&e.item_id===p.item_id&&Date.parse(e.start_at)<=Date.parse(stamp(p.date,p.start_min))&&Date.parse(e.end_at)>=Date.parse(stamp(p.date,p.end_min))))).map(p=>{
 const r=rowFromRecord(p,'plan',data),item=data.items.find(i=>i.id===p.item_id);return {...r,key:crypto.randomUUID(),original:undefined,sourcePlan:p.id,category:r.category||(item?'work':''),project:r.project||item?.project_id||'',goal:r.goal||item?.annual_goal_id||'',direction:r.direction||data.annualGoals.find(g=>g.id===item?.annual_goal_id)?.direction_id||''};
 });}
