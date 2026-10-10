import importlib.util
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest

spec=importlib.util.spec_from_file_location('collector',Path(__file__).with_name('collector.py'))
c=importlib.util.module_from_spec(spec);spec.loader.exec_module(c)
class CollectorTests(unittest.TestCase):
 def test_capture_replay_and_import(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);payload={'hook_event_name':'Stop','session_id':'s','turn_id':'t','cwd':temp,'last_assistant_message':'以后加搜索'}
   c.capture(root,payload);c.capture(root,payload)
   self.assertEqual(len(list((root/'pending').glob('*.json'))),1)
   c.process(root,lambda *_:[{'title':'搜索','body':'增加搜索','quote':'加搜索'}])
   c.capture(root,payload)
   self.assertEqual(len(list((root/'ready').glob('*.json'))),1)
   self.assertEqual(len(list((root/'pending').glob('*.json'))),0)
 def test_concurrent_capture_keeps_every_event(self):
  from concurrent.futures import ThreadPoolExecutor
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp)
   def capture(i):c.capture(root,{'hook_event_name':'Stop','session_id':'s','turn_id':str(i),'cwd':temp,'last_assistant_message':'建议增加搜索'})
   with ThreadPoolExecutor(max_workers=4) as pool:list(pool.map(capture,range(12)))
   self.assertEqual(len(list((root/'pending').glob('*.json'))),12)
 def test_failure_preserves_event(self):
  with tempfile.TemporaryDirectory() as temp:
   root=Path(temp);c.capture(root,{'hook_event_name':'Stop','session_id':'s','turn_id':'t','cwd':temp,'last_assistant_message':'以后搜索'})
   def fail(*_):raise RuntimeError('offline')
   c.process(root,fail)
   event=json.loads(next((root/'pending').glob('*.json')).read_text())
   self.assertEqual(event['attempts'],1);self.assertEqual(event['error'],'offline');self.assertGreater(event['next_attempt'],0)
 def test_validation_requires_real_evidence(self):
  with self.assertRaises(ValueError):c.validate({'todos':[{'title':'搜索','body':'','quote':'不存在'}]},'增加搜索')
  self.assertEqual(c.validate({'todos':[]},'已完成'),[])
 def test_worktrees_share_identity(self):
  with tempfile.TemporaryDirectory() as temp:
   repo=Path(temp)/'main';worktree=Path(temp)/'branch';repo.mkdir()
   def git(*args):subprocess.run(['git','-C',str(repo),*args],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
   git('init');git('-c','user.name=Test','-c','user.email=test@example.invalid','commit','--allow-empty','-m','test');git('worktree','add','--detach',str(worktree))
   self.assertEqual(c.source(str(repo)),c.source(str(worktree)))
 def test_extractor_is_not_recaptured(self):
  with tempfile.TemporaryDirectory() as temp:
   os.environ['WORKBENCH_INBOX_EXTRACTOR']='1'
   try:c.capture(Path(temp),{'hook_event_name':'Stop','session_id':'s','turn_id':'t','last_assistant_message':'建议'})
   finally:os.environ.pop('WORKBENCH_INBOX_EXTRACTOR')
   self.assertFalse((Path(temp)/'pending').exists())
if __name__=='__main__':unittest.main()
