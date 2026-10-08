import concurrent.futures
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('server',Path(__file__).with_name('server.py'))
s=importlib.util.module_from_spec(spec);spec.loader.exec_module(s)

class ToolsTests(unittest.TestCase):
 def setUp(self):
  self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
  self.root=Path(self.temp.name);self.b=s.Bridge(self.root)
  self.b.rpc=lambda *_: (_ for _ in ()).throw(RuntimeError('App closed'))
  self.args={'request_id':'stable-id','source_directory':str(self.root),'todos':[{'title':'测试待办','body':'具体做法','quote':'后续测试待办','priority':'P1'}]}
 def test_offline_receipt_and_replay(self):
  a=self.b.submit(self.args);b=self.b.submit(self.args)
  self.assertEqual(a,b);self.assertEqual(a['status'],'queued')
  self.assertEqual(len(list((self.root/'ai-tools/submissions').glob('*.json'))),1)
 def test_reused_request_cannot_change_payload(self):
  self.b.submit(self.args);self.args['todos'][0]['title']='不同内容'
  with self.assertRaises(ValueError):self.b.submit(self.args)
 def test_parallel_submissions_and_retries(self):
  def submit(i):return self.b.submit({**self.args,'request_id':str(i%4)})
  with concurrent.futures.ThreadPoolExecutor(max_workers=8) as pool:list(pool.map(submit,range(24)))
  self.assertEqual(len(list((self.root/'ai-tools/submissions').glob('*.json'))),4)
 def test_import_receipt_survives_archival(self):
  self.b.submit(self.args);key=s.digest('stable-id');home=self.root/'ai-tools'
  s.atomic(home/'receipts'/f'{key}.json',{'status':'imported','count':1})
  (home/'processed').mkdir();(home/'submissions'/f'{key}.json').rename(home/'processed'/f'{key}.json')
  self.assertEqual(self.b.submit(self.args)['status'],'imported')
 def test_scope_limits_and_invalid_payloads(self):
  self.assertEqual({t['name'] for t in s.tools('submit')},{'submit_todos','todo_receipt'})
  with self.assertRaises(ValueError):self.b.call('query',{'kind':'task'})
  self.args['todos'][0]['quote']=''
  with self.assertRaises(ValueError):self.b.submit(self.args)
 def test_worktree_identity(self):
  import subprocess
  repo=self.root/'repo';repo.mkdir();worktree=self.root/'worktree'
  def git(*args):subprocess.run(['git','-C',str(repo),*args],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  git('init');git('-c','user.name=Test','-c','user.email=test@example.invalid','commit','--allow-empty','-m','test');git('worktree','add','--detach',str(worktree))
  self.assertEqual(s.project_root(str(repo)),s.project_root(str(worktree)))

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
