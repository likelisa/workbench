import { beforeEach, describe, expect, it, vi } from 'vitest';
const database=vi.hoisted(()=>({select:vi.fn(),execute:vi.fn()}));
vi.mock('@tauri-apps/plugin-sql',()=>({default:{load:async()=>database}}));
vi.mock('@tauri-apps/api/core',()=>({invoke:vi.fn(async()=>{})}));
import { updateDayTask } from './db';
const values={status:'done' as const,priority:null,date:'2026-10-07',title:'修改任务',assignment_id:null,project_id:null,task_id:null,scheduled_start_at:null,scheduled_end_at:null,scheduled_category_id:null};
beforeEach(()=>{vi.clearAllMocks();database.execute.mockResolvedValue({rowsAffected:1});});
describe('日任务编辑持久化状态',()=>{
  it.each([null,'repeat'])('普通或重复任务状态保存：%s',async recurrence=>{
    database.select.mockResolvedValue([{date:values.date,recurrence_id:recurrence}]);
    await updateDayTask('day',values);
    const [sql,params]=database.execute.mock.calls[0];
    expect(sql).toContain('status=$11');expect(params[10]).toBe('done');expect(params[8]).toBe('day');
    expect(database.execute).toHaveBeenCalledTimes(1);
    expect(sql).not.toContain('time_entries');
  });
  it('重复任务改日期时保留状态修改，并排除原日期',async()=>{
    database.select.mockResolvedValue([{date:'2026-10-06',recurrence_id:'repeat'}]);
    await updateDayTask('day',values);
    expect(database.execute.mock.calls[0][1]).toEqual(['repeat','2026-10-06']);
    const [sql,params]=database.execute.mock.calls[1];
    expect(sql).toContain('recurrence_id=NULL');expect(sql).toContain('status=$11');expect(params[10]).toBe('done');
  });
});
