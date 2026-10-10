import { describe, expect, it } from 'vitest';
import { filterDevTodos, type DevTodo } from './devInboxModel';
const base:DevTodo={id:'a',source_key:'repo-a',source_name:'同名项目',source_root:'/a',title:'增加搜索',body:'后续优化',status:'todo',linked_project_id:'project-1',created_at:'',updated_at:'',sources:[]};
describe('独立开发收件箱筛选',()=>{
 it('来源项目和工作台关联分别筛选，不用名称猜测',()=>{
  const rows=[base,{...base,id:'b',source_key:'repo-b',source_root:'/b',linked_project_id:'project-2'}];
  expect(filterDevTodos(rows,'','repo-a','open').map(t=>t.id)).toEqual(['a']);
  expect(filterDevTodos(rows,'','','open','project-2').map(t=>t.id)).toEqual(['b']);
 });
 it('完成事项默认隐藏，但保留在全部状态中',()=>{
  const rows=[base,{...base,id:'b',status:'done' as const}];
  expect(filterDevTodos(rows,'搜索','','open')).toHaveLength(1);
  expect(filterDevTodos(rows,'搜索','','all')).toHaveLength(2);
 });
});
