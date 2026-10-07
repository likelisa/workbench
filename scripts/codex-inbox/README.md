# Codex 开发收件箱

独立于本轮待办、日任务、周任务和年度目标。来源仓库自动识别；用户可另行关联一个工作台项目。关联不会创建任何执行任务。

## 安装（macOS）

运行 `python3 scripts/codex-inbox/install.py`，然后在 Codex `/hooks` 中审核并信任新增的 Stop Hook。安装器追加 Hook，保留已有 hooks 和 `config.toml` 的 notify 配置。已启动会话可能需要重新打开才能加载 Hook。

后台程序通过 LaunchAgent `com.personal.workbench.codex-inbox` 每 30 秒处理队列。默认使用桌面应用附带的 Codex，沿用登录和模型设置；提取需要联网并消耗 Codex 额度。不会启动后续开发。

数据位于 `~/Library/Application Support/com.personal.workbench/codex-inbox/`。pending 是待提取事件；ready 是待导入结果；processed 是已导入原始事件。工作台不运行时继续收集，下次打开导入。来源回复、聊天标识和文件路径仅保存在本机；模型提取请求会将该轮回复发给当前配置的 Codex 服务。

每轮最终回复中的具体未来行动（含可选建议）进入收件箱。纯解释、已完成工作和虚构示例不收集。程序不会回溯历史或扫描其他机器/云端聊天。语义提取不是零误差；保留引用供核查。工作台内编辑、完成、关联、删除均优先于自动重放；删除保留隐藏的去重标记，防止旧事件复活。自动采集不会推断任务已完成。

跨 worktree 使用 Git common directory 识别同一仓库。相同来源项目和规范化标题合并；语义近似但标题不同的事项不会擅自合并。关联工作台项目只能由用户选择，项目删除后解除关联而保留开发待办。

## 停用

在 Codex `/hooks` 禁用该 Stop Hook。运行 `launchctl bootout gui/$(id -u) ~/Library/LaunchAgents/com.personal.workbench.codex-inbox.plist` 停止后台整理。已有队列和待办保留。删除此 LaunchAgent plist 可防止下次登录自动启动。不要删除其他 Hook 或覆盖 notify 配置。

## 验证

- `python3 -m unittest discover -s scripts/codex-inbox -p 'test_*.py'`
- `npm test -- src/devInbox.test.ts`
- 在 src-tauri 中运行 `cargo test --lib dev_inbox::tests`

运行真实 Codex 回合后，应看到 `capture-status.json` 更新时间，以及 pending → ready → processed 的转换。Hook 启用和真实触发是两项检查；不能只凭配置文件存在判断成功。
