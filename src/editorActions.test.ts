import { describe, expect, it, vi } from 'vitest';
import { createEditorQueue, runSavedEditorAction, type EditableRecord } from './editorActions';

const form=(title='原内容'):EditableRecord=>({kind:'assignment',editingId:'week',values:{title},initial:JSON.stringify({title:'原内容'})});
const deferred=()=>{let resolve!:()=>void;const promise=new Promise<void>(done=>{resolve=done;});return {promise,resolve};};

describe('任务编辑保存与关联操作',()=>{
  it('等待正在进行的失焦保存，连续周次操作按顺序执行',async()=>{
    const queue=createEditorQueue(),gate=deferred(),events:string[]=[];
    let current=form('已修改');
    const saving=queue.run(async()=>{events.push('开始保存');await gate.promise;current={...current,initial:JSON.stringify(current.values)};events.push('保存完成');});
    const action=()=>runSavedEditorAction(current,()=>current,vi.fn(),async()=>{events.push('添加周次');});
    const first=queue.run(action),second=queue.run(async()=>{events.push('移除周次');});
    await Promise.resolve();expect(events).toEqual(['开始保存']);
    gate.resolve();await Promise.all([saving,first,second]);
    expect(events).toEqual(['开始保存','保存完成','添加周次','移除周次']);
    expect(current.values.title).toBe('已修改');
  });
  it('保存失败时不安排周次、不切换子任务，保留父表单修改',async()=>{
    const current=form('尚未保存'),action=vi.fn(),save=vi.fn(async()=>false);
    expect(await runSavedEditorAction(current,()=>current,save,action)).toBe(false);
    expect(action).not.toHaveBeenCalled();expect(current.values.title).toBe('尚未保存');
  });
  it('保存期间继续编辑时，先补存最新内容再打开子任务',async()=>{
    let current=form('第一次修改'),saveCount=0;const action=vi.fn();
    const save=vi.fn(async()=>{
      const snapshot=JSON.stringify(current.values);
      current={...current,values:{title:++saveCount===1?'第二次修改':current.values.title},initial:snapshot};
      return true;
    });
    expect(await runSavedEditorAction(current,()=>current,save,action)).toBe(true);
    expect(save).toHaveBeenCalledTimes(2);expect(action).toHaveBeenCalledWith(expect.objectContaining({values:{title:'第二次修改'}}));
  });
  it('弹窗切换后忽略旧表单排队的操作',async()=>{
    const target=form(),other={...form(),editingId:'other'},action=vi.fn();
    expect(await runSavedEditorAction(target,()=>other,vi.fn(),action)).toBe(false);
    expect(action).not.toHaveBeenCalled();
  });
  it('操作异常不会阻塞后续保存',async()=>{
    const queue=createEditorQueue();
    await expect(queue.run(async()=>{throw Error('失败');})).rejects.toThrow('失败');
    await expect(queue.run(async()=>true)).resolves.toBe(true);
  });
});
