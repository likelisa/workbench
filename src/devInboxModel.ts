import { useCallback, useEffect, useRef, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
export type DevTodoStatus='todo'|'doing'|'done'|'dismissed';
export type DevTodo={id:string;source_key:string;source_name:string;source_root:string;title:string;body:string;status:DevTodoStatus;linked_project_id:string|null;created_at:string;updated_at:string;sources:{session_id:string;quote:string;captured_at:string}[]};
export type InboxSnapshot={items:DevTodo[];pending:number;capture:{last_capture?:string}|null;worker:{last_success?:string;last_error?:string}|null;errors:string[]};
export const DEV_STATUS:Record<DevTodoStatus,string>={todo:'待处理',doing:'进行中',done:'已完成',dismissed:'已放弃'};
export const EMPTY_INBOX:InboxSnapshot={items:[],pending:0,capture:null,worker:null,errors:[]};
export function filterDevTodos(items:DevTodo[],query:string,source:string,status:string,project?:string){
 const q=query.trim().toLocaleLowerCase();
 return items.filter(t=>(!project||t.linked_project_id===project)&&(!source||t.source_key===source)&&(status==='all'||(status==='open'?['todo','doing'].includes(t.status):t.status===status))&&(!q||`${t.title}\n${t.body}\n${t.source_name}`.toLocaleLowerCase().includes(q)));
}
export function useDevInbox(enabled:boolean){
 const [snapshot,setSnapshot]=useState(EMPTY_INBOX),[error,setError]=useState('');
 const alive=useRef(false),pending=useRef<Promise<void>|null>(null);
 const refresh=useCallback(()=>{
  if(pending.current)return pending.current;
  const job=invoke<InboxSnapshot>('dev_inbox_read',{projectId:null}).then(value=>{if(alive.current){setSnapshot(value);setError('');}}).catch(e=>{if(alive.current)setError(String(e));}).finally(()=>{pending.current=null;});
  pending.current=job;return job;
 },[]);
 useEffect(()=>{alive.current=true;if(!enabled)return()=>{alive.current=false;};void refresh();const timer=window.setInterval(()=>void refresh(),10000);const focus=()=>void refresh();window.addEventListener('focus',focus);return()=>{alive.current=false;window.clearInterval(timer);window.removeEventListener('focus',focus);};},[enabled,refresh]);
 return {snapshot,error,refresh};
}
