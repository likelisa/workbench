import { describe, expect, it } from 'vitest';
import { annualGoalTasks, assignmentInWeek, assignmentsForDate, assignmentsInRange, calendarRange, categoryTotals, cycleEndDate, dayTaskProjectId, dayTaskTaskId, EMPTY, historyYearGroups, minutesForEntryInDay, mondayOf, progress, shiftCalendarDate, validateTimeEntry, type Data, type TimeEntry } from './model';

const entry = (id:string,start:string,end:string,category_id='c'):TimeEntry => ({id,start_at:new Date(start).toISOString(),end_at:new Date(end).toISOString(),title:id,category_id,day_task_id:null,project_id:null,task_id:null,created_at:''});
describe('任务与时间统计',()=>{
  it('一条本轮待办跨两周仍只计入年度目标完成率一次',()=>{
    const data={tasks:[{id:'t',annual_goal_id:'annual',status:'done'}],assignments:[{task_id:'t',week_no:1},{task_id:'t',week_no:2}]} as unknown as Data;
    const tasks=annualGoalTasks(data,{id:'annual'} as Data['annualGoals'][number]);
    expect(tasks).toHaveLength(1);
    expect(progress(tasks)?.percent).toBe(100);
    expect(progress([])).toBeNull();
  });
  it('跨午夜时间按日期拆分，分类比例以已记录时间为分母',()=>{
    const entries=[entry('a','2026-09-29T23:00:00','2026-09-30T01:00:00'),entry('b','2026-09-29T12:00:00','2026-09-29T13:00:00','d')];
    expect(minutesForEntryInDay(entries[0],'2026-09-29')).toBe(60);
    expect(minutesForEntryInDay(entries[0],'2026-09-30')).toBe(60);
    const totals=categoryTotals(entries,[{id:'c',name:'工作',color:'#fff',created_at:''},{id:'d',name:'生活',color:'#000',created_at:''}],'2026-09-29','2026-09-30');
    expect(totals.total).toBe(120);
    expect(totals.rows[0].percent).toBe(50);
  });
  it('拒绝重叠的实际记录',()=>{
    const entries=[entry('a','2026-09-29T09:00:00','2026-09-29T10:00:00')];
    expect(validateTimeEntry(entries,new Date('2026-09-29T09:30:00').toISOString(),new Date('2026-09-29T10:30:00').toISOString())).toContain('重叠');
    expect(validateTimeEntry(entries,new Date('2026-09-29T10:00:00').toISOString(),new Date('2026-09-29T11:00:00').toISOString())).toBeNull();
  });
  it('临时日任务后来关联周安排后，能归入对应项目',()=>{
    const data={tasks:[{id:'t',project_id:'p'}],assignments:[{id:'a',task_id:'t'}]} as unknown as Data;
    expect(dayTaskProjectId(data,{assignment_id:'a',project_id:null} as Data['dayTasks'][number])).toBe('p');
  });
  it('年度目标汇总不同周期的本轮待办',()=>{
    const data={tasks:[{id:'a',cycle_id:'c1',annual_goal_id:'annual',status:'done'},{id:'b',cycle_id:'c2',annual_goal_id:'annual',status:'todo'}]} as unknown as Data;
    const tasks=annualGoalTasks(data,{id:'annual'} as Data['annualGoals'][number]);
    expect(tasks).toHaveLength(2);
    expect(progress(tasks)?.percent).toBe(50);
  });
  it('直接创建的本轮待办归入年度目标，日任务也能关联同一条待办',()=>{
    const data={tasks:[{id:'t',cycle_id:'c',annual_goal_id:'annual',project_id:null,status:'todo'}],assignments:[]} as unknown as Data;
    expect(annualGoalTasks(data,{id:'annual'} as Data['annualGoals'][number]).map(t=>t.id)).toEqual(['t']);
    expect(dayTaskTaskId(data,{task_id:'t',assignment_id:null,project_id:null} as Data['dayTasks'][number])).toBe('t');
    expect(dayTaskTaskId(data,{task_id:null,assignment_id:null,project_id:null} as Data['dayTasks'][number])).toBeNull();
  });
});

