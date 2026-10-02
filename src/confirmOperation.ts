import { ask } from '@tauri-apps/plugin-dialog';
// WKWebView's browser confirm is not a reliable desktop confirmation surface.
export const confirmOperation=(message:string)=>ask(message,{title:'请确认操作',kind:'warning',okLabel:'确认',cancelLabel:'取消'});
