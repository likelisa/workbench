import { useEffect, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import './aiTools.css';
type Review={id:string;snapshot_id:string;title:string;body:string;evidence_json:string;created_at:string};
export default function AiReviews(){
 const [items,setItems]=useState<Review[]>([]),[error,setError]=useState(''),[snapshot,setSnapshot]=useState<{id:string;data_json:string}|null>(null),[offset,setOffset]=useState(0),[next,setNext]=useState<number|null>(null);
 useEffect(()=>{let live=true;const refresh=()=>invoke<{items:Review[];next_offset:number|null}>('ai_call',{method:'query',args:{kind:'review',limit:20,offset}}).then(r=>{if(live){setItems(r.items);setNext(r.next_offset);setError('');}}).catch(e=>{if(live)setError(String(e));});void refresh();const subscription=listen('workbench-changed',()=>void refresh());return()=>{live=false;void subscription.then(f=>f());};},[offset]);
 async function show(id:string){try{const r=await invoke<{items:{id:string;data_json:string}[]}>('ai_call',{method:'query',args:{kind:'snapshot',id}});setSnapshot(r.items[0]||null);}catch(e){setError(String(e));}}
 return <section className="panel ai-reviews"><div className="section-head"><h2>AI 复盘</h2><span className="ai-badge">年度 · 十二周</span></div><p className="muted">在「工作台助手」中提出复盘要求，报告会保存在这里。每份报告保留生成时的数据依据。</p>{error&&<p role="alert">{error}</p>}{!items.length&&<p className="project-empty">暂无 AI 复盘。现有时间统计和历史复盘记录仍然保留。</p>}{items.map(r=><details className="ai-review" key={r.id}><summary><strong>{r.title}</strong><time>{new Date(r.created_at).toLocaleString('zh-CN')}</time></summary><div className="ai-review-body">{r.body}</div><small>数据快照：{r.snapshot_id.slice(0,12)} · 每次生成单独保存</small><div><button onClick={()=>void show(r.snapshot_id)}>查看统计与数据依据</button></div></details>)}<div className="record-actions">{offset>0&&<button onClick={()=>setOffset(Math.max(0,offset-20))}>上一页</button>}{next!==null&&<button onClick={()=>setOffset(next)}>下一页</button>}</div>{snapshot&&<div className="modal-backdrop"><div className="modal" role="dialog" aria-modal="true" aria-label="复盘数据依据"><div className="modal-head"><h2>生成时的数据依据</h2><button onClick={()=>setSnapshot(null)}>关闭</button></div><pre className="ai-snapshot">{JSON.stringify(JSON.parse(snapshot.data_json),null,2)}</pre></div></div>}</section>;
}

export function AiToolsStatus(){
 const [state,setState]=useState<{queued:number;failed:number;legacy_pending:number;error?:string}|null>(null),[error,setError]=useState('');
 useEffect(()=>{let live=true;const refresh=()=>invoke<typeof state>('ai_call',{method:'connection_status',args:{}}).then(r=>{if(live){setState(r);setError('');}}).catch(e=>{if(live)setError(String(e));});void refresh();const timer=setInterval(refresh,5000);return()=>{live=false;clearInterval(timer);};},[]);
 return <div className="ai-tools-status" role="status"><strong>工具提交收件箱</strong><span>{state?`等待导入 ${state.queued} · 导入失败 ${state.failed}`:'正在检查接入状态'}</span><p>回执显示「已暂存」时等待导入；「已导入」表示待办已保存。此入口不再扫描聊天或调用 AI 提取。</p>{!!state?.legacy_pending&&<p>旧链路仍保留 {state.legacy_pending} 个未处理回合，已暂停；原始记录未删除。</p>}{(error||state?.error)&&<p role="alert">{error||state?.error}</p>}</div>;
}
