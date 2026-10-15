# 个人工作台

单人离线使用的 macOS 计划与时间管理应用，包含执行工作台、项目管理、12周计划三个入口。统一事项共享项目、周安排和日安排；计划时间与实际时间独立保存。

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

生成的应用位于 `src-tauri/target/release/bundle/macos/个人工作台.app`，退出已运行的工作台后，将它复制到“应用程序”即可更新，原有数据库继续保留。正式安装包包含前端资源，无需启动开发服务器。

若双击应用出现白屏，检查安装的是否为上述正式安装包。直接通过 `cargo build --release` 生成的可执行文件仍可能使用开发地址 `http://127.0.0.1:1420`；仅运行 `npm run build` 也不会更新已安装的应用。

前端检查：`npm run build`；统计逻辑测试：`npm test`。

## 数据

SQLite 数据库位于应用的配置目录，文件名 `workbench.db`。设置页可管理时间分类、简单重复事项以及备份。每次修改都会更新当天的本地备份；只保留最近 30 个有修改的日期。手动备份可保存到指定位置。从备份恢复会在下次完全退出并重开应用时生效，恢复前自动留一份原数据库副本。

首版不接入飞书或系统日历；旧记录可在对应历史日期使用日程界面手动补录。
