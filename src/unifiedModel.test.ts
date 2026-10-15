import { describe, it, expect } from 'vitest';
import { EMPTY, type Data } from './model';
import { activeCycle, completion, cycleTodos, dayTaskParents, dayTaskValues, executionWeeks, frozenOffset, itemsInWeek, itemsOnDay, overdueItems, timeDistribution, weekTaskValues, type Item, type Placement } from './unifiedModel';
const item=(id:string,status:Item['status']='todo'):Item=>({id,title:id,type:'todo',status,priority:'P2',description:'',project_id:null,parent_id:null,annual_goal_id:null,cycle_goal_id:null,cycle_id:null,level:'backlog',version_id:null,milestone_id:null,created_at:''});
const placement=(id:string,item_id:string,kind:'day'|'week',date:string):Placement=>({id,item_id,kind,date:kind==='day'?date:null,week_start:kind==='week'?date:null,cycle_id:null,week_no:null,created_at:''});
describe('统一事项执行查询',()=>{
 it('逾期仅包含待办和进行中，按日周粒度分别检查过去安排',()=>{
  for(const status of ['todo','doing','done','missed'] as const)for(const level of ['day','week'] as const)for(const offset of [-1,0,1]){
   const date=level==='day'?['2026-10-11','2026-10-12','2026-10-13'][offset+1]:['2026-10-05','2026-10-06','2026-10-07'][offset+1];
   const data:Data={...EMPTY,items:[{...item('a',status),level}],placements:[placement('p','a',level,date)]};
   expect(overdueItems(data,'2026-10-12').length,`${level} ${status} ${date}`).toBe(offset===-1&&(status==='todo'||status==='doing')?1:0);
  }
 });
 it('跨周以最后一周判断，忽略错误粒度安排和无安排事项，去重且不修改数据',()=>{
  const data:Data={...EMPTY,items:[{...item('day'),level:'day'},{...item('week'),level:'week'},{...item('unplanned'),level:'day'},{...item('cycle'),level:'cycle'},item('backlog')],placements:[placement('d1','day','day','2026-10-08'),placement('d2','day','day','2026-10-08'),placement('d3','day','week','2026-09-01'),placement('w1','week','week','2026-10-01'),placement('w2','week','week','2026-10-08'),placement('w3','week','day','2026-09-01'),placement('c','cycle','day','2026-09-01'),placement('b','backlog','week','2026-09-01')]};
  const before=JSON.stringify(data);
  expect(overdueItems(data,'2026-10-12').map(r=>r.kind)).toEqual(['day']);
  expect(overdueItems(data,'2026-10-15')).toEqual([{item:data.items[0],kind:'day',dates:['2026-10-08']},{item:data.items[1],kind:'week',weekStart:'2026-10-08',weekEnd:'2026-10-14'}]);
  expect(JSON.stringify(data)).toBe(before);
  data.placements=data.placements.filter(p=>p.kind!=='day'||p.item_id!=='day').filter(p=>p.kind!=='week'||p.item_id!=='week');
  expect(overdueItems(data,'2026-10-15')).toEqual([]);
 });
 it('逾期排序和状态切换保持父子独立，失败任务仍保留在原视图',()=>{
  const data:Data={...EMPTY,items:[{...item('day'),level:'day',parent_id:'week',priority:'P0'},{...item('week','doing'),level:'week',priority:'P3'},{...item('urgent','doing'),level:'week',priority:'P0'}],placements:[placement('d','day','day','2026-10-09'),placement('w','week','week','2026-10-01'),placement('u','urgent','week','2026-10-01')]};
  expect(overdueItems(data,'2026-10-12').map(r=>r.item.id)).toEqual(['urgent','week','day']);
  data.items[0].status='missed';expect(overdueItems(data,'2026-10-12')).toHaveLength(2);expect(itemsOnDay(data,'2026-10-09')[0].status).toBe('missed');expect(data.items[1].status).toBe('doing');
  data.items[0].status='todo';expect(overdueItems(data,'2026-10-12')).toHaveLength(3);
  data.items[1].status='done';expect(overdueItems(data,'2026-10-12')).toHaveLength(2);expect(data.items[0].status).toBe('todo');
  data.items[1].status='doing';expect(overdueItems(data,'2026-10-12')).toHaveLength(3);
 });
 it('周任务只关联本轮待办并继承目标，历史目标的保留身份不进入选项',()=>{
 const data:Data={...EMPTY,cycleGoals:[{id:'goal',title:'本轮目标',cycle_id:'c',annual_goal_id:'annual',source_item_id:'source',created_at:''}],items:[{...item('source'),level:'cycle',cycle_goal_id:'goal',cycle_id:'c',annual_goal_id:'annual'},{...item('todo'),level:'cycle',parent_id:'source',cycle_goal_id:'goal',cycle_id:'c',annual_goal_id:'annual'},item('other')]};
 expect(cycleTodos(data,'c').map(i=>i.id)).toEqual(['todo']);expect(weekTaskValues(data,{level:'week',parent_id:'todo',annual_goal_id:'wrong'},)).toMatchObject({parent_id:'todo',annual_goal_id:'annual',cycle_goal_id:'goal',cycle_id:'c'});expect(weekTaskValues(data,{level:'week',parent_id:'other'})).toMatchObject({parent_id:null,annual_goal_id:null,cycle_goal_id:null});
 });
 it('12周轨道按跨年周期边界定位，跨周去重且保留独立周安排',()=>{
 const data:Data={...EMPTY,cycles:[{id:'c',title:'跨年',start_date:'2026-12-03',review:'',created_at:''}],items:[item('multi'),item('independent'),item('other')],placements:[{...placement('1','multi','week','2026-12-31'),cycle_id:'c',week_no:5},{...placement('2','multi','week','2027-01-07'),cycle_id:'c',week_no:6},placement('3','multi','week','2027-01-04'),placement('4','independent','week','2027-01-04'),{...placement('5','other','week','2027-01-07'),cycle_id:'different',week_no:6}]};
 const weeks=executionWeeks(data,'2027-01-08');expect(weeks).toHaveLength(12);expect(weeks.filter(w=>w.current).map(w=>w.week)).toEqual([6]);expect(weeks[5]).toMatchObject({start:'2027-01-07',end:'2027-01-13'});expect(weeks[5].items.map(i=>i.id)).toEqual(['independent','multi']);expect(weeks[4].items.map(i=>i.id)).toContain('multi');
 });
 it('没有当前周期时显示自然周，不把历史或未来周期标为当前',()=>{
 const data:Data={...EMPTY,cycles:[{id:'old',title:'历史',start_date:'2025-01-01',review:'',created_at:''}]};
 const weeks=executionWeeks(data,'2026-10-12');expect(weeks).toHaveLength(12);expect(weeks[4]).toMatchObject({week:null,cycle_id:null,current:true,start:'2026-10-12',end:'2026-10-18'});
 });
 it('日任务使用当前周期，仅关联本周周任务，继承完整上级且子任务状态独立',()=>{
 const data:Data={...EMPTY,cycles:[{id:'old',title:'过去周期',start_date:'2026-07-01',created_at:'',review:''},{id:'current',title:'当前周期',start_date:'2026-10-01',created_at:'',review:''},{id:'future',title:'未来周期',start_date:'2027-01-01',created_at:'',review:''}],cycleGoals:[{id:'goal',title:'本轮目标',cycle_id:'current',annual_goal_id:'annual'} as Data['cycleGoals'][number]],items:[{...item('todo'),level:'cycle',project_id:'project',cycle_goal_id:'goal',cycle_id:'current'},{...item('week'),level:'week',parent_id:'todo'}, {...item('past'),level:'week'}, {...item('next'),level:'week'}, {...item('multi'),level:'week'}],placements:[placement('1','week','week','2026-10-08'),placement('2','past','week','2026-09-28'),placement('3','next','week','2026-10-12'),placement('4','multi','week','2026-09-28'),placement('5','multi','week','2026-10-05')]};
 expect(activeCycle(data,'2026-10-10')?.id).toBe('current');expect(dayTaskParents(data,'2026-10-10').map(i=>i.id).sort()).toEqual(['multi','week']);
 expect(dayTaskValues(data,{level:'day',parent_id:'week',cycle_id:'future',priority:'P0',status:'done'},'2026-10-10')).toMatchObject({cycle_id:'current',parent_id:'week',project_id:'project',annual_goal_id:'annual',cycle_goal_id:'goal',priority:'P0',status:'done'});
 expect(data.items.find(i=>i.id==='week')?.status).toBe('todo');
 expect(dayTaskValues(data,{level:'day',cycle_id:'future'},'2026-10-10')).toMatchObject({cycle_id:'current',parent_id:null,annual_goal_id:null,cycle_goal_id:null});
 expect(activeCycle(data,'2027-05-01')).toBeUndefined();
 });
 it('各视图引用同一身份，多周按最后结束日判逾期且不自动改期',()=>{
 const data:Data={...EMPTY,items:[{...item('a'),level:'day'},item('b','done'),item('c')],placements:[placement('1','a','day','2026-10-09'),placement('2','a','week','2026-10-05'),placement('3','a','week','2026-10-12'),placement('4','b','day','2026-10-08')]};
 expect(itemsOnDay(data,'2026-10-09')[0]).toBe(data.items[0]);expect(itemsInWeek(data,'2026-10-10')[0]).toBe(data.items[0]);
 expect(overdueItems(data,'2026-10-10')).toEqual([{item:data.items[0],kind:'day',dates:['2026-10-09']}]);expect(completion([data.items[0],data.items[0]])).toEqual({done:0,total:1});
 expect(data.placements[0].date).toBe('2026-10-09');data.items[0].status='done';expect(overdueItems(data,'2026-10-10')).toEqual([]);
 });
 it('冻结列在右侧、自然位置、左侧切换，四档缩放没有克隆',()=>{for(const scale of [.7,.85,1,1.25]){const tree=280*scale,width=164*scale,left=tree+width*6;expect(left+frozenOffset(left,width,0,900*scale,tree)).toBeCloseTo((900-164)*scale);expect(frozenOffset(left,width,600*scale,900*scale,tree)).toBe(0);expect(left+frozenOffset(left,width,1200*scale,900*scale,tree)).toBeCloseTo(1480*scale);}});
 it('跨午夜剪裁、主要归属只计一次、未来不计未记录',()=>{
 const data:Data={...EMPTY,timeEntries:[{id:'e',title:'开发',start_at:'2026-10-09T23:00:00+08:00',end_at:'2026-10-10T01:00:00+08:00',category_id:'x',day_task_id:null,project_id:null,task_id:null,created_at:'',work_kind:'work',primary_goal_id:null}]};
 const stats=timeDistribution(data,'2026-10-10','2026-10-12',new Date('2026-10-10T12:00:00+08:00'));expect(stats.total).toBe(720);expect(stats.work).toBe(60);expect(stats.life.get('未记录')).toBe(660);expect(stats.goals.get('未归属工作')).toBe(60);
 expect(timeDistribution(data,'2026-10-11','2026-10-12',new Date('2026-10-10T12:00:00+08:00')).total).toBe(0);
 });
});
