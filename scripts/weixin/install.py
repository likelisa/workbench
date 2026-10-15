#!/usr/bin/env python3
"""Install pinned bridge/runtime and a private launchd service on macOS."""
from pathlib import Path
import hashlib
import json
import os
import platform
import plistlib
import shutil
import subprocess
import sys
import tarfile
import urllib.request
from patch_bridge import patch

ROOT = Path.home()/'Library/Application Support/com.personal.workbench'
NODE_VERSION = 'v22.23.3'
BRIDGE_VERSION = '0.3.14'

def main():
    support = ROOT/'weixin';support.mkdir(parents=True,exist_ok=True);os.chmod(support,0o700)
    arch = 'arm64' if platform.machine()=='arm64' else 'x64'
    basename = f'node-{NODE_VERSION}-darwin-{arch}'
    node = support/basename/'bin/node'
    url = f'https://nodejs.org/dist/{NODE_VERSION}/'
    archive = support/(basename+'.tar.gz')
    sums = urllib.request.urlopen(url+'SHASUMS256.txt').read().decode()
    expected = next(line.split()[0] for line in sums.splitlines() if line.endswith('  '+archive.name))
    if not node.exists():
        subprocess.run(['curl','--fail','--location','--retry','5','--retry-all-errors','--silent','--show-error','--output',str(archive),url+archive.name],check=True)
        if hashlib.sha256(archive.read_bytes()).hexdigest()!=expected:raise RuntimeError('Node校验失败')
        with tarfile.open(archive) as t:
            for m in t.getmembers():
                if not (support/m.name).resolve().is_relative_to(support.resolve()):raise RuntimeError('无效归档路径')
                if m.issym() and not (support/m.name).parent.joinpath(m.linkname).resolve().is_relative_to(support.resolve()):raise RuntimeError('无效归档链接')
            t.extractall(support)
    elif archive.exists() and hashlib.sha256(archive.read_bytes()).hexdigest()!=expected:raise RuntimeError('Node归档校验失败')
    (support/'package.json').write_text(json.dumps({'private':True,'dependencies':{'codex-weixin':BRIDGE_VERSION}}))
    package = support/'node_modules/codex-weixin'
    if not (package/'package.json').exists() or json.loads((package/'package.json').read_text())['version']!=BRIDGE_VERSION:
        lock=Path(__file__).with_name('package-lock.json')
        if lock.exists():shutil.copy2(lock,support/'package-lock.json')
        subprocess.run([str(node),str(node.parent.parent/'lib/node_modules/npm/bin/npm-cli.js'),'ci' if lock.exists() else 'install','--ignore-scripts','--no-audit','--no-fund','--registry=https://registry.npmjs.org'],cwd=support,check=True)
    patch(package,support)
    assistant = Path.home()/'Desktop/codex/工作台助手'
    weixin_home = support/'codex-home';weixin_home.mkdir(exist_ok=True)
    # Inherit login/model configuration; keep private MCP configuration separate from desktop tools.
    source_home = Path.home()/'.codex'
    raw=(source_home/'config.toml').read_text();kept=[];skip=False
    for line in raw.splitlines():
        if line.strip().startswith('['):skip=line.strip().startswith(('[mcp_servers','[plugins','[skills','[[skills'))
        if not skip:kept.append(line)
    config='\n'.join(kept)+'\n\n'
    config+='[mcp_servers.workbench_manage]\ncommand = '+json.dumps(sys.executable)+'\nargs = '+json.dumps([str(ROOT/'ai-tools/server.py'),'--scope','weixin'],ensure_ascii=False)+'\nstartup_timeout_sec = 10\ntool_timeout_sec = 30\n'
    (weixin_home/'config.toml').write_text(config)
    for name in ['auth.json','AGENTS.md','rules']:
        source=source_home/name;target=weixin_home/name
        if source.exists() and not target.exists():target.symlink_to(source)
    # Global skills are deliberately absent from the dedicated WeChat home.
    state = support/'state';state.mkdir(exist_ok=True)
    # CLI overrides outrank project-local MCP config. Codex does not inherit all
    # bridge environment variables into MCP children, so force the scope here.
    wrapper=support/'codex-workbench'
    codex_binary=str(Path.home()/'.npm-global/bin/codex')
    overrides=[
        'mcp_servers.workbench_manage.command='+json.dumps(sys.executable),
        'mcp_servers.workbench_manage.args='+json.dumps([str(ROOT/'ai-tools/server.py'),'--scope','weixin'],ensure_ascii=False),
        'mcp_servers.workbench_manage.env={ WORKBENCH_WEIXIN_ROOT = '+json.dumps(str(support))+' }',
    ]
    wrapper.write_text('#!'+sys.executable+'\nimport os,sys\nargs='+repr(sum((['-c',x] for x in overrides),[]))+'\nos.execv('+repr(codex_binary)+',['+repr(codex_binary)+']+args+sys.argv[1:])\n')
    os.chmod(wrapper,0o700)
    config_path=state/'config.json';prior=json.loads(config_path.read_text()) if config_path.exists() else {}
    prior.update({'defaultCwd':str(assistant),'allowedWorkspaces':[str(assistant)],'codexBackend':'app-server','codexBin':str(wrapper),'codexExecSandbox':'read-only','streamReplies':False,'maxInboundBytes':10*1024*1024})
    config_path.write_text(json.dumps(prior,ensure_ascii=False,indent=2));os.chmod(config_path,0o600)
    binding=support/'bridge-config.json'
    if not binding.exists():binding.write_text(json.dumps({'allowedSenderIds':[],'authorizedAccountId':None}))
    os.chmod(binding,0o600)
    # Capture the lockfile so the installation is reproducible even if transitive versions change.
    shutil.copy2(support/'package-lock.json',Path(__file__).with_name('package-lock.json'))
    (support/'runtime.json').write_text(json.dumps({'node':str(node),'version':NODE_VERSION,'bridge_version':BRIDGE_VERSION,'sha256':expected},indent=2))
    start=support/'start.py'
    start.write_text('''import os,subprocess\nfrom pathlib import Path\nroot=Path(__file__).parent\nimport json\nnode=json.loads((root/'runtime.json').read_text())['node']\nsubprocess.run(['/usr/bin/open','-a','/Applications/个人工作台.app'],check=False)\nos.execv(node,[node,str(root/'node_modules/codex-weixin/dist/server/index.js')])\n''')
    label='com.personal.workbench.weixin'
    launch=Path.home()/'Library/LaunchAgents'/f'{label}.plist';launch.parent.mkdir(exist_ok=True)
    log=support/'logs';log.mkdir(exist_ok=True)
    env={'CODEX_WEIXIN_PORT':'8787','CODEX_WEIXIN_STATE_DIR':str(state),'CODEX_WEIXIN_OPEN':'0','CODEX_HOME':str(weixin_home),'WORKBENCH_WEIXIN_ROOT':str(support),'WORKBENCH_ASSISTANT_CWD':str(assistant),'PATH':str(node.parent)+':'+str(Path.home()/'.npm-global/bin')+':/usr/local/bin:/usr/bin:/bin:/usr/sbin:/sbin','HOME':str(Path.home())}
    with launch.open('wb') as f:plistlib.dump({'Label':label,'ProgramArguments':[sys.executable,str(start)],'WorkingDirectory':str(assistant),'EnvironmentVariables':env,'RunAtLoad':True,'KeepAlive':True,'ThrottleInterval':30,'StandardOutPath':str(log/'bridge.log'),'StandardErrorPath':str(log/'error.log')},f)
    target=f'gui/{os.getuid()}'
    subprocess.run(['launchctl','bootout',target,str(launch)],capture_output=True)
    subprocess.run(['launchctl','bootstrap',target,str(launch)],check=True)
    print('微信桥接已启动：http://127.0.0.1:8787；已有账号与授权保留，首次连接需扫码。')

if __name__=='__main__':main()
