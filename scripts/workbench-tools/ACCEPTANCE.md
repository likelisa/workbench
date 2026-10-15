# 2026-10-08 开发后续事项收集移除验收

开发 TODO、开发收件箱及项目详情的开发待办入口已移除。提交、导入、回执、Hook、会话扫描和 AI 提取代码已删除；管理任务与 AI 复盘工具保留。

通过项：

- MCP 仅开放 query、create_task、set_task_status、review_snapshot、save_review、operation_receipt 六项管理工具。旧提交与 TODO 回执名称被明确拒绝。
- 正式 App 已构建、签名校验并更新到 `/Applications/个人工作台.app`，运行中的本地接口拒绝旧导入方法。
- 在「工作台助手」目录使用桌面内置 Codex CLI 的独立临时会话，真实执行周期和复盘两项只读查询，均返回成功。当前工具列表没有开发 TODO 提交入口；没有直接写数据库或手工补收。
- App 关闭时管理操作明确返回“工作台未打开”。
- 全局提交配置、收集规则、旧 Hook、LaunchAgent、提交凭据和活动队列已清理；13 个旧聊天保留的提交工具进程已终止，核对剩余数量为 0。
- 正式数据库核对保持一致：开发待办 47 条、本轮待办 20 条、周任务 38 条、日任务 26 条、AI 复盘 0 条。历史开发待办表保留但不再被功能读取。旧链路待处理 1 条、提交队列待处理 0 条，原始文件均已归档。
- 日程 Skill 删除过时的收集说明，任务管理规则保留，Skill 校验通过。

自动验证：前端 71 项、Rust 23 项、Python 4 项通过；TypeScript/Vite 与发布 App 构建成功，Git 差异格式检查通过。原有 AGENTS.md 的其他修改未纳入此次功能改动。

本次未新增正式任务或复盘。界面自动化工具连接超时，未完成原生界面的交互检查；功能入口的移除通过源码、前端构建和已安装 App 的接口验证。

备份目录：`~/Library/Application Support/com.personal.workbench/update-backups/remove-dev-todo-20261008-211815`。包含原 App、源码、数据库、配置、规则、旧工具和队列；真实 Codex 调用记录为 `readonly-codex.jsonl`。原接入验收文档保存在该目录的 `source-before.tar.gz` 中。


# 2026-10-09 微信全面管理与确认草稿验收

- Rust新增9项测试，覆盖完整上级链预演及原子提交、真实确认消息、用户绑定、旧版本/取消/重复确认、并发变化、时间冲突回滚、单记录状态变更、删除保留历史，以及项目/计划/模板/重复事项/分类/统计。全部Rust32项通过。
- Python5项、桥接上下文1项、前端71项通过；TypeScript/Vite及发布App构建成功。
- 新版已安装并重启。正式App上的周期、项目类型、阶段、项目规划、重复事项、每日模板只读查询和日期范围统计均成功。验收未新建正式任务。
- Node v22.23.3独立安装，SHA256校验；codex-weixin固定0.3.14并记录依赖锁。LaunchAgent已运行，账号已扫码连接并在管理页授权，真实微信查询已收到。
- 真实微信查询已成功回传；用户通过微信完成了任务状态和日期修改，工作台只读核对一致。初次回执发现项目配置覆盖微信凭据，已用CLI配置覆盖修正；修正后真实微信查询成功，运行中的MCP明确使用weixin scope，后端核对真实发送账号和消息上下文。
- 后台会话改用专用持久Codex home，避免上游临时home清理后无法恢复会话。已迁移当前会话，并验证thread/read可恢复。
- 新增、修订、确认及取消通过隔离后端测试；语音转写和截图尚未用用户实际输入验证，不能声称已完成这些真实链路验收。
- App及数据库更新前备份：`~/Library/Application Support/com.personal.workbench/update-backups/weixin-20261009-232851`。

收尾部署修正了单独改状态时保持独立实际时间记录；最终二进制与数据库备份：`~/Library/Application Support/com.personal.workbench/update-backups/weixin-final-20261009-235042`。
