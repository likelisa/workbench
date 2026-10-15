#!/usr/bin/env python3
"""Install the project-scoped workbench management MCP."""
import datetime
import json
import os
from pathlib import Path
import secrets
import shutil
import sqlite3
import subprocess
import sys
from server import ROOT, atomic


def section(path, name, content):
    start, end = '# BEGIN ' + name, '# END ' + name
    prior = path.read_text() if path.exists() else ''
    if start in prior:
        before, rest = prior.split(start, 1); _, after = rest.split(end, 1)
        prior = before + after
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(prior.rstrip() + '\n\n' + start + '\n' + content.strip() + '\n' + end + '\n')


def main():
    home=Path.home();codex=home/'.codex';folder=ROOT/'ai-tools';folder.mkdir(parents=True,exist_ok=True);os.chmod(folder,0o700)
    backup=ROOT/'update-backups'/('mcp-'+datetime.datetime.now().strftime('%Y%m%d-%H%M%S'));backup.mkdir(parents=True)
    for name in ['config.toml','AGENTS.md','hooks.json']:
        f=codex/name
        if f.exists(): shutil.copy2(f,backup/name)
    db=ROOT/'workbench.db'
    if db.exists():
        with sqlite3.connect(db) as source, sqlite3.connect(backup/'workbench.db') as dest:source.backup(dest)
    shutil.copy2(Path(__file__).with_name('server.py'),folder/'server.py')
    credentials=json.loads((folder/'capabilities.json').read_text()) if (folder/'capabilities.json').exists() else {}
    for scope in ('manage','weixin'): credentials.setdefault(scope,secrets.token_urlsafe(32))
    atomic(folder/'capabilities.json',credentials)
    os.chmod(folder/'capabilities.json',0o600)
    assistant=home/'Desktop/codex/工作台助手';assistant.mkdir(parents=True,exist_ok=True)
    if not (assistant/'.git').exists():subprocess.run(['git','init',str(assistant)],check=True,capture_output=True)
    q=lambda v:json.dumps(str(v),ensure_ascii=False)
    def config(scope):return f'[mcp_servers.workbench_{scope}]\ncommand = {q(sys.executable)}\nargs = [{q(folder/"server.py")}, "--scope", "{scope}"]\nstartup_timeout_sec = 10\ntool_timeout_sec = 30'
    section(assistant/'.codex/config.toml','WORKBENCH MANAGEMENT MCP',config('manage'))
    # Trust is necessary for project-scoped config, explicitly authorized for this assistant project.
    existing=(codex/'config.toml').read_text();project_key=f'[projects.{q(assistant)}]'
    if project_key not in existing:section(codex/'config.toml','WORKBENCH ASSISTANT PROJECT',project_key+'\ntrust_level = "trusted"')
    section(assistant/'AGENTS.md','WORKBENCH ASSISTANT RULE',Path(__file__).with_name('ASSISTANT.md').read_text())
    print('工具安装完成；专用助手目录：',assistant)
    print('配置和数据库备份：',backup)


if __name__=='__main__':main()
