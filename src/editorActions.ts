export type EditableRecord = {kind:string;editingId?:string;values:Record<string,string>;initial?:string};

// Share one queue between blur saves and actions that depend on a saved parent.
export function createEditorQueue() {
  let tail:Promise<unknown>=Promise.resolve();
  return {
    run<T>(operation:()=>Promise<T>):Promise<T> {
      const result=tail.then(operation);
      tail=result.catch(()=>{});
      return result;
    }
  };
}

export async function runSavedEditorAction<T extends EditableRecord>(
  target:T,
  current:()=>T|null,
  save:()=>Promise<boolean>,
  action:(editor:T)=>Promise<unknown>|unknown
):Promise<boolean> {
  for (;;) {
    const editor=current();
    if(!editor?.editingId||editor.kind!==target.kind||editor.editingId!==target.editingId)return false;
    if(editor.initial===JSON.stringify(editor.values)) {
      await action(editor);
      return true;
    }
    if(!await save())return false;
  }
}
