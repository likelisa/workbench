import { addDays, cycleEndDate, mondayOf, type Cycle } from './model';

export const TIME_RANGES = { today:'今日', seven:'最近7天', week:'本周', thirty:'最近30天', month:'本月', cycle:'本轮' } as const;
export type TimeRange = keyof typeof TIME_RANGES;
export function shanghaiDate(now:Date) { return new Intl.DateTimeFormat('sv-SE',{timeZone:'Asia/Shanghai',year:'numeric',month:'2-digit',day:'2-digit'}).format(now); }
export function statisticsRange(range:TimeRange,now:Date,cycle?:Cycle) {
 const today=shanghaiDate(now);
 if(range==='cycle'&&!cycle)return null;
 const from=range==='cycle'?cycle!.start_date:range==='seven'?addDays(today,-7):range==='thirty'?addDays(today,-30):range==='week'?mondayOf(today):range==='month'?`${today.slice(0,7)}-01`:today;
 const to=range==='cycle'?cycleEndDate(cycle!):range==='seven'||range==='thirty'?addDays(today,-1):today;
 const start=new Date(`${from}T00:00:00+08:00`),end=new Date(Math.min(new Date(`${addDays(to,1)}T00:00:00+08:00`).getTime(),now.getTime()));
 return {from,to,start,end,future:end<start};
}
export function defaultReviewCycle(cycles:Cycle[],today:string) {
 return cycles.find(c=>c.start_date<=today&&cycleEndDate(c)>=today)||[...cycles].filter(c=>cycleEndDate(c)<today).sort((a,b)=>cycleEndDate(b).localeCompare(cycleEndDate(a)))[0]||[...cycles].sort((a,b)=>a.start_date.localeCompare(b.start_date))[0];
}
const COLORS=['#2f6b21','#67D649','#D2FA78','#52D11F','#446c51','#93b982','#697b43','#b6cf9a','#709d55','#3e8a57','#afc877','#587348'];
const LIFE_COLORS:Record<string,string>={
 '睡觉':'#000000','杂事':'#FACC15','恋爱':'#F472B6',
 '工作':'#267DFF','学习':'#FF0D0D','吃饭':'#9CA3AF',
 '健身':'#67D649','社交':'#A855F7','娱乐':'#FF5D00',
};
export function chartColor(label:string) {
 if(label==='未记录')return '#cbd3ce';
 if(Object.hasOwn(LIFE_COLORS,label))return LIFE_COLORS[label];
 const hash=[...label].reduce((n,c)=>(n*31+c.charCodeAt(0))>>>0,0);
 return COLORS[hash%COLORS.length];
}
export function chartRows(values:Map<string,number>,denominator:number) {
 return [...values].sort((a,b)=>b[1]-a[1]||a[0].localeCompare(b[0])).map(([label,minutes])=>({label,minutes,percent:denominator>0?minutes/denominator*100:0,color:chartColor(label)}));
}
export type ChartRow=ReturnType<typeof chartRows>[number];
export function pieArc(start:number,fraction:number) {
 const point=(n:number)=>[150+124*Math.cos(n*2*Math.PI-Math.PI/2),150+124*Math.sin(n*2*Math.PI-Math.PI/2)].join(',');
 return `M150,150 L${point(start)} A124,124 0 ${fraction>.5?1:0} 1 ${point(start+fraction)} Z`;
}
