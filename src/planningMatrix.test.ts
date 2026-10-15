import { describe, expect, it } from 'vitest';
import { EMPTY, type Data } from './model';
import { planningGeometry, planningRows, unscheduledPool, taskPool, DEFAULT_POOL_FILTERS, UNLINKED_POOL_FILTER, normalizePoolFilters, poolAnnualGoals } from './planningMatrix';
import type { Item, Placement } from './unifiedModel';

const item = (id: string, goal: string | null, annual = 'a', level: Item['level'] = 'cycle'): Item => ({
  id, title: id, type: 'todo', status: 'todo', priority: 'P2', description: '', project_id: null,
  parent_id: null, annual_goal_id: annual, cycle_goal_id: goal, cycle_id: 'c', level,
  version_id: null, milestone_id: null, created_at: '',
});
const data: Data = { ...EMPTY,
  directions: [{ id: 'd', title: '写作', created_at: '' }],
  annualGoals: ['a', 'b'].map(id => ({ id, direction_id: 'd', title: id, year: 2026, completed: 0, created_at: '' })),
  cycleGoals: [{ id: 'g1', annual_goal_id: 'a' }, { id: 'g2', annual_goal_id: 'a' }, { id: 'g3', annual_goal_id: 'b' }].map(g => ({ ...g, title: g.id, cycle_id: 'c', created_at: '' })),
  items: [item('t1', 'g1'), item('t2', 'g1'), item('t3', 'g2'), item('t4', 'g3', 'b'), item('t5', null)],
};

describe('合并单元格规划矩阵', () => {
  it('修正后的旧目标进入第三列，子待办进入第四列，目标保留身份不重复算任务', () => {
    const source={...item('source','g1'),title:'完成第1-7卷细纲'};
    const todo={...item('child','g1'),parent_id:'source',title:'细纲【关外篇】'};
    const fixture:Data={...data,cycleGoals:[{...data.cycleGoals[0],title:source.title,source_item_id:'source'}],items:[source,todo]};
    const rows=planningRows(fixture,'c',new Set());expect(rows.filter(r=>r.todo).map(r=>r.todo?.title)).toEqual(['细纲【关外篇】']);expect(rows[0].path[2].title).toBe('完成第1-7卷细纲');expect(unscheduledPool(fixture,'cycle').map(i=>i.id)).toEqual(['child']);
  });
  it('方向、年度目标、本轮目标分别跨越其全部待办，四列没有重复标题行', () => {
    const rows = planningRows(data, 'c', new Set());
    expect(rows.filter(r => r.todo).length).toBe(5);
    expect(rows[0].starts.map(g => [g.id, g.span])).toEqual([['dir:d', 5], ['annual:a', 4], ['goal:a:g1', 2]]);
    expect(rows[1].starts).toEqual([]);
    expect(rows.flatMap(r => r.starts).filter(g => g.id === 'annual:b')[0].span).toBe(1);
    expect(rows.at(-1)?.unassigned).toBe(true);
    expect(rows.map(r => r.id)).toEqual(['t1', 't2', 't3', 't5', 't4', 'unassigned']);
  });
  it('逐级折叠重新计算跨行数，空目标和未归属事项依然可见', () => {
    const folded = planningRows(data, 'c', new Set(['goal:a:g1']));
    expect(folded[0].folded).toBe(true);
    expect(folded[0].starts.map(g => g.span)).toEqual([4, 3, 1]);
    expect(planningRows(data, 'c', new Set(['dir:d'])).map(r => r.id)).toEqual(['dir:d', 'unassigned']);
    const empty = planningRows({ ...data, items: [] }, 'c', new Set());
    expect(empty.filter(r => r.path.length === 3)).toHaveLength(3);
    expect(empty[0].starts.map(g => g.span)).toEqual([3, 2, 1]);
    const unlinked = planningRows({ ...EMPTY, items: [{ ...item('standalone', null), annual_goal_id: null }] }, 'c', new Set());
    expect(unlinked[0].path.map(g => g.title)).toEqual(['未归属年度方向', '未归属年度目标', '未归属本轮目标']);
  });
  it('四档缩放为四个冻结层级列和当前周预留可用宽度', () => {
    for (const scale of [.7, .85, 1, 1.25]) for (const viewport of [520, 700, 1080]) {
      const geometry = planningGeometry(scale, viewport);
      expect(geometry.tree + geometry.week).toBeLessThanOrEqual(viewport);
      expect(geometry.columns.reduce((a, b) => a + b, 0)).toBeCloseTo(geometry.tree);
      expect(geometry.lefts[3] + geometry.columns[3]).toBeCloseTo(geometry.tree);
    }
  });
});

