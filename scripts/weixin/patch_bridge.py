"""Small version-pinned integration hooks, with fail-closed patch anchors."""
from pathlib import Path
import json
import shutil

def patch(package, support):
    package, support = Path(package), Path(support)
    assert json.loads((package/'package.json').read_text())['version'] == '0.3.14'
    helper = support/'workbench-context.mjs'
    shutil.copy2(Path(__file__).with_name('workbench-context.mjs'), helper)
    def edit(relative, replacements, imports=''):
        target = package/'dist'/relative
        original = target.with_suffix(target.suffix+'.workbench-original')
        if not original.exists(): shutil.copy2(target, original)
        value = original.read_text()
        for before, after in replacements:
            if value.count(before) != 1: raise RuntimeError('上游补丁定位失败：'+relative+' '+before[:70])
            value = value.replace(before, after, 1)
        target.write_text(imports + value)
    hook = helper.as_uri()
    edit('bridge/service.js', [
        ('async runCodexTurn(message, text, attachments = []) {',
         'async runCodexTurn(message, text, attachments = []) {\n        return withWorkbenchContext(this, message, () => this.runWorkbenchTurn(message, text, attachments));\n    }\n    async runWorkbenchTurn(message, text, attachments = []) {'),
        ('prompt: buildPrompt(text, attachments), cwd: workspace,',
         'prompt: workbenchPrompt(message) + buildPrompt(text, attachments), cwd: workspace,'),
        ('async handlePromptCommand(senderId, arg) {',
         'async handlePromptCommand(senderId, arg) {\n        await this.reply(senderId, "工作台助手请逐条发送任务；暂不支持合并消息缓冲。"); return;'),
        ('const items = await this.promptItemsFromMessageWithNotice(message);',
         'if (!message.text.trim() && message.attachments?.some(item => item.kind === "audio")) {\n            await this.reply(message.senderId, "这条语音没有微信转写，请补充文字。"); return;\n        }\n        const items = await this.promptItemsFromMessageWithNotice(message);')
    ], f'import {{ withWorkbenchContext, workbenchPrompt }} from {json.dumps(hook)};\n')
    edit('server/account-manager.js', [
        ('const service = this.bridgeFactory({\n            config,',
         'const service = this.bridgeFactory({\n            config: { ...config, workbenchAccountId: account.accountId },'),
        ('allowSender(accountId, senderId) {', 'allowSender(accountId, senderId) {\n        authorizeWorkbenchSender(accountId, senderId);'),
        ('removeSender(accountId, senderId) {', 'removeSender(accountId, senderId) {\n        revokeWorkbenchSender(accountId, senderId);'),
        ('async continueSession(accountId, sessionId, text, uploads = [], onProgress) {',
         'async continueSession(accountId, sessionId, text, uploads = [], onProgress) {\n        throw new Error("工作台操作请从已授权的微信发送，本地页面用于连接与管理。");')
    ], f'import {{ authorizeWorkbenchSender, revokeWorkbenchSender }} from {json.dumps(hook)};\n')
    edit('codex/app-server-runner.js', [
        ('model: input.model,\n            approvalPolicy: "never"',
         'model: input.model,\n            sandbox: "read-only",\n            approvalPolicy: "never"'),
        ('cwd: input.cwd,\n            approvalPolicy: "never",',
         'cwd: input.cwd,\n            sandboxPolicy: { type: "readOnly" },\n            approvalPolicy: "never",')
    ])
    edit('codex/isolated-home.js', [
        ('const home = fs.mkdtempSync(path.join(os.tmpdir(), "codex-weixin-home-"));',
         'if (process.env.WORKBENCH_WEIXIN_ROOT) return { path: sourceHome, cleanup: () => {} };\n    const home = fs.mkdtempSync(path.join(os.tmpdir(), "codex-weixin-home-"));')
    ])
    edit('state/config.js', [
        ('...loaded,\n        codexExecSandbox,',
         '...loaded,\n        ...(process.env.WORKBENCH_ASSISTANT_CWD ? { defaultCwd: process.env.WORKBENCH_ASSISTANT_CWD, codexBackend: "app-server", codexBin: path.join(process.env.WORKBENCH_WEIXIN_ROOT, "codex-workbench") } : {}),\n        codexExecSandbox,'),
        ('allowedWorkspaces: (loaded.allowedWorkspaces?.length ? loaded.allowedWorkspaces : base.allowedWorkspaces)',
         'allowedWorkspaces: (process.env.WORKBENCH_ASSISTANT_CWD ? [process.env.WORKBENCH_ASSISTANT_CWD] : (loaded.allowedWorkspaces?.length ? loaded.allowedWorkspaces : base.allowedWorkspaces))'),
        ('export function isWorkspaceAllowed(workspace, allowedWorkspaces) {',
         'export function isWorkspaceAllowed(workspace, allowedWorkspaces) {\n    if (process.env.WORKBENCH_ASSISTANT_CWD) return path.resolve(workspace) === path.resolve(process.env.WORKBENCH_ASSISTANT_CWD);')
    ])
    edit('server/http-server.js', [
        ('if (method === "POST" && url.pathname === "/api/update") {',
         'if (method === "POST" && url.pathname === "/api/update") {\n        throw new Error("工作台桥接固定版本0.3.14，请通过安装脚本复核补丁后升级。");')
    ])

if __name__ == '__main__':
    import sys
    patch(sys.argv[1], sys.argv[2])
