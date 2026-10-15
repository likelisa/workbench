import { describe,it,expect } from 'vitest';
import { EMPTY,type Data,type PlanBlock,type TimeEntry } from './model';
import { backfillRows,blankRow,dayEntries,draftErrors,draftOperations,draftSummary,monthDates,rowChanged,rowFromRecord,shiftMonth,stamp } from './timeEditorModel';
import { frozenOffset,timeDistribution } from './unifiedModel';
const data=():Data=>({...EMPTY,categories:[{id:'system:work',name:'工作',color:'#67D649',kind:'fixed',active:1,created_at:''},{id:'sleep',name:'睡觉',color:'#eee',kind:'fixed',active:1,created_at:''}]});
const plan=(extra:Partial<PlanBlock>={}):PlanBlock=>({id:'p',date:'2026-10-14',start_min:540,end_min:600,title:'工作',day_task_id:null,recurrence_id:null,created_at:'',work_kind:'work',category_id:'system:work',...extra});
const time=(extra:Partial<TimeEntry>={}):TimeEntry=>({id:'t',start_at:stamp('2026-10-14',1410),end_at:stamp('2026-10-15',30),title:'睡觉',category_id:'sleep',day_task_id:null,project_id:null,task_id:null,created_at:'',work_kind:'life',life_category:'睡觉',...extra});
describe('whole day time editor',()=>{
 it('keeps gap entries on the edited day after a midnight-ending record',()=>{
  const d=data(),last=rowFromRecord(time({start_at:stamp('2026-10-09',1350),end_at:stamp('2026-10-09',1440)}),'time',d);
  const next=blankRow('2026-10-09',last);
  expect(next).toMatchObject({startDate:'2026-10-09',start:'',nextDay:false});
  const op=draftOperations([{...next,start:'19:00',end:'20:30',category:'work'}],'time',d).operations[0];
  expect(op.values).toMatchObject({start_at:'2026-10-09T11:00:00.000Z',end_at:'2026-10-09T12:30:00.000Z'});
  expect(blankRow('2026-10-09',{...last,end:'24:00',nextDay:false}).start).toBe('');
  expect(blankRow('2026-10-09',{...last,end:'01:00',nextDay:true}).startDate).toBe('2026-10-09');
 });
 it('carries forward only an end time on the edited date',()=>{
  const r={...blankRow('2026-10-09'),end:'18:30'};
  expect(blankRow('2026-10-09',r).start).toBe('18:30');
  expect(blankRow('2026-10-10',{...r,end:'08:00',nextDay:true})).toMatchObject({startDate:'2026-10-10',start:'08:00'});
 });
 it('handles leap months and year boundaries',()=>{expect(monthDates('2024-02')).toHaveLength(29);expect(monthDates('2026-02')).toHaveLength(28);expect(shiftMonth('2026-12',1)).toBe('2027-01');expect(shiftMonth('2026-01',-1)).toBe('2025-12');});
 it('saves category only work and keeps optimistic versions and hidden links',()=>{const d=data(),r={...blankRow('2026-10-14'),start:'09:00',end:'10:00',category:'work'};expect(draftErrors([r],'time',d)).toEqual({});const op=draftOperations([r],'time',d).operations[0];expect(op.values).toMatchObject({work_kind:'work',category_id:'system:work',title:'工作',item_id:null,long_project_id:null});const old=rowFromRecord({...time(),item_id:'item',day_task_id:'day',source_plan_id:'p',version:'old'},'time',d);expect(rowChanged(old,'time',d)).toBe(false);const edit=draftOperations([{...old,end:'00:45'}],'time',d).operations[0];expect(edit.expected_version).toBe('old');expect(edit.values).not.toHaveProperty('item_id');expect(edit.values).not.toHaveProperty('day_task_id');expect(edit.values).not.toHaveProperty('source_plan_id');});
 it('preserves longer historical intervals',()=>{const d=data(),e=time({end_at:stamp('2026-10-17',30)}),r=rowFromRecord(e,'time',d);expect(rowChanged(r,'time',d)).toBe(false);expect(draftOperations([{...r,note:'补充备注'}],'time',d).operations[0].values?.end_at).toBe(new Date(e.end_at).toISOString());});
 it('requires explicit next day and rejects overlaps but accepts adjacency',()=>{const d=data(),a={...blankRow('2026-10-14'),start:'23:00',end:'01:00',category:'sleep'};expect(draftErrors([a],'time',d)[a.key]).toBeTruthy();expect(draftErrors([{...a,nextDay:true}],'time',d)).toEqual({});const b={...blankRow('2026-10-14'),start:'22:00',end:'23:30',category:'sleep'};expect(Object.keys(draftErrors([{...a,nextDay:true},b],'time',d))).toHaveLength(2);expect(draftErrors([{...a,nextDay:true},{...b,end:'23:00'}],'time',d)).toEqual({});});
 it('splits plans at midnight and preserves actual record identity',()=>{const d=data();const r={...rowFromRecord(plan(),'plan',d),start:'23:30',end:'00:30',nextDay:true};const ops=draftOperations([r],'plan',d).operations;expect(ops).toHaveLength(2);expect(ops[0].values).toMatchObject({end_min:1440});expect(ops[1].values).toMatchObject({date:'2026-10-15',start_min:0,end_min:30});d.timeEntries=[time()];expect(dayEntries(d,'2026-10-14')).toHaveLength(1);expect(dayEntries(d,'2026-10-15')).toHaveLength(1);expect(draftSummary([rowFromRecord(time(),'time',d)],'2026-10-14','time',Date.parse(stamp('2026-10-16',0))).minutes).toBe(30);});
 it('does not include future hours as unrecorded',()=>{const summary=draftSummary([],'2026-10-14','time',Date.parse(stamp('2026-10-14',600)));expect(summary.gaps).toEqual([{start:0,end:600}]);expect(draftSummary([],'2026-10-15','time',Date.parse(stamp('2026-10-14',600))).gaps).toEqual([]);});
 it('backfill remains draft and excludes existing source and linked coverage',()=>{const d=data();d.planBlocks=[plan()];const drafts=backfillRows(d,d.planBlocks,[]);expect(drafts).toHaveLength(1);expect(d.timeEntries).toHaveLength(0);expect(backfillRows(d,d.planBlocks,drafts)).toHaveLength(0);d.timeEntries=[time({source_plan_id:'p'})];expect(backfillRows(d,d.planBlocks,[])).toHaveLength(0);});
 it('keeps unclassified legacy plans untouched and deletes only explicitly removed rows',()=>{const d=data(),r=rowFromRecord(plan({work_kind:null,category_id:null}),'plan',d);expect(draftOperations([r],'plan',d).operations).toHaveLength(0);expect(draftErrors([r],'plan',d)).toEqual({});expect(draftOperations([{...r,deleted:true}],'plan',d).operations[0]).toMatchObject({action:'delete',id:'p'});});
 it('uses explicit direction snapshots without inventing goals',()=>{const d=data();d.directions=[{id:'dir',title:'开发',created_at:''}];d.timeEntries=[time({work_kind:'work',direction_id:'dir',category_id:'system:work'})];const stats=timeDistribution(d,'2026-10-14','2026-10-15',new Date(stamp('2026-10-16',0)));expect(stats.directions.get('开发')).toBe(60);expect(stats.goals.get('未归属工作')).toBe(60);});
 it('freezes one column at either edge and releases it inside viewport',()=>{expect(frozenOffset(292*13,292,0,900,0)).toBe(608-292*13);expect(frozenOffset(292*13,292,292*12,900,0)).toBe(0);expect(frozenOffset(292*13,292,292*15,900,0)).toBe(292*2);});
});