describe('三个未安排任务池', () => {
  it('按粒度和对应安排查询，跨视图安排不误删另一种待安排资格', () => {
    const placement = (id: string, kind: Placement['kind']): Placement => ({ id, item_id: id, kind, date: kind === 'day' ? '2026-10-10' : null, week_start: kind === 'week' ? '2026-10-05' : null, cycle_id: 'c', week_no: 2, created_at: '' });
    const fixture: Data = { ...EMPTY, items: [
      item('day-free', null, 'a', 'day'), item('day-scheduled', null, 'a', 'day'),
      item('week-free', null, 'a', 'week'), item('week-scheduled', null, 'a', 'week'),
      item('cycle-free', null), item('cycle-scheduled', null),
      { ...item('unbound', null, 'a', 'day'), cycle_id: null },
      { ...item('other-cycle', null, 'a', 'day'), cycle_id: 'other' },
    ], placements: [placement('day-free', 'week'), placement('day-scheduled', 'day'), placement('week-free', 'day'), placement('week-scheduled', 'week'), placement('cycle-scheduled', 'week')] };
    expect(unscheduledPool(fixture, 'day', 'c').map(i => i.id)).toEqual(['day-free', 'unbound']);
    expect(unscheduledPool(fixture, 'week', 'c').map(i => i.id)).toEqual(['week-free']);
    expect(unscheduledPool(fixture, 'cycle', 'c').map(i => i.id)).toEqual(['cycle-free']);
  });
});

