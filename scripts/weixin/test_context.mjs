import { test } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { authorizeWorkbenchSender, revokeWorkbenchSender, withWorkbenchContext } from './workbench-context.mjs';

test('one sender, serial contexts and invalidation after completion', async () => {
  const dir=fs.mkdtempSync(path.join(os.tmpdir(),'workbench-context-test-'));
  process.env.WORKBENCH_WEIXIN_ROOT=dir;
  fs.writeFileSync(path.join(dir,'bridge-config.json'),JSON.stringify({allowedSenderIds:[],authorizedAccountId:null}));
  const service={options:{config:{workbenchAccountId:'account'}}};
  const message=id=>({id,senderId:'owner',text:'确认'});
  try {
    await assert.rejects(withWorkbenchContext(service,message('m0'),async()=>{}),/授权/);
    authorizeWorkbenchSender('account','owner');
    assert.throws(()=>authorizeWorkbenchSender('another','other'),/仅授权/);
    const seen=[];
    await Promise.all(['m1','m2'].map(id=>withWorkbenchContext(service,message(id),async()=>{
      const get=()=>JSON.parse(fs.readFileSync(path.join(dir,'context.json'))).messageId;
      assert.equal(get(),id);
      await new Promise(resolve=>setTimeout(resolve,10));
      assert.equal(get(),id);seen.push(id);
    })));
    assert.deepEqual(seen,['m1','m2']);
    assert.equal(JSON.parse(fs.readFileSync(path.join(dir,'context.json'))).receivedAt,0);
    revokeWorkbenchSender('account','owner');
    await assert.rejects(withWorkbenchContext(service,message('m3'),async()=>{}),/授权/);
  } finally {fs.rmSync(dir,{recursive:true,force:true});delete process.env.WORKBENCH_WEIXIN_ROOT;}
});
