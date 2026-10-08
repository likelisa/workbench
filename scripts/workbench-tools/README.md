# 个人工作台 MCP

`server.py` 是无第三方依赖的 Python 3 stdio MCP 适配器。安装后从 macOS 应用数据目录运行，不依赖源码、当前目录或 worktree。

## 安装

运行 `python3 scripts/workbench-tools/install.py`。先备份数据库、Codex 配置和全局规则，然后追加全局提交工具、创建 `~/Desktop/codex/工作台助手` 及其项目级管理工具配置。重复安装保留其他配置。已打开的聊天如未加载新工具，需要重新打开；不能把配置写入成功视为所有旧聊天已经接通。

新链路真实验收通过后运行 `python3 scripts/workbench-tools/install.py --retire-legacy`：仅移除本项目 collector 的 Stop Hook，卸载对应 LaunchAgent，保留原始队列、已有收件箱和其他 Hook。

## 数据路径与能力

应用数据目录：`~/Library/Application Support/com.personal.workbench`。

- `ai-tools/server.py`：稳定工具入口。
- `ai-tools/submissions/`：原子写入的提交，支持多进程文件锁。
- `ai-tools/processed/`：导入后的原始提交，用于重试去重。
- `ai-tools/receipts/`：按提交编号哈希保存的回执。
- `ai-tools/capabilities.json`：submit/manage 两个能力凭据，仅当前用户可读写。
- `ai.sock`：App 启动的本机 Unix socket，退出后不可用，不监听网络端口。
- 数据库 `ai_operations`、`ai_audit`：请求去重、前后值与操作记录。
- `ai_snapshots`、`ai_reviews`：不可覆盖的复盘快照、报告版本。

全局只有 submit_todos、todo_receipt；专用助手项目额外开放 query、create_task、set_task_status、review_snapshot、save_review、operation_receipt。管理接口不开放 SQL、删除、批量重排和恢复备份。能力范围防止误调用，不隔离拥有完整本机权限的 Agent。

TODO 回执 queued 仅代表已持久暂存，imported 才代表数据库已有记录。App 每两秒导入，不扫描聊天，不调用模型提取。request_id 在重试时必须不变，内容不同则报错。source_directory 会按 Git common-dir 归一化 worktree 来源。session_id 无法确认时留空。

管理写操作在同一数据库事务中保存业务记录、请求结果和审计记录；提交后沿用每日在线备份，通知界面刷新。备份失败时返回 backup_error，业务提交不会被谎报为回滚。修改状态必须携带 query 返回的 version；冲突要重新确认新状态，不盲目重试。

## 复盘口径

年度任务按年度目标年份归属；十二周任务按 cycle_id 归属。日任务、计划、时间记录按上海时区日期范围统计。十二周范围是周期开始日到第 84 天，跨界时间记录只计算范围内部分。完成率基于生成时当前状态。计划时段、已记录用时分开；已有业务规则可能从带时间的日任务生成时间记录，不能宣称它们都是实际计时。没有历史状态证据时不推测历史变化。

先 review_snapshot，后 save_review。evidence 必须是快照 data 中存在的 JSON Pointer。新版本使用新 request_id，重试使用旧编号。报告在 App「复盘统计 → AI 复盘」查看。

## 验证

Python：`python3 -m unittest discover -s scripts/workbench-tools -p 'test_*.py'`

Rust：`cargo test --manifest-path src-tauri/Cargo.toml --lib ai_service::tests`

调试版支持 `WORKBENCH_TEST_DATA_DIR` 指向独立测试数据目录，界面数据库和本机工具接口使用同一目录；发布版忽略该变量。工具测试可用 `--root` 指向该目录。测试不得以手工补收或直接修改数据库替代真实 MCP 调用。