describe('全量任务池', () => {
  const slot=(id:string,item_id:string,kind:Placement['kind'],cycle_id:string|null='c'):Placement=>({id,item_id,kind,cycle_id,date:kind==='day'?'2026-10-12':null,week_start:kind==='week'?'2026-10-08':null,week_no:2,created_at:''});
  it('所选周期、继承周期、安排周期与无周期任务可见，排除其他周期和项目积压项',()=>{
    const fixture:Data={...data,items:[item('parent','g1'),{...item('inherited',null,'','day'),annual_goal_id:null,cycle_id:null,parent_id:'parent'},item('current',null,'a','day'),{...item('global',null,'a','day'),cycle_id:null},{...item('other',null,'a','day'),cycle_id:'other'},{...item('cross',null,'a','day'),cycle_id:'other'},{...item('other-placement',null,'a','day'),cycle_id:null},item('backlog',null,'a','backlog')],placements:[slot('1','cross','week'),slot('2','cross','week'),slot('3','other-placement','week','other')]};
    const before=JSON.stringify(fixture);expect(taskPool(fixture,'day','c',DEFAULT_POOL_FILTERS).map(i=>i.id)).toEqual(['cross','current','global','inherited']);expect(JSON.stringify(fixture)).toBe(before);
  });
  it('安排按任务粒度和自身安排判断，保留四种状态，状态与优先级排序',()=>{
    const fixture:Data={...data,items:[{...item('todo',null,'a','day'),priority:'P0'}, {...item('doing',null,'a','day'),status:'doing'}, {...item('done',null,'a','day'),status:'done'}, {...item('missed',null,'a','day'),status:'missed'},item('weekly',null,'a','week'),item('parent',null),{...item('child',null,'a','day'),parent_id:'parent'},item('own-slot',null)],placements:[slot('1','todo','week'),slot('2','done','day'),slot('3','weekly','day'),slot('4','child','day'),slot('5','own-slot','week')]};
    expect(taskPool(fixture,'day','c',DEFAULT_POOL_FILTERS).map(i=>i.id)).toEqual(['doing','todo','child','missed','done']);
    expect(taskPool(fixture,'day','c',{...DEFAULT_POOL_FILTERS,arrangement:'scheduled'}).map(i=>i.id)).toEqual(['child','done']);
    expect(taskPool(fixture,'week','c',{...DEFAULT_POOL_FILTERS,arrangement:'unscheduled'}).map(i=>i.id)).toEqual(['weekly']);
    expect(taskPool(fixture,'cycle','c',{...DEFAULT_POOL_FILTERS,arrangement:'unscheduled'}).map(i=>i.id)).toEqual(['parent']);
    expect(taskPool(fixture,'cycle','c',{...DEFAULT_POOL_FILTERS,arrangement:'scheduled'}).map(i=>i.id)).toEqual(['own-slot']);
  });
  it('日任务沿周任务和待办继承目标，方向和目标与安排条件取交集',()=>{
    const fixture:Data={...data,items:[item('todo','g1'),{...item('weekly',null,'','week'),annual_goal_id:null,parent_id:'todo'},{...item('daily',null,'','day'),annual_goal_id:null,parent_id:'weekly'},{...item('unlinked',null,'','day'),annual_goal_id:null}],placements:[slot('1','daily','day')]};
    expect(taskPool(fixture,'day','c',{...DEFAULT_POOL_FILTERS,arrangement:'scheduled',direction:'d',annualGoal:'a'}).map(i=>i.id)).toEqual(['daily']);
    expect(taskPool(fixture,'day','c',{...DEFAULT_POOL_FILTERS,arrangement:'unscheduled',direction:'d',annualGoal:'a'})).toEqual([]);
    expect(taskPool(fixture,'day','c',{...DEFAULT_POOL_FILTERS,direction:UNLINKED_POOL_FILTER,annualGoal:UNLINKED_POOL_FILTER}).map(i=>i.id)).toEqual(['unlinked']);
  });
  it('选项目标随方向联动，新增改名立即可见，删除或转移后清理失效筛选',()=>{
    const fixture:Data={...data,directions:[...data.directions,{id:'new',title:'开发',created_at:''}],annualGoals:[...data.annualGoals,{...data.annualGoals[0],id:'new-goal',direction_id:'new',title:'新目标'}]};
    expect(poolAnnualGoals(fixture,'new').map(g=>g.title)).toEqual(['新目标']);
    expect(normalizePoolFilters(fixture,{...DEFAULT_POOL_FILTERS,direction:'new',annualGoal:'a'}).annualGoal).toBe('');
    expect(normalizePoolFilters({...fixture,directions:[]},{...DEFAULT_POOL_FILTERS,direction:'new',annualGoal:'new-goal'})).toMatchObject({direction:'',annualGoal:'new-goal'});
    expect(normalizePoolFilters({...fixture,annualGoals:[]},{...DEFAULT_POOL_FILTERS,direction:'new',annualGoal:'new-goal'})).toMatchObject({direction:'new',annualGoal:''});
    const renamed={...fixture,annualGoals:fixture.annualGoals.map(g=>({...g,title:'改名'}))};expect(poolAnnualGoals(renamed,'new')[0].title).toBe('改名');
  });
  it('四种状态和四档优先级与已有筛选交集，空优先级按列表归入P2',()=>{
    const fixture:Data={...data,items:[{...item('a',null,'a','day'),status:'todo',priority:'P0'},{...item('b',null,'a','day'),status:'done',priority:'P1'},{...item('c',null,'a','day'),status:'missed',priority:'P2'},{...item('d',null,'a','day'),status:'doing',priority:'P3'},{...item('legacy',null,'a','day'),status:'done',priority:null}],placements:[slot('1','b','day'),slot('2','legacy','day')]};
    const before=JSON.stringify(fixture);
    for(const status of ['todo','done','missed','doing'] as const)expect(taskPool(fixture,'day','c',{...DEFAULT_POOL_FILTERS,status}).every(i=>i.status===status)).toBe(true);
    for(const priority of ['P0','P1','P2','P3'] as const)expect(taskPool(fixture,'day','c',{...DEFAULT_POOL_FILTERS,priority}).map(i=>i.id).sort()).toEqual(({P0:['a'],P1:['b'],P2:['c','legacy'],P3:['d']})[priority]);
    expect(taskPool(fixture,'day','c',{arrangement:'scheduled',direction:'d',annualGoal:'a',status:'done',priority:'P2'}).map(i=>i.id)).toEqual(['legacy']);
    expect(taskPool(fixture,'day','c',{...DEFAULT_POOL_FILTERS,status:'doing',priority:'P0'})).toEqual([]);expect(taskPool(fixture,'day','c',DEFAULT_POOL_FILTERS)).toHaveLength(5);expect(JSON.stringify(fixture)).toBe(before);
  });
  it('历史目标保留身份不成为任务池中的重复待办',()=>{
    const fixture:Data={...data,cycleGoals:[{...data.cycleGoals[0],source_item_id:'source'}],items:[item('source','g1'),item('todo','g1')]};
    expect(taskPool(fixture,'cycle','c',DEFAULT_POOL_FILTERS).map(i=>i.id)).toEqual(['todo']);
  });
});
