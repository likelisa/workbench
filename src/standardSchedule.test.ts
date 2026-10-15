import { expect,it } from 'vitest';
import { actualSpan,scheduleOperations,scheduleRow,spanPosition,timelineHourLabels,timelineLanes,timelineSegments,TIMELINE } from './standardSchedule';
import { EMPTY,type TimeEntry } from './model';
import { stamp } from './timeEditorModel';
it('starts at 07:00 and places the same calendar day’s midnight hours at the bottom',()=>{
 expect(timelineHourLabels[0]).toBe('07:00');expect(timelineHourLabels[17]).toBe('00:00');expect(timelineHourLabels[24]).toBe('07:00');
 const [sleep]=timelineSegments([{id:'sleep',start:0,end:420}]);
 expect(spanPosition(sleep.displayStart,sleep.displayEnd)).toEqual({top:1020,height:420});
 const [work]=timelineSegments([{start:420,end:480}]);expect(spanPosition(work.displayStart,work.displayEnd)).toEqual({top:0,height:60});expect(TIMELINE.height).toBe(1440);
});
it('splits oversleep at 07:00 without duplicating minutes or changing the original record',()=>{
 const record={id:'sleep',start:0,end:510},before={...record};const blocks=timelineSegments([record]);
 expect(blocks.map(b=>[b.id,b.start,b.end,b.displayStart,b.displayEnd])).toEqual([['sleep',420,510,0,90],['sleep',0,420,1020,1440]]);
 expect(blocks.reduce((sum,b)=>sum+b.end-b.start,0)).toBe(510);expect(record).toEqual(before);
 const allDay=timelineSegments([{start:0,end:1440}]);expect(allDay.reduce((sum,b)=>sum+b.displayEnd-b.displayStart,0)).toBe(1440);
 expect(timelineSegments([{start:420,end:420}])).toEqual([]);
});
it('clips one cross-midnight record into matching local-day coordinates',()=>{
 const e={id:'e',start_at:stamp('2026-10-14',1410),end_at:stamp('2026-10-15',60)} as TimeEntry;
 expect(actualSpan(e,'2026-10-14')).toEqual({id:'e',start:1410,end:1440});expect(actualSpan(e,'2026-10-15')).toEqual({id:'e',start:0,end:60});
 expect(timelineSegments([actualSpan(e,'2026-10-14')])[0].displayStart).toBe(990);
 expect(timelineSegments([actualSpan(e,'2026-10-15')])[0].displayStart).toBe(1020);
});
it('keeps overlapping historical blocks visible in lanes without inflating short durations',()=>{
 const blocks=timelineLanes([{id:'a',start:0,end:120},{id:'b',start:60,end:90},{id:'c',start:90,end:110},{id:'d',start:120,end:121}]);
 expect(blocks.map(b=>[b.lane,b.lanes])).toEqual([[0,2],[1,2],[1,2],[0,1]]);expect(spanPosition(120,121).height).toBe(1);
});
it('saves shared standard schedule without dates, time entries or daily plan copies',()=>{
 const row=scheduleRow({id:'s',title:'睡觉',start_min:0,end_min:420,created_at:'',version:'v1'});
 expect(scheduleOperations([row]).operations).toHaveLength(0);const edited=scheduleOperations([{...row,end:'08:00'}]);expect(edited.errors).toEqual({});expect(edited.operations).toEqual([{kind:'dailyPlan',action:'update',id:'s',expected_version:'v1',values:{start_min:0,end_min:480,title:'睡觉'}}]);expect(EMPTY.timeEntries).toHaveLength(0);
});
it('rejects reversed times and blank activities, permits midnight endings and undo',()=>{
 expect(scheduleOperations([{key:'n',title:'',start:'10:00',end:'09:00'}]).errors.n).toBeTruthy();expect(scheduleOperations([{key:'n',title:'休息',start:'23:00',end:'24:00'}]).errors).toEqual({});const row=scheduleRow({id:'s',title:'睡觉',start_min:0,end_min:420,created_at:'',version:'v1'});expect(scheduleOperations([{...row,deleted:true}]).operations[0].action).toBe('delete');expect(scheduleOperations([{...row,deleted:false}]).operations).toHaveLength(0);
});
