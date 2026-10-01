import {describe,it,expect} from 'vitest';
import {calendarTaskChoices,calendarTaskPlacement,calendarWeekNumber} from './calendarWeekTask';
import {EMPTY,type Data,type Cycle} from './model';
const cycle=(id:string,start_date:string):Cycle=>({id,start_date,title:id,review:'',created_at:''});
describe('日程周任务归属',()=>{
 it('使用待办自己的周期计算周次，即使两个周期重叠',()=>{
  const data={...EMPTY,cycles:[cycle('old','2026-09-28'),cycle('own','2026-10-01')],tasks:[{id:'t',cycle_id:'own'}]} as Data;
  expect(calendarTaskPlacement(data,'t','2026-10-08')?.week).toBe(2);
  expect(calendarTaskPlacement(data,'t','2026-10-08')?.cycle.id).toBe('own');
 });
 it('非周一开始的首周、末周和跨年均能安排，范围外拒绝',()=>{
  const c=cycle('c','2026-10-01');
  expect(calendarWeekNumber(c,'2026-09-28')).toBe(1);
  expect(calendarWeekNumber(c,'2026-12-27')).toBe(12);
  expect(calendarWeekNumber(c,'2026-09-21')).toBeNull();
  expect(calendarWeekNumber(c,'2026-12-28')).toBeNull();
  expect(calendarWeekNumber(cycle('cross','2026-10-26'),'2027-01-10')).toBe(11);
 });
 it('按方向、目标及查看周筛选，旧的未归属待办仍可直接选择',()=>{
  const data={...EMPTY,cycles:[cycle('c','2026-10-01'),cycle('future','2027-03-01')],annualGoals:[{id:'a',direction_id:'d1'},{id:'b',direction_id:'d2'}],tasks:[{id:'one',annual_goal_id:'a',cycle_id:'c'},{id:'two',annual_goal_id:'b',cycle_id:'c'},{id:'orphan',annual_goal_id:null,cycle_id:'c'},{id:'future',annual_goal_id:'a',cycle_id:'future'},{id:'no-cycle',annual_goal_id:'a',cycle_id:null}]} as Data;
  expect(calendarTaskChoices(data,'2026-10-01','d1').map(t=>t.id)).toEqual(['one']);
  expect(calendarTaskChoices(data,'2026-10-01','','b').map(t=>t.id)).toEqual(['two']);
  expect(calendarTaskChoices(data,'2026-10-01').map(t=>t.id)).toEqual(['one','two','orphan']);
  expect(calendarTaskPlacement(data,'no-cycle','2026-10-01')).toBeNull();
 });
});
