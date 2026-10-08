import { useEffect, useRef, useState } from 'react';
import { CheckCircle2, CircleAlert, Info, LoaderCircle, X } from 'lucide-react';

export default function ActionNotice({message,onDismiss}:{message:string;onDismiss:()=>void}){
 const [leaving,setLeaving]=useState(false),dismiss=useRef(onDismiss);
 dismiss.current=onDismiss;
 const pending=message.startsWith('正在'),failed=/失败|错误/.test(message),success=message.startsWith('已')&&!failed;
 useEffect(()=>{
  if(pending)return;
  const fade=setTimeout(()=>setLeaving(true),failed?6500:3000);
  const remove=setTimeout(()=>dismiss.current(),failed?6800:3300);
  return()=>{clearTimeout(fade);clearTimeout(remove);};
 },[pending,failed]);
 const Icon=pending?LoaderCircle:failed?CircleAlert:success?CheckCircle2:Info;
 return <div className={`action-notice ${failed?'error':''} ${leaving?'leaving':''}`} role={failed?'alert':'status'} aria-live={failed?'assertive':'polite'}>
  <Icon className={pending?'notice-spinner':''} size={23}/><strong>{message}</strong><button type="button" aria-label="关闭提示" onClick={onDismiss}><X size={18}/></button>
 </div>;
}
