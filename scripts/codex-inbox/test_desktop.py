import json
from pathlib import Path
import sqlite3
import tempfile
import unittest
from unittest.mock import patch
from test_collector import c


class DesktopTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.home = Path(self.temp.name) / 'codex'
        self.root = Path(self.temp.name) / 'inbox'
        self.home.mkdir(); self.root.mkdir()
        self.path = self.home / 'rollout.jsonl'
        self.path.write_text('')
        with sqlite3.connect(self.home / 'state_5.sqlite') as db:
            db.execute('CREATE TABLE threads(id,cwd,rollout_path,updated_at,source)')
            db.execute('INSERT INTO threads VALUES(?,?,?,?,?)', ('desktop-chat', self.temp.name, str(self.path), 200, 'vscode'))
        c.write_json(self.root / 'desktop-cursor.json', {'version': 1, 'since': 100, 'files': {}})

    def line(self, turn='new', timestamp=200, text='后续增加搜索'):
        return json.dumps({'timestamp': c.dt.datetime.fromtimestamp(timestamp, c.dt.timezone.utc).isoformat(),
                           'type': 'event_msg', 'payload': {'type': 'task_complete', 'turn_id': turn, 'last_agent_message': text}}) + '\n'

    def events(self):
        return [json.loads(p.read_text()) for p in (self.root / 'pending').glob('*.json')]

    def test_old_turn_skipped_new_completion_captured_and_hook_deduplicated(self):
        self.path.write_text(self.line('old', 99) + self.line())
        c.poll_desktop(self.root, self.home)
        c.poll_desktop(self.root, self.home)
        c.recover_turn(self.root, self.home, 'desktop-chat', 'new')
        self.assertEqual([e['turn_id'] for e in self.events()], ['new'])
        self.assertEqual(self.events()[0]['cwd'], self.temp.name)

    def test_partial_write_and_in_progress_reply_wait_until_completed(self):
        partial = self.line()
        self.path.write_text(partial[:30])
        c.poll_desktop(self.root, self.home)
        self.assertEqual(self.events(), [])
        with self.path.open('a') as f: f.write(partial[30:])
        c.poll_desktop(self.root, self.home)
        self.assertEqual(len(self.events()), 1)
        with self.path.open('a') as f:
            f.write(json.dumps({'timestamp': '1970-01-01T00:04:00Z', 'type': 'response_item', 'payload': {'type': 'message', 'role': 'assistant', 'text': '未完成'}}) + '\n')
        c.poll_desktop(self.root, self.home)
        self.assertEqual(len(self.events()), 1)

    def test_capture_failure_does_not_advance_cursor(self):
        self.path.write_text(self.line())
        with patch.object(c, 'capture', side_effect=OSError('disk full')):
            with self.assertRaises(OSError): c.poll_desktop(self.root, self.home)
        c.poll_desktop(self.root, self.home)
        self.assertEqual(len(self.events()), 1)

    def test_extractor_directory_excluded(self):
        self.path.write_text(self.line())
        with sqlite3.connect(self.home / 'state_5.sqlite') as db:
            db.execute('UPDATE threads SET cwd=?', (str(self.root),))
        c.poll_desktop(self.root, self.home)
        self.assertEqual(self.events(), [])

    def test_file_rotation_replays_safely(self):
        self.path.write_text(self.line())
        c.poll_desktop(self.root, self.home)
        self.path.rename(self.home / 'old.jsonl')
        self.path.write_text(self.line() + self.line('next', 201))
        c.poll_desktop(self.root, self.home)
        self.assertEqual({e['turn_id'] for e in self.events()}, {'new', 'next'})

    def test_unknown_format_reports_error_and_keeps_cursor(self):
        record=json.loads(self.line()); del record['payload']['turn_id']
        self.path.write_text(json.dumps(record) + '\n')
        c.process(self.root, codex_home=self.home)
        status=json.loads((self.root / 'worker-status.json').read_text())
        self.assertIn('format changed', status['last_error'])
        self.assertEqual(json.loads((self.root / 'desktop-cursor.json').read_text())['files'], {})


if __name__ == '__main__': unittest.main()
