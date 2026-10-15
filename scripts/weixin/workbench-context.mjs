import fs from 'node:fs';
import path from 'node:path';

let queue = Promise.resolve();
function root() {
  const value = process.env.WORKBENCH_WEIXIN_ROOT;
  if (!value) throw new Error('工作台微信配置缺失');
  return value;
}
function config() { return JSON.parse(fs.readFileSync(path.join(root(), 'bridge-config.json'), 'utf8')); }
function atomic(file, value) {
  fs.writeFileSync(file + '.tmp', JSON.stringify(value), { mode: 0o600 });
  fs.renameSync(file + '.tmp', file);
}
export function authorizeWorkbenchSender(accountId, senderId) {
  const file = path.join(root(), 'bridge-config.json');
  const cfg = config();
  if (cfg.allowedSenderIds.length &&
      (cfg.allowedSenderIds[0] !== senderId || cfg.authorizedAccountId !== accountId)) {
    throw new Error('第一版仅授权一个微信账号；请先移除原授权。');
  }
  cfg.allowedSenderIds = [senderId];
  cfg.authorizedAccountId = accountId;
  atomic(file, cfg);
}
export function revokeWorkbenchSender(accountId, senderId) {
  const cfg = config();
  if (cfg.authorizedAccountId === accountId && cfg.allowedSenderIds.includes(senderId)) {
    cfg.allowedSenderIds = [];
    cfg.authorizedAccountId = null;
    atomic(path.join(root(), 'bridge-config.json'), cfg);
  }
}
export function withWorkbenchContext(service, message, action) {
  const task = queue.then(async () => {
    const cfg = config();
    const accountId = service.options.config.workbenchAccountId;
    if (cfg.allowedSenderIds.length !== 1 || cfg.allowedSenderIds[0] !== message.senderId ||
        cfg.authorizedAccountId !== accountId) {
      throw new Error('请先在本地管理页授权你的微信账号。');
    }
    const context = {
      accountId, senderId: message.senderId, messageId: message.id,
      text: message.text, receivedAt: Date.now()
    };
    const file = path.join(root(), 'context.json');
    atomic(file, context);
    try { return await action(); }
    finally { atomic(file, { ...context, receivedAt: 0, text: '' }); }
  });
  queue = task.catch(() => {});
  return task;
}
export function workbenchPrompt(message) {
  const received = new Intl.DateTimeFormat('zh-CN', {
    timeZone: 'Asia/Shanghai', dateStyle: 'full', timeStyle: 'long'
  }).format(new Date());
  return `工作台助手。消息编号：${message.id}；接收时间：${received}。相对日期按此北京时间解释。\n` +
    '仅使用 workbench_manage 管理工具；不加载日程skill。新增待办先查询并补全分类与关联，生成草稿并把完整确认单发给用户，等待用户回复确认。确认文本由服务端核对真实微信消息。明确修改直接执行，删除先确认。\n';
}
