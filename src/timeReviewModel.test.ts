import { expect,it } from 'vitest';
import { EMPTY,type Cycle,type TimeEntry } from './model';
import { chartColor,chartRows,defaultReviewCycle,shanghaiDate,statisticsRange } from './timeReviewModel';
import { timeDistribution } from './unifiedModel';
const cycle:Cycle={id:'c',title:'跨年周期',start_date:'2026-11-02',review:'',created_at:''};
const now=new Date('2026-10-14T15:00:00+08:00');
it('uses six exact ranges and excludes today from recent periods',()=>{
 const expected={today:['2026-10-14','2026-10-14'],seven:['2026-10-07','2026-10-13'],week:['2026-10-12','2026-10-14'],thirty:['2026-09-14','2026-10-13'],month:['2026-10-01','2026-10-14']};
 for(const [key,[from,to]] of Object.entries(expected)){const r=statisticsRange(key as keyof typeof expected,now)!;expect([r.from,r.to]).toEqual([from,to]);expect(r.end.getTime()).toBe(key==='seven'||key==='thirty'?new Date('2026-10-14T00:00:00+08:00').getTime():now.getTime());}
 expect(statisticsRange('seven',now)!.end.getTime()-statisticsRange('seven',now)!.start.getTime()).toBe(7*86400000);
 expect(statisticsRange('thirty',now)!.end.getTime()-statisticsRange('thirty',now)!.start.getTime()).toBe(30*86400000);
});
it('handles Monday, month start, leap February and year boundaries in Shanghai',()=>{
 expect(statisticsRange('week',new Date('2026-10-12T01:00:00+08:00'))!.from).toBe('2026-10-12');
 expect(statisticsRange('month',new Date('2027-01-01T01:00:00+08:00'))!.from).toBe('2027-01-01');
 expect(statisticsRange('seven',new Date('2027-01-01T01:00:00+08:00'))!.from).toBe('2026-12-25');
 expect(statisticsRange('thirty',new Date('2024-03-01T01:00:00+08:00'))!.from).toBe('2024-01-31');
 expect(shanghaiDate(new Date('2026-10-13T23:30:00Z'))).toBe('2026-10-14');
});
it('keeps historical cycles complete and does not invent time for future cycles',()=>{
 expect(statisticsRange('cycle',now)).toBeNull();
 expect(statisticsRange('cycle',now,cycle)!.future).toBe(true);
 const r=statisticsRange('cycle',new Date('2027-02-01T00:00:00+08:00'),cycle)!;
 expect(r.to).toBe('2027-01-24');expect(r.end.toISOString()).toBe('2027-01-24T16:00:00.000Z');
 const old={...cycle,id:'old',start_date:'2026-01-01'},current={...cycle,id:'current',start_date:'2026-10-01'};
 expect(defaultReviewCycle([old,cycle,current],'2026-10-14')?.id).toBe('current');
 expect(defaultReviewCycle([old,cycle],'2026-10-14')?.id).toBe('old');
 expect(defaultReviewCycle([],'2026-10-14')).toBeUndefined();
});
it('uses the same totals and stable colors for both charts and details',()=>{
 const e={id:'time',start_at:'2026-10-13T23:30:00+08:00',end_at:'2026-10-14T01:30:00+08:00',work_kind:'work',title:'工作',category_id:'system:work',created_at:'',day_task_id:null,project_id:null,task_id:null} as TimeEntry;
 const r=statisticsRange('seven',now)!;const data={...EMPTY,timeEntries:[e]};const stats=timeDistribution(data,r.from,r.to,now);
 expect(stats.work).toBe(30);expect(stats.total).toBe(7*1440);expect(stats.projects.get('未归属工作')).toBe(30);
 const rows=chartRows(stats.life,stats.total);expect(rows.reduce((n,r)=>n+r.percent,0)).toBeCloseTo(100);
 expect(rows.find(r=>r.label==='工作')?.color).toBe(chartColor('工作'));
 expect(chartRows(stats.projects,stats.work)[0].percent).toBe(100);
 expect(timeDistribution({...EMPTY},r.from,r.to,now).life.get('未记录')).toBe(7*1440);
 expect(chartRows(new Map(),0)).toEqual([]);
 expect(chartRows(new Map([['未记录',0]]),0)[0].percent).toBe(0);
 expect(e.end_at).toBe('2026-10-14T01:30:00+08:00');
});
