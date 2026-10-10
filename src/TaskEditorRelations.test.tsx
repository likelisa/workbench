import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import TaskEditorRelations from './TaskEditorRelations';
import AssignmentWeeks from './AssignmentWeeks';
import TaskManagement from './TaskManagement';
import { EMPTY, cyclePlanTitle, weekDateRange, type Data } from './model';

const noop=()=>{};
const cycle={id:'c',title:'周期',start_date:'2026-10-01',review:'',created_at:''};
const data:Data={...EMPTY,cycles:[cycle],tasks:[{id:'t',title:'本轮待办',cycle_id:'c',annual_goal_id:null,due_date:null,status:'todo',created_at:'',project_id:null,cycle_goal_id:null}],assignments:[{id:'w',task_id:'t',cycle_id:'c',commitment:'完整周任务',status:'doing',priority:'P1',review:'复盘内容',created_at:''}],assignmentWeeks:[{assignment_id:'w',week_no:3},{assignment_id:'w',week_no:1}],dayTasks:[{id:'d',title:'关联日任务内容',assignment_id:'w',task_id:'t',project_id:null,date:'2026-10-08',status:'todo',priority:'P2',recurrence_id:null,scheduled_category_id:null,scheduled_start_at:'2026-10-08T09:00:00',scheduled_end_at:'2026-10-08T10:00:00',created_at:''}]};
const render=(kind:'task'|'assignment',value=data)=>renderToStaticMarkup(<TaskEditorRelations kind={kind} recordId={kind==='task'?'t':'w'} data={value} disabled={false} onCreate={noop} onEdit={noop} onDelete={noop} onSchedule={noop} onUnschedule={noop}/>);

describe('统一编辑弹窗详情',()=>{
  it('周任务显示周次和关联日任务完整信息，并保留不匹配提示',()=>{
    const markup=render('assignment');
    for(const text of ['周次管理','10月1日–10月7日','10月15日–10月21日','关联日任务内容','2026-10-08','09:00—10:00','日期与周次不匹配','P2','删除关联日任务内容'])expect(markup).toContain(text);
    expect(markup.indexOf('移除第1周安排')).toBeLessThan(markup.indexOf('移除第3周安排'));
  });
  it('本轮待办显示下层周任务和操作，空列表给出提示',()=>{
    const markup=render('task');
    for(const text of ['下层周任务','完整周任务','P1','删除完整周任务','＋ 周任务'])expect(markup).toContain(text);
    expect(render('task',{...data,assignments:[]})).toContain('暂无下层周任务');
  });
  it('无周次或无周期时禁用新增关联日任务，不删除原有关联',()=>{
    const unassigned=render('assignment',{...data,assignmentWeeks:[]});
    expect(unassigned).toContain('title="请先安排周次"');expect(unassigned).toContain('关联日任务内容');
    const noCycle=render('assignment',{...data,cycles:[]});expect(noCycle).toContain('请先为所属待办指定周期');
    expect(noCycle).toMatch(/disabled=""[^>]*aria-label="安排到周"/);
  });
  it.each(['task','assignment'] as const)('任务池 %s 不再渲染详情入口或侧栏',kind=>{
    const markup=renderToStaticMarkup(<TaskManagement kind={kind} data={data} cycleId="c" onCreate={noop} onEdit={noop} onStatus={noop}/>);
    expect(markup).not.toContain('详情');expect(markup).not.toContain('task-detail');expect(markup).toContain('compact-task-main');
  });
  it('12周全部安排后禁用安排控件',()=>{
    const markup=renderToStaticMarkup(<AssignmentWeeks cycle={cycle} weeks={Array.from({length:12},(_,i)=>i+1)} onSchedule={noop} onUnschedule={noop}/>);
    expect(markup).toContain('已安排全部周次');expect(markup).toMatch(/disabled=""[^>]*aria-label="安排到周"/);
  });
});

describe('周期日期和标题',()=>{
  it('以周期开始日计算一周，包含完整的首尾日期',()=>{
    expect(weekDateRange(cycle,1)).toBe('10月1日–10月7日');
    expect(weekDateRange(cycle,12)).toBe('12月17日–12月23日');
    expect(weekDateRange({...cycle,start_date:'2026-10-28'},1)).toBe('10月28日–11月3日');
  });
  it('跨年时保留两端年份',()=>{
    expect(weekDateRange({...cycle,start_date:'2026-12-29'},1)).toBe('2026年12月29日–2027年1月4日');
  });
  it('标题随日期和所选周期变化，周期范围以外不显示周数',()=>{
    expect(cyclePlanTitle(cycle,'2026-10-01')).toBe('12周计划 · 第1周');
    expect(cyclePlanTitle(cycle,'2026-10-08')).toBe('12周计划 · 第2周');
    expect(cyclePlanTitle(cycle,'2026-12-23')).toBe('12周计划 · 第12周');
    expect(cyclePlanTitle({...cycle,start_date:'2026-10-08'},'2026-10-08')).toBe('12周计划 · 第1周');
    for(const today of ['2026-09-30','2026-12-24'])expect(cyclePlanTitle(cycle,today)).toBe('12周计划');
    expect(cyclePlanTitle(undefined,'2026-10-08')).toBe('12周计划');
  });
});
