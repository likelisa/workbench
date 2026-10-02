import { dayTaskTaskId, type Category, type CategoryKind, type Data } from './model';
export const FIXED_NAMES=['睡觉','吃饭','娱乐','健身','学习','社交','恋爱','杂事'];
export const CATEGORY_LABELS:Record<CategoryKind,string>={fixed:'固定类',goal:'不固定类',legacy:'历史分类'};
export const categoryKind=(category:Category):CategoryKind=>category.kind||'legacy';
export const categoryAvailable=(category:Category)=>category.active===1&&['fixed','goal'].includes(categoryKind(category));
export function validCategory(data:Data,id:string,originalId?:string){const item=data.categories.find(c=>c.id===id);return Boolean(item&&(categoryAvailable(item)||item.id===originalId));}
export function linkedGoalCategory(data:Data,kind:'entry'|'dayTask',values:Record<string,string>){
 const day=data.dayTasks.find(t=>t.id===values.day_task_id);
 const taskId=kind==='entry'?(day?dayTaskTaskId(data,day):null):data.assignments.find(w=>w.id===values.assignment_id)?.task_id;
 const goalId=data.tasks.find(t=>t.id===taskId)?.annual_goal_id;
 return data.categories.find(c=>c.annual_goal_id===goalId&&categoryKind(c)==='goal'&&categoryAvailable(c));
}
export function applyLinkedCategory(data:Data,kind:'entry'|'dayTask',values:Record<string,string>,force=false){
 if(!force&&values.__category_manual==='1')return values;
 if(kind==='dayTask'&&!values.start&&!values.end)return values;
 const linked=linkedGoalCategory(data,kind,values);
 return linked?{...values,category_id:linked.id}:values;
}
