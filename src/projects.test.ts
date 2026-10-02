import { describe,it,expect } from 'vitest';
import { EMPTY,type Data } from './model';
import { DEFAULT_FILTERS,filterProjects,projectStats,planExecution,type LongProject } from './projects';
const p=(id:string,type='product'):LongProject=>({id,title:id,type_id:type,description:'长期产品',focus:'',links:'',status:'active',favorite:0,created_at:'2026-01-01T00:00:00Z',updated_at:'2026-10-01T00:00:00Z'});
const fixture=()=>({...EMPTY,longProjects:[p('p'),p('q','media')],annualGoals:[{id:'g',direction_id:'d',year:2026,title:'年度目标',completed:0,created_at:''},{id:'g2',direction_id:'d2',year:2026,title:'目标2',completed:0,created_at:''}],tasks:[{id:'t',annual_goal_id:'g',cycle_id:'c',title:'待办',status:'doing',due_date:'2026-09-01'},{id:'t2',annual_goal_id:'g2',cycle_id:'c2',title:'待办2',status:'todo'}],taskProjects:[{task_id:'t',project_id:'p'},{task_id:'t',project_id:'q'},{task_id:'t2',project_id:'p'}],assignments:[{id:'w',task_id:'t',long_project_id:'p',status:'doing'}],dayTasks:[{id:'d',long_project_id:'p'}],timeEntries:[{id:'e',long_project_id:'p',start_at:'2026-10-01T23:00:00Z',end_at:'2026-10-02T01:00:00Z'}]} as Data);
describe('长期项目查询',()=>{
 it('年度方向与周期必须由同一待办匹配，避免组合误匹配',()=>{const d=fixture();expect(filterProjects(d,{...DEFAULT_FILTERS,directions:['d'],cycles:['c2']})).toHaveLength(0);expect(filterProjects(d,{...DEFAULT_FILTERS,directions:['d'],cycles:['c']}).map(x=>x.id)).toEqual(['p','q']);});
 it('同一待办关联多个项目，各项目只展示一次',()=>{const d=fixture();expect(filterProjects(d,{...DEFAULT_FILTERS,goals:['g','g2']}).map(x=>x.id)).toEqual(['p','q']);});
 it('未手动选择项目的周任务不因待办关系归入项目',()=>{const d=fixture();d.assignments[0].long_project_id=null;expect(projectStats(d,'p').weeks).toHaveLength(0);expect(filterProjects(d,{...DEFAULT_FILTERS,doing:true})).toHaveLength(0);});
 it('类型状态搜索与逾期筛选组合生效',()=>{const d=fixture();expect(filterProjects(d,{...DEFAULT_FILTERS,types:['product'],query:'长期',overdue:true},'2026-10-01').map(x=>x.id)).toEqual(['p']);d.longProjects[0].status='archived';expect(filterProjects(d,DEFAULT_FILTERS).map(x=>x.id)).toEqual(['q']);});
 it('实际时间按明确归属统计，跨午夜不遗漏，不重复计入其他项目',()=>{const d=fixture();expect(projectStats(d,'p').minutes).toBe(120);expect(projectStats(d,'q').minutes).toBe(0);expect(projectStats(d,'p').tasks).toHaveLength(2);});
 it('关联查询按任务ID去重，取消关联后不显示',()=>{const d=fixture();d.planTasks=[{plan_id:'a',task_id:'t'},{plan_id:'a',task_id:'t'}];d.planWeeks=[{plan_id:'a',assignment_id:'w'}];expect(planExecution(d,'a').tasks).toHaveLength(1);expect(planExecution(d,'a').weeks).toHaveLength(1);expect(planExecution(d,'b').tasks).toHaveLength(0);});
});
