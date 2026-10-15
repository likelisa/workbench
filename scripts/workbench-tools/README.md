# 个人工作台管理 MCP

`server.py` 是无第三方依赖的 Python 3 stdio MCP 适配器。稳定入口位于 `~/Library/Application Support/com.personal.workbench/ai-tools/server.py`，不依赖源码或 worktree。

运行 `python3 scripts/workbench-tools/install.py` 安装项目级管理工具，并备份原有数据库、配置与规则。专用项目为 `~/Desktop/codex/工作台助手`；项目配置位于 `.codex/config.toml`。旧聊天未加载新配置时需重新打开。

## 可用工具

- query：查询全部业务对象，返回版本；支持搜索、项目、周期、上级、周次和日期筛选及分页。
- execute_operation：执行用户明确指定的一项业务操作；修改需当前版本。新增待办、重复事项和删除不可绕过草稿。
- create_draft / revise_draft / query_drafts / confirm_draft / cancel_draft：预演、保存、修订、查询、确认或取消方案。缺失上级用ref/$ref引用，目标时间分类可用goal:$ref。
- set_task_status：明确修改一条任务状态，不联动其他层级。
- statistics：按上海日期统计任务状态、计划分钟、已记录用时及分类分布。
- review_snapshot / save_review：生成年度或十二周快照并保存有依据的复盘。
- operation_receipt：查询原request_id结果，超时重试不换编号。

完整助手规则在 [ASSISTANT.md](ASSISTANT.md)，不加载日程skill。支持方向、年度目标、周期、三层任务、项目及项目类型/阶段/规划/进展、计划时段、每日模板、重复事项、时间记录和分类颜色。分类结构沿用现有制度，备份恢复保留在桌面端。

App必须运行。通过本机Unix socket及manage/weixin独立凭据调用业务服务，不暴露数据库或任意SQL。微信模式从桥接写入的可信上下文读取用户、消息和真实确认文本；仅授权的单一账号可操作自己的草稿。新增任务和删除在服务端强制确认。拥有完整本机权限的用户或进程仍可访问本机数据，这不是多用户隔离系统。

草稿以SAVEPOINT预演后回滚业务写入，返回全部新增、删除及关联影响；提交在单个事务内完成并通知界面刷新。草稿采用保守的全部业务数据指纹，期间任何业务数据变化都要求修订和再次确认；内部草稿、审计和回执不会使其他草稿失效。直接修改使用目标记录version。所有写操作去重并记录审计；在线备份失败返回backup_error。

桌面manage入口的confirm_draft需要confirmation_text；微信入口忽略模型传入的确认文本，读取后续真实微信消息，仅接受“确认 草稿编号 版本”，或唯一待确认草稿时的“确认”。服务端只将status=saved视为业务保存成功。旧create_task不再向MCP开放，App本身仍可使用原创建入口。

复盘基于生成时当前任务状态。年度任务按年度目标年份归属，十二周任务按周期归属；日任务、计划和时间记录按上海时区日期范围统计。计划时段和已记录用时分开统计；现有规则可能由带时间的日任务生成时间记录，不能将所有记录都视为经核实的实际投入。没有历史证据时不推测历史变化。

开发后续事项收集已取消。历史数据表和原始文件只作保留备份，当前 App 和工具不再读取或导入它们。

## 验证

Python：`python3 -m unittest discover -s scripts/workbench-tools -p 'test_*.py'`

Rust：`cargo test --manifest-path src-tauri/Cargo.toml --lib`

调试 App 支持 `WORKBENCH_TEST_DATA_DIR` 指向独立测试目录，MCP 可用 `--root` 指向同一目录。正式 App 忽略该变量。验收写入必须通过工具和 App，不直接修改数据库。
