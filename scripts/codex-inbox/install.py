#!/usr/bin/env python3
"""Install collector without replacing existing Codex hooks or notify settings."""
import datetime
import json
import os
from pathlib import Path
import plistlib
import shlex
import shutil
import subprocess
import sys

home = Path.home()
root = home / 'Library/Application Support/com.personal.workbench/codex-inbox'
root.mkdir(parents=True, exist_ok=True)
os.chmod(root, 0o700)
collector = root / 'collector.py'
shutil.copy2(Path(__file__).with_name('collector.py'), collector)
python = sys.executable
codex_home = Path(os.environ.get('CODEX_HOME', home / '.codex'))
config = codex_home / 'hooks.json'
config.parent.mkdir(parents=True, exist_ok=True)
old = json.loads(config.read_text()) if config.exists() else {}
command = shlex.join([python, str(collector), 'capture'])
hooks = old.setdefault('hooks', {}).setdefault('Stop', [])
if not any(h.get('command') == command for group in hooks for h in group.get('hooks', [])):
    if config.exists():
        shutil.copy2(config, root / ('hooks-before-' + datetime.datetime.now().strftime('%Y%m%d%H%M%S') + '.json'))
    hooks.append({'hooks': [{'type': 'command', 'command': command, 'timeout': 10}]})
    config.write_text(json.dumps(old, ensure_ascii=False, indent=2) + '\n')
label = 'com.personal.workbench.codex-inbox'
plist = home / 'Library/LaunchAgents' / (label + '.plist')
plist.parent.mkdir(parents=True, exist_ok=True)
plist.write_bytes(plistlib.dumps({'Label': label, 'ProgramArguments': [python, str(collector), 'worker'],
    'StartInterval': 30, 'RunAtLoad': True, 'WorkingDirectory': str(root),
    'StandardOutPath': str(root / 'worker.log'), 'StandardErrorPath': str(root / 'worker-error.log'),
    'EnvironmentVariables': {'PATH': os.environ.get('PATH', '/usr/bin:/bin')}}))
subprocess.run(['launchctl', 'bootout', 'gui/' + str(os.getuid()), str(plist)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
subprocess.run(['launchctl', 'bootstrap', 'gui/' + str(os.getuid()), str(plist)], check=True)
print('Collector installed. Review and trust the new Stop hook via Codex /hooks before expecting capture.')
print('Existing notify configuration was not modified.')
print('Desktop fallback tails newly completed local Codex turns; old history is excluded.')
