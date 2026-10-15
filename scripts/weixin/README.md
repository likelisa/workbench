# 微信工作台助手

运行 `python3 scripts/workbench-tools/install.py`，再运行 `python3 scripts/weixin/install.py`。需要新版工作台App；安装管理工具不会自动重建App。

独立安装Node v22.23.3和codex-weixin 0.3.14，使用依赖锁，不改变全局Node。运行目录为 `~/Library/Application Support/com.personal.workbench/weixin`；LaunchAgent为 `~/Library/LaunchAgents/com.personal.workbench.weixin.plist`。开机登录后运行，自动打开工作台。Mac休眠、离线或工作台关闭时不可操作。

打开 [本地连接页面](http://127.0.0.1:8787)，点“添加微信”扫码。从手机向助手发消息，在管理页授权你的发送账号。仅允许一个账号；更换前先撤销原授权。网页用于连接与管理，工作台操作从微信发起。

固定App Server后端，不回退codex exec；模型沿用当前Codex配置。专用目录为 `~/Desktop/codex/工作台助手`。独立Codex配置和登录继承到codex-home，全局日程skill不加载。App Server线程和轮次明确使用只读文件权限，业务修改通过管理MCP进行。微信MCP与桌面MCP使用不同凭据。

固定版本的少量补丁由patch_bridge.py安装：每次真实微信消息串行绑定可信上下文，授权绑定账号与发送人，App Server只读策略，网页继续对话和合并消息缓冲禁用。消息结束后上下文失效。升级时必须复核补丁锚点，不能自动安装上游新版本。

文字和微信语音转写进入同一任务流程。截图保留本地附件并由模型查看，新增仍先确认。语音无转写时要求补充文字，不接入额外语音识别服务。

新增待办：自动建议分类与必要上级关联→展示完整草稿→微信回复返回的确认指令→事务保存。唯一待确认草稿可回复“确认”，多份须编号及版本。修改草稿产生新版；明确修改直接执行，删除先展示影响并确认。

检查服务：`launchctl print gui/$(id -u)/com.personal.workbench.weixin`。日志在运行目录的logs中，可能包含微信账号标识，不要公开。停止服务：`launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.personal.workbench.weixin.plist`；恢复：`launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.personal.workbench.weixin.plist`。

测试：独立Node执行 `--test scripts/weixin/test_context.mjs`。后端及MCP验收见 `scripts/workbench-tools/ACCEPTANCE.md`。端到端成功必须同时验证真实微信返回及工作台保存回执，不能仅凭扫码成功判断。
