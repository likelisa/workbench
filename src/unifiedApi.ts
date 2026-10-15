import { invoke } from '@tauri-apps/api/core';
export type Operation={kind:string;action:'create'|'update'|'delete';id?:string;ref?:string;expected_version?:string;values?:Record<string,unknown>};
export async function readItemRecord(id:string){
 const q=await invoke<{items:(Record<string,unknown>&{version:string})[]}>('ai_call',{method:'query',args:{kind:'item',id}});
 if(!q.items[0])throw Error('记录已删除，请刷新');return q.items[0];
}
export async function writeOperations(operations:Operation[]){
 const hydrated=[];
 for(const op of operations){if(op.action!=='create'&&!op.expected_version){const q=await invoke<{items:{version:string}[]}>('ai_call',{method:'query',args:{kind:op.kind,id:op.id}});if(!q.items[0])throw Error('记录已删除，请刷新');hydrated.push({...op,expected_version:q.items[0].version});}else hydrated.push(op);}
 const request_id=crypto.randomUUID();
 const args={request_id,intent:'explicit',operations:hydrated};
 type Receipt={status:string;records:{id:string;version?:string}[];backup_error?:string};
 let result:Receipt;
 try {result=await invoke<Receipt>('ai_call',{method:'execute_operation',args});}
 catch(error){try{result=await invoke<Receipt>('ai_call',{method:'operation_receipt',args:{request_id}});}catch{throw error;}}
 if(result.status!=='saved')throw Error('保存未完成');
 return result;
}
