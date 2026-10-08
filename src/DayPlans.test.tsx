import { describe, expect, it } from 'vitest';
import { renderToStaticMarkup } from 'react-dom/server';
import DayPlans from './DayPlans';
import WeekTimeline from './WeekTimeline';
import { EMPTY, type Data } from './model';
const noop=()=>{};
const renderDay=(data:Data)=>renderToStaticMarkup(<DayPlans data={data} date="2026-10-07" onCreate={noop} onEdit={noop} onDelete={noop} onEditLegacy={noop} onDeleteLegacy={noop}/>);
const renderWeek=(data:Data)=>renderToStaticMarkup(<WeekTimeline data={data} date="2026-10-07" onCreateTemplate={noop} onEditTemplate={noop} onDeleteTemplate={noop} onCreateEntry={noop} onEditEntry={noop} onDeleteEntry={noop}/>);
describe('周与日共用每日规划',()=>{
  it('两种视图随同一数据源新增、修改、删除',()=>{
    const data:Data={...EMPTY,dailyPlanTemplates:[{id:'plan',title:'共用标题',start_min:540,end_min:600,created_at:''}]};
    for(const render of [renderDay,renderWeek])expect(render(data)).toContain('共用标题');
    const changed={...data,dailyPlanTemplates:[{...data.dailyPlanTemplates[0],title:'更新标题',start_min:660,end_min:720}]};
    for(const render of [renderDay,renderWeek]){expect(render(changed)).toContain('更新标题');expect(render(changed)).toContain('11:00');expect(render(changed)).not.toContain('共用标题');}
    for(const render of [renderDay,renderWeek])expect(render({...changed,dailyPlanTemplates:[]})).not.toContain('更新标题');
  });
  it('按日期保留旧记录，不将它们扩散为每天重复的模板',()=>{
    const data:Data={...EMPTY,planBlocks:[{id:'old',date:'2026-10-07',title:'原单日记录',start_min:600,end_min:660,day_task_id:null,recurrence_id:null,created_at:''}]};
    expect(renderDay(data)).toContain('原单日记录');expect(renderDay(data)).toContain('历史单日记录');expect(renderWeek(data)).not.toContain('原单日记录');
  });
});
