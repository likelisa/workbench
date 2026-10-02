import { useEffect, useState } from 'react';
import Select from './Select';
import { type Data } from './model';
import { CATEGORY_LABELS, FIXED_NAMES, categoryAvailable, categoryKind } from './timeCategories';
export default function TimeCategoryPicker({data,date,value,onChange,originalId,linkedId,onUseLinked,disabled=false,labelPrefix=''}:{data:Data;date:string;value:string;onChange:(id:string)=>void;originalId?:string;linkedId?:string;onUseLinked?:()=>void;disabled?:boolean;labelPrefix?:string}){
 const selected=data.categories.find(c=>c.id===value),original=data.categories.find(c=>c.id===originalId);
 const [emptyKind,setEmptyKind]=useState(''),[year,setYear]=useState(date.slice(0,4)),[query,setQuery]=useState('');
 useEffect(()=>{setYear(date.slice(0,4));},[date]);
 const kind=selected?categoryKind(selected):emptyKind;
 const years=[...new Set([date.slice(0,4),...data.annualGoals.map(g=>String(g.year)),...(selected?.goal_year?[String(selected.goal_year)]:[])])].sort().reverse();
 const items=data.categories.filter(c=>categoryKind(c)===kind&&categoryAvailable(c)&&(kind!=='goal'||String(c.goal_year)===year)).filter(c=>c.name.toLocaleLowerCase().includes(query.toLocaleLowerCase())).sort((a,b)=>kind==='fixed'?FIXED_NAMES.indexOf(a.name)-FIXED_NAMES.indexOf(b.name):a.name.localeCompare(b.name,'zh-CN'));
 return <div className="time-category-picker">
 <label><span>时间类型</span><Select aria-label={`${labelPrefix}时间类型`} value={kind} disabled={disabled} onChange={e=>{setEmptyKind(e.target.value);setQuery('');onChange('');}}><option value="">请选择时间类型</option><option value="fixed">固定类 · 日常生活</option><option value="goal">不固定类 · 年度目标</option>{original&&categoryKind(original)==='legacy'&&<option value={categoryKind(original)}>{CATEGORY_LABELS[categoryKind(original)]}（保留原分类）</option>}</Select></label>
 {kind==='goal'&&<><label><span>目标年份</span><Select aria-label={`${labelPrefix}目标年份`} value={year} disabled={disabled} onChange={e=>setYear(e.target.value)}>{years.map(y=><option value={y} key={y}>{y} 年</option>)}</Select></label><label><span>搜索年度目标</span><input aria-label={`${labelPrefix}搜索年度目标`} placeholder="搜索方向或目标…" disabled={disabled} value={query} onChange={e=>setQuery(e.target.value)}/></label></>}
 {kind&&<label><span>{kind==='fixed'?'生活类别':kind==='goal'?'年度目标':'原分类'}</span><Select aria-label={`${labelPrefix}时间分类`} value={value} displayLabel={selected?.name} disabled={disabled} onChange={e=>onChange(e.target.value)}><option value="">请选择{kind==='fixed'?'生活类别':'年度目标'}</option>{items.map(c=><option value={c.id} key={c.id}>{c.name}{!categoryAvailable(c)?'（历史分类）':''}</option>)}{original&&!items.some(c=>c.id===original.id)&&categoryKind(original)===kind&&<option value={original.id}>{original.name}（历史分类）</option>}</Select></label>}
 {kind==='goal'&&!items.some(categoryAvailable)&&<p className="muted">该年份没有匹配的年度目标，请切换年份、调整搜索，或先在 12 周计划中创建年度目标。</p>}
 {linkedId&&onUseLinked&&<button type="button" className="text-button" disabled={disabled} onClick={()=>{setEmptyKind('goal');setQuery('');const linked=data.categories.find(c=>c.id===linkedId);if(linked?.goal_year)setYear(String(linked.goal_year));onUseLinked();}}>使用关联任务的年度目标</button>}
 </div>;
}
