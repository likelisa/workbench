import { useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { X } from 'lucide-react';
import { cycleEndDate, formatHours, type Cycle, type Data } from './model';
import { timeDistribution } from './unifiedModel';
import { chartRows, defaultReviewCycle, pieArc, shanghaiDate, statisticsRange, TIME_RANGES, type ChartRow, type TimeRange } from './timeReviewModel';
import { writeOperations } from './unifiedApi';
import { DayTimeEditor } from './MonthlyTime';
import ActionNotice from './ActionNotice';
import Select from './Select';
import './unified.css';
import './timeReview.css';

type Props={data:Data;onRefresh:()=>Promise<void>};
const GROUPS={projects:'项目',goals:'年度目标',directions:'年度方向'} as const;
type Group=keyof typeof GROUPS;
const rangeLabel=(date:Date)=>new Intl.DateTimeFormat('sv-SE',{timeZone:'Asia/Shanghai',year:'numeric',month:'2-digit',day:'2-digit',hour:'2-digit',minute:'2-digit',hour12:false}).format(date);
const detail=(row:ChartRow)=>`${row.label} · ${formatHours(row.minutes)} · ${row.percent.toFixed(1)}%`;

export default function TimeReview({data,onRefresh}:Props) {
 const [now,setNow]=useState(()=>new Date()),[range,setRange]=useState<TimeRange>('seven'),[dimension,setDimension]=useState<'life'|'work'>('life'),[group,setGroup]=useState<Group>('projects');
 const today=shanghaiDate(now);
 const [cycleId,setCycleId]=useState(()=>defaultReviewCycle(data.cycles,today)?.id||''),[historyEdit,setHistoryEdit]=useState(false);
 const [editingReview,setEditingReview]=useState(false),[timeDate,setTimeDate]=useState<string|null>(null),[notice,setNotice]=useState({id:0,message:''});
 const announce=(message:string)=>setNotice(n=>({id:n.id+1,message}));
 useEffect(()=>{const timer=setInterval(()=>setNow(new Date()),60_000);return()=>clearInterval(timer);},[]);
 useEffect(()=>{if(!data.cycles.some(c=>c.id===cycleId)){setCycleId(defaultReviewCycle(data.cycles,today)?.id||'');setHistoryEdit(false);}},[data.cycles,cycleId,today]);
 const cycle=data.cycles.find(c=>c.id===cycleId),historical=!!cycle&&cycleEndDate(cycle)<today,readonly=historical&&!historyEdit;
 const bounds=statisticsRange(range,now,cycle),stats=bounds?timeDistribution(data,bounds.from,bounds.to,now):null;
 const life=chartRows(stats?.life||new Map(),stats?.total||0),work=chartRows(stats?.[group]||new Map(),stats?.work||0),chart=dimension==='life'?life:work;
 const pastYears=[...new Set(data.cycles.filter(c=>cycleEndDate(c)<today).map(c=>cycleEndDate(c).slice(0,4)))].sort().reverse();
 const option=(c:Cycle)=><option key={c.id} value={c.id}>{c.title} · {c.start_date}—{cycleEndDate(c)}</option>;
 const cycleSelect=<Select aria-label="复盘周期" value={cycleId} onChange={e=>{setCycleId(e.target.value);setHistoryEdit(false);}}><option value="" disabled>选择周期</option>{['当前','未来'].map(label=><optgroup key={label} label={label}>{data.cycles.filter(c=>label==='当前'?c.start_date<=today&&cycleEndDate(c)>=today:c.start_date>today).map(option)}</optgroup>)}{pastYears.map(y=><optgroup key={y} label={`${y}年历史周期`}>{data.cycles.filter(c=>cycleEndDate(c)<today&&cycleEndDate(c).startsWith(y)).map(option)}</optgroup>)}</Select>;
 const groupTabs=<div className="u-tabs" aria-label="工作归属维度">{Object.entries(GROUPS).map(([key,label])=><button key={key} aria-pressed={group===key} className={group===key?'active':''} onClick={()=>setGroup(key as Group)}>{label}</button>)}</div>;
 return <div className="unified-workbench time-review">
 {notice.message&&<ActionNotice key={notice.id} message={notice.message} onDismiss={()=>setNotice(n=>({...n,message:''}))}/>}
 <div className="u-heading"><div><h1>时间与复盘</h1><p>看清时间分布，回顾投入与节奏。</p></div><div className="review-top-filters"><label className="review-range">周期{cycleSelect}</label><label className="review-range">统计范围<Select aria-label="统计范围" value={range} onChange={e=>setRange(e.target.value as TimeRange)}>{Object.entries(TIME_RANGES).map(([key,label])=><option key={key} value={key} disabled={key==='cycle'&&!cycle}>{label}</option>)}</Select></label></div></div>
 <section className="u-section review-charts"><header><div><h2>{dimension==='life'?'生活时间分布':`工作时间归属 · ${GROUPS[group]}`}</h2><p className="review-period">{bounds?bounds.future?'所选周期尚未开始，暂无可统计时间。':`${rangeLabel(bounds.start)} — ${rangeLabel(bounds.end)}`:'暂无周期，请先在12周计划中创建。'}</p><p className="u-muted">{range==='seven'||range==='thirty'?'完整自然日，不含今天。':'统计截止不晚于当前时刻。'}{dimension==='life'?'包含未记录时间。':'占比以实际工作时间为分母。'}</p></div><div className="review-chart-controls"><div className="u-tabs review-dimensions" aria-label="图表维度"><button aria-pressed={dimension==='life'} className={dimension==='life'?'active':''} onClick={()=>setDimension('life')}>生活时间</button><button aria-pressed={dimension==='work'} className={dimension==='work'?'active':''} onClick={()=>setDimension('work')}>工作归属</button></div>{dimension==='work'&&groupTabs}</div></header>
 {range==='cycle'&&<p className="review-cycle-context">统计周期：{cycle?.title} · {cycle?.start_date} — {cycle&&cycleEndDate(cycle)}</p>}
 <TimeCharts key={`${range}-${dimension}-${group}-${cycleId}`} rows={chart} empty={dimension==='work'?'这段时间暂无工作记录':'这段时间暂无可统计时间'}/>
 </section>
 <section className="u-section review-details"><div className="u-time-columns"><div><header><h2>完整生活时间分布</h2><span>占统计区间</span></header><Distribution rows={life}/></div><div><header><h2>工作时间归属</h2><span>占工作时间</span></header>{groupTabs}<Distribution rows={work}/></div></div></section>
 <section className="u-section review-cycle"><header><div><h2>周期复盘记录</h2><p className="u-muted">复盘保存到所选周期，与上方日期范围独立。</p></div></header>
 {cycle?<><div className="review-cycle-heading"><strong>{cycle.title}</strong><span>{cycle.start_date} — {cycleEndDate(cycle)}</span>{historical&&<button onClick={()=>setHistoryEdit(!historyEdit)}>{historyEdit?'结束历史编辑':'编辑历史周期'}</button>}</div>{readonly&&<p className="u-history-banner">历史周期 · 浏览模式</p>}<p className="u-review">{cycle.review||'暂无复盘记录。'}</p><div className="u-actions"><button disabled={readonly} onClick={()=>setEditingReview(true)}>编辑复盘</button><button disabled={range==='cycle'&&readonly} onClick={()=>setTimeDate(bounds&&!bounds.future?shanghaiDate(new Date(bounds.end.getTime()-1)):today)}>补录实际时间</button></div></>:<><p className="u-empty">暂无周期，仍可查看时间统计和补录实际时间。</p><button onClick={()=>setTimeDate(today)}>补录实际时间</button></>}
 </section>
 {editingReview&&cycle&&<ReviewEditor cycle={cycle} onClose={()=>setEditingReview(false)} onRefresh={onRefresh} onNotice={announce}/>}
 {timeDate&&<DayTimeEditor key={timeDate} date={timeDate} data={data} initialKind="time" actualOnly onClose={()=>setTimeDate(null)} onDate={setTimeDate} onRefresh={onRefresh} onNotice={announce}/>}
 </div>;
}

function Distribution({rows}:{rows:ChartRow[]}) {
 return rows.length?<div className="review-numbers">{rows.map(row=><div className="u-stat-row" key={row.label}><span><b className="review-swatch" style={{background:row.color}}/>{row.label}</span><strong>{formatHours(row.minutes)}</strong><small>{row.percent.toFixed(1)}%</small><i style={{width:`${Math.min(100,row.percent)}%`,background:row.color}}/></div>)}</div>:<p className="u-empty">暂无记录。</p>;
}

export function TimeCharts({rows,empty}:{rows:ChartRow[];empty:string}) {
 const [active,setActive]=useState<string|null>(null),positive=rows.filter(r=>r.minutes>0),total=positive.reduce((n,r)=>n+r.minutes,0),max=Math.max(1,...positive.map(r=>r.minutes));let cursor=0;
 const selected=rows.find(r=>r.label===active),interact=(row:ChartRow)=>({tabIndex:0,onFocus:()=>setActive(row.label),onBlur:()=>setActive(null),onMouseEnter:()=>setActive(row.label),onMouseLeave:()=>setActive(null),'aria-label':detail(row)});
 return <><div className="review-chart-grid"><figure><figcaption>时间占比</figcaption>{positive.length?<><svg viewBox="0 0 300 300" className="review-pie" role="group" aria-label="时间分布饼图">{positive.map(row=>{const start=cursor,fraction=row.minutes/total;cursor+=fraction;const props={...interact(row),fill:row.color,stroke:'#fff',strokeWidth:2,opacity:active&&active!==row.label?0.5:1};return positive.length===1?<circle key={row.label} cx={150} cy={150} r={124} {...props}><title>{detail(row)}</title></circle>:<path key={row.label} d={pieArc(start,fraction)} {...props}><title>{detail(row)}</title></path>;})}</svg><div className="review-legend">{positive.map(row=><button key={row.label} {...interact(row)}><b className="review-swatch" style={{background:row.color}}/>{row.label}<small>{row.percent.toFixed(1)}%</small></button>)}</div></>:<p className="review-chart-empty">{empty}</p>}</figure>
 <figure><figcaption>类别时长 <small>小时</small></figcaption>{positive.length?<div className="review-bars">{positive.map(row=><div key={row.label} className={`review-bar ${active===row.label?'is-active':''}`} {...interact(row)}><div><span>{row.label}</span><strong>{formatHours(row.minutes)}</strong></div><svg viewBox="0 0 400 14" preserveAspectRatio="none" aria-hidden="true"><rect width={400} height={14} rx={3} fill="#eef3eb"/><rect width={400*row.minutes/max} height={14} rx={3} fill={row.color}/></svg></div>)}</div>:<p className="review-chart-empty">{empty}</p>}</figure></div><p className="review-chart-hint" role="status">{selected?detail(selected):'悬停或用 Tab 聚焦图形，查看时长与占比。'}</p></>;
}

function ReviewEditor({cycle,onClose,onRefresh,onNotice}:{cycle:Cycle;onClose:()=>void;onRefresh:()=>Promise<void>;onNotice:(s:string)=>void}) {
 const [draft,setDraft]=useState(''),[original,setOriginal]=useState(''),[version,setVersion]=useState(''),[error,setError]=useState(''),[saving,setSaving]=useState(false);const panel=useRef<HTMLFormElement>(null);
 useEffect(()=>{let alive=true;void invoke<{items:(Cycle&{version:string})[]}>('ai_call',{method:'query',args:{kind:'cycle',id:cycle.id}}).then(r=>{if(alive){const c=r.items[0];if(!c)throw Error('周期已不存在，请刷新');setDraft(c.review||'');setOriginal(c.review||'');setVersion(c.version);}}).catch(e=>{if(alive)setError(String(e));});const old=document.body.style.overflow;document.body.style.overflow='hidden';panel.current?.querySelector<HTMLElement>('button')?.focus();return()=>{alive=false;document.body.style.overflow=old;};},[cycle.id]);
 const close=()=>{if(!saving&&(draft===original||window.confirm('复盘尚未保存，放弃修改并关闭？')))onClose();};
 const save=async()=>{if(!version||saving)return;setSaving(true);setError('');try{const r=await writeOperations([{kind:'cycle',action:'update',id:cycle.id,expected_version:version,values:{review:draft}}]);await onRefresh();onNotice(r.backup_error?`复盘已保存；备份失败：${r.backup_error}`:'周期复盘已保存');onClose();}catch(e){setError(String(e));}finally{setSaving(false);}};
 return <div className="u-modal-backdrop" onClick={e=>{if(e.target===e.currentTarget)close();}} onKeyDown={e=>{if(e.key==='Escape')close();if(e.key==='Tab'){const nodes=[...panel.current!.querySelectorAll<HTMLElement>('button:not(:disabled),textarea:not(:disabled)')];if(e.shiftKey&&document.activeElement===nodes[0]){e.preventDefault();nodes.at(-1)?.focus();}else if(!e.shiftKey&&document.activeElement===nodes.at(-1)){e.preventDefault();nodes[0]?.focus();}}}}><form className="u-modal review-editor" role="dialog" aria-modal="true" aria-label="编辑周期复盘" ref={panel} onSubmit={e=>{e.preventDefault();void save();}}><header><div><h2>编辑复盘</h2><p>{cycle.title}</p></div><button type="button" aria-label="关闭复盘编辑" disabled={saving} onClick={close}><X/></button></header><textarea aria-label="周期复盘内容" rows={10} disabled={!version||saving} value={draft} onChange={e=>setDraft(e.target.value)}/>{error&&<p role="alert" className="u-error">{error}</p>}<footer><button type="button" disabled={saving} onClick={close}>取消</button><button className="primary" disabled={!version||saving||draft===original}>{saving?'正在保存…':'保存复盘'}</button></footer></form></div>;
}
