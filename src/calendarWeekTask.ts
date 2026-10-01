import { addDays, calendarRange, cycleWeek, type Cycle, type Data } from './model';

// Calendar weeks are Monday–Sunday; a cycle may start on any weekday.
export function calendarWeekNumber(cycle:Cycle,date:string):number|null {
  const [from,to]=calendarRange(date,'week'),end=addDays(cycle.start_date,83);
  if(cycle.start_date>=to||end<from)return null;
  const anchor=date<cycle.start_date?cycle.start_date:date>end?end:date;
  return cycleWeek(cycle,anchor);
}
export function calendarTaskPlacement(data:Data,taskId:string,date:string){
  const task=data.tasks.find(item=>item.id===taskId),cycle=data.cycles.find(item=>item.id===task?.cycle_id);
  if(!task||!cycle)return null;
  const week=calendarWeekNumber(cycle,date);
  return week===null?null:{task,cycle,week};
}
export function calendarTaskChoices(data:Data,date:string,directionId='',annualId='') {
  return data.tasks.filter(task=>{
    const goal=data.annualGoals.find(item=>item.id===task.annual_goal_id);
    return (!annualId||task.annual_goal_id===annualId)&&(!directionId||goal?.direction_id===directionId)&&!!calendarTaskPlacement(data,task.id,date);
  });
}
