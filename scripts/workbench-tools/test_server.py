import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('server',Path(__file__).with_name('server.py'))
s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)

class ToolsTests(unittest.TestCase):
 def test_weixin_scope_is_forced_and_has_no_model_confirmation_field(self):
  import os
  from unittest.mock import patch
  with patch.dict(os.environ,{'WORKBENCH_WEIXIN_ROOT':'/tmp/trusted-bridge'}):
   self.assertEqual(s.Bridge(scope='manage').scope,'weixin')
  confirm=next(t for t in s.tools('weixin') if t['name']=='confirm_draft')
  self.assertNotIn('confirmation_text',confirm['inputSchema']['properties'])
  self.assertIn('confirmation_text',next(t for t in s.tools('manage') if t['name']=='confirm_draft')['inputSchema']['properties'])
 def test_removed_tools_are_rejected_before_connection(self):
  with tempfile.TemporaryDirectory() as d:
   bridge=s.Bridge(Path(d))
   for name in ['submit_todos','todo_receipt','sync_submissions']:
    with self.assertRaisesRegex(ValueError,'未知工具'):bridge.call(name,{})
   self.assertFalse((Path(d)/'ai-tools').exists())
 def test_app_closed_management_reports_unavailable(self):
  with tempfile.TemporaryDirectory(dir='/tmp') as d:
   root=Path(d);s.atomic(root/'ai-tools/capabilities.json',{'manage':'test'})
   with self.assertRaisesRegex(RuntimeError,'工作台未打开'):s.Bridge(root).call('query',{'kind':'cycle'})
 def test_stdio_listing_and_removed_call(self):
  import subprocess,sys
  requests=[{'jsonrpc':'2.0','id':1,'method':'tools/list'},{'jsonrpc':'2.0','id':2,'method':'tools/call','params':{'name':'submit_todos','arguments':{}}}]
  result=subprocess.run([sys.executable,str(Path(__file__).with_name('server.py')),'--scope','manage'],input=''.join(json.dumps(r)+'\n' for r in requests),text=True,capture_output=True,check=True)
  listing,call=[json.loads(l)['result'] for l in result.stdout.splitlines()]
  self.assertEqual({t['name'] for t in listing['tools']},{'query','execute_operation','create_draft','revise_draft','query_drafts','confirm_draft','cancel_draft','set_task_status','statistics','review_snapshot','save_review','operation_receipt'})
  self.assertTrue(call['isError'])

class SocketTests(unittest.TestCase):
 def test_large_fragmented_unicode_response(self):
  import socket, threading
  with tempfile.TemporaryDirectory(dir='/tmp') as d:
   root=Path(d);s.atomic(root/'ai-tools/capabilities.json',{'manage':'test'})
   expected={'body':'中文复盘内容' * 12000}
   payload=(json.dumps({'result':expected},ensure_ascii=False)+'\n').encode()
   with socket.socket(socket.AF_UNIX) as listener:
    listener.bind(str(root/'ai.sock'));listener.listen()
    def serve():
     conn,_=listener.accept()
     with conn:
      conn.recv(4096)
      for i in range(0,len(payload),997):conn.sendall(payload[i:i+997])
    thread=threading.Thread(target=serve);thread.start()
    self.assertEqual(s.Bridge(root,'manage').rpc('review_snapshot',{}),expected)
    thread.join()

if __name__=='__main__':unittest.main()