describe('历史情况分组',()=>{
  it('第 84 天仍属于进行中的周期，次日按结束年份归档',()=>{
    const cycle={id:'cross-year',title:'跨年计划',start_date:'2026-10-26',review:'',created_at:''};
    const data={cycles:[cycle],annualGoals:[],assignments:[],dayTasks:[]} as unknown as Data;
    expect(cycleEndDate(cycle)).toBe('2027-01-17');
    expect(historyYearGroups(data,'2027-01-17')[0].cycles).toHaveLength(0);
    expect(historyYearGroups(data,'2027-01-18')[0].cycles.map(item=>item.id)).toEqual(['cross-year']);
  });
  it('超过四个周期照实展示，无年度目标的年份仍保留，未归属日任务按日期归档',()=>{
    const cycles=Array.from({length:5},(_,i)=>({id:`c${i}`,title:`计划${i}`,start_date:`2025-0${i+1}-01`,created_at:''}));
    const dayTasks=[{id:'old',date:'2025-07-01',assignment_id:null},{id:'today',date:'2026-09-30',assignment_id:null},{id:'linked',date:'2025-07-02',assignment_id:'assignment'}];
    const data={cycles,annualGoals:[],assignments:[{id:'assignment'}],dayTasks} as unknown as Data;
    const groups=historyYearGroups(data,'2026-09-30');
    expect(groups.find(group=>group.year===2025)?.cycles).toHaveLength(5);
    expect(groups.find(group=>group.year===2025)?.annualGoals).toHaveLength(0);
    expect(groups.find(group=>group.year===2025)?.unlinkedDayTasks.map(task=>task.id)).toEqual(['old']);
    expect(groups.find(group=>group.year===2026)?.unlinkedDayTasks).toHaveLength(0);
  });
});

describe('日程时间维度',()=>{
  it('周一开始一周，跨月和跨年导航保持正确范围',()=>{
    expect(mondayOf('2026-10-01')).toBe('2026-09-28');
    expect(calendarRange('2026-10-01','week')).toEqual(['2026-09-28','2026-10-05']);
    expect(calendarRange('2026-12-31','month')).toEqual(['2026-12-01','2027-01-01']);
    expect(calendarRange('2026-12-31','year')).toEqual(['2026-01-01','2027-01-01']);
    expect(shiftCalendarDate('2026-12-31','month',1)).toBe('2027-01-01');
    expect(shiftCalendarDate('2026-12-31','year',1)).toBe('2027-01-01');
  });
  it('周安排跨两个日历月时，在相交月份中均可见',()=>{
    const data={cycles:[{id:'cycle',start_date:'2026-09-30'}],assignments:[{id:'assignment',cycle_id:'cycle'}],assignmentWeeks:[{assignment_id:'assignment',week_no:1}]} as unknown as Data;
    expect(assignmentsInRange(data,'2026-09-01','2026-10-01').map(a=>a.id)).toEqual(['assignment']);
    expect(assignmentsInRange(data,'2026-10-01','2026-11-01').map(a=>a.id)).toEqual(['assignment']);
    expect(assignmentsInRange(data,'2026-11-01','2026-12-01')).toHaveLength(0);
  });
});

describe('周任务待办池',()=>{
    const makeData=()=>({...EMPTY,cycles:[{id:'cycle',title:'计划',start_date:'2026-09-30',review:'',created_at:''}],assignments:[
    {id:'shared',cycle_id:'cycle',task_id:'t',commitment:'持续推进',status:'todo' as const,review:'共用复盘',created_at:''},
    {id:'pool',cycle_id:'cycle',task_id:'t',commitment:'暂未安排',status:'todo' as const,review:'',created_at:''}
  ],assignmentWeeks:[{assignment_id:'shared',week_no:1},{assignment_id:'shared',week_no:3}]});
  it('按日期只选择对应周已安排项，并处理周期边界',()=>{
    const data=makeData();
    expect(assignmentsForDate(data,'2026-09-29')).toHaveLength(0);
    expect(assignmentsForDate(data,'2026-09-30').map(a=>a.id)).toEqual(['shared']);
    expect(assignmentsForDate(data,'2026-10-06').map(a=>a.id)).toEqual(['shared']);
    expect(assignmentsForDate(data,'2026-10-07')).toHaveLength(0);
    expect(assignmentsForDate(data,'2026-10-14')[0]).toBe(data.assignments[0]);
    expect(assignmentsForDate(data,'2026-12-23')).toHaveLength(0);
  });
  it('多周安排按同一任务汇总，移除全部周次后仍保留任务',()=>{
    const data=makeData();
    expect(assignmentsInRange(data,'2026-09-01','2026-11-01').map(a=>a.id)).toEqual(['shared']);
    expect(assignmentInWeek(data,'shared',2)).toBe(false);
    data.assignmentWeeks=[];
    expect(assignmentsForDate(data,'2026-09-30')).toHaveLength(0);
    expect(data.assignments).toHaveLength(2);
  });
});
