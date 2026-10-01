# 个人工作台

单人离线使用的 macOS 计划与时间管理应用。年度方向、年度目标、本轮待办、周安排和日任务相互关联；同一条本轮待办会出现在 12 周计划与待办管理页，每日计划和实际时间记录分别保存。

## 运行

需要 Node.js、npm、Rust 工具链和 macOS 的 Xcode Command Line Tools。

```bash
npm install
npm run tauri dev
```

打包 macOS 应用：

```bash
npm run tauri build
```

前端检查：`npm run build`；统计逻辑测试：`npm test`。

## 数据

SQLite 数据库位于应用的配置目录，文件名 `workbench.db`。设置页可管理时间分类、简单重复事项以及备份。每次修改都会更新当天的本地备份；只保留最近 30 个有修改的日期。手动备份可保存到指定位置。从备份恢复会在下次完全退出并重开应用时生效，恢复前自动留一份原数据库副本。

首版不接入飞书或系统日历；旧记录可在对应历史日期使用日程界面手动补录。
