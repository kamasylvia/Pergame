# GOAL-STATE — alexandrie-infra(自托管基建改造)

## 1. 任务总览

- 目标:fork 侧自托管基建现代化——对象存储环境变量中立化(`S3_*`)、数据库 MySQL → PostgreSQL(随 Rust 后端重写落地)。
- 流程链:issue 登记 → 小步实施(每主题一 commit)→ 调研裁决 → Rust 重写(后续批次)。

## 2. 队列与状态

| # | 项 | 状态 | 产物/跟踪 |
|---|---|---|---|
| 1 | workspace 基建(AGENTS.md + .work/) | done(本批) | commit |
| 2 | issue ×2(pg / S3 重命名) | done(本批) | origin issue |
| 3 | `MINIO_*` → `S3_*` 重命名 | done(2026-11-07,smoke 待人工) | commit;验证见 TODO |
| 4 | Rust 技术栈调研 | done(2026-11-07,结论 go) | `.work/rust-migration/RESEARCH.md` |
| 5 | Rust 后端重写 + pg | pending(go 已裁决 2026-11-07;db 层 = Toasty + sqlx 逃逸;P0 前置待办) | issue #1 |
| 6 | MCP server(读写全量,内置 HTTP) | pending(随 #5 实施) | issue #3;spec `.work/features/mcp-server.md` |
| 7 | 目录索引文件(dataview) | pending(随 #5 实施;index 块 = #8 预设) | issue #4;spec `.work/features/directory-index.md` |
| 8 | Dataview 查询引擎(DQL 子集) | pending(随 #5 实施) | issue #5;spec `.work/features/dataview.md` |
| 9 | ob 插件兼容性盘点 | done(2026-11-07;决策门待用户) | `.work/obsidian-compat/RESEARCH.md` |
| 10 | M0 语法/数据兼容(wikilinks/callout/properties/导入导出) | pending(阻塞:用户裁决并入重写否) | 盘点报告 §6 |
| 11 | 自建插件系统 | pending(设计提案待批) | `.work/plugin-system/DESIGN.md`;批准后开 issue |
| 12 | 媒体播放器内置化(video-md) | pending(纯前端,时机待裁决) | issue #6;spec `features/markdown-parity.md` §2 |
| 13 | Joplin md 特性集对齐 | pending(纯前端,时机待裁决) | issue #7;spec 同上 §3 |

## 3. 续接点

- go 已裁决(2026-11-07,ORM = Toasty)。下一步:**新批次开工 Rust 重写**,从 P0 前置开始(核实 Snowflake 序列化、密码哈希算法、补集成测试),P1 做 Toasty spike(不过则回退 sqlx 为主);阶段拆分见报告 §6。
- 部署侧待用户手动:GitHub secrets 改名(`S3_ACCESSKEY/SECRETKEY`)。

## 4. 机制配置指针

- 编排/裁决:全局 `~/.zcode/AGENTS.md` §10-12;本任务暂未接入 `/matrix --review`。
- 项目约定:根 `AGENTS.md`。

## 5. Automation 登记

- 无(本批为会话内同步实施,无定时任务)。

## 6. 环境备忘

- origin = `git@github.com:kamasylvia/Alexandrie.git`(公开 fork);upstream = Smaug6739。
- compose backend = 本地构建(`build: ./backend`);上游预构建镜像不认 `S3_*`/pg。
- 本地开发对象存储:`make minio`(minio 二进制,9000/9001);compose 默认 rustfs。

## 7. 教训索引

- → `.work/LESSONS.md`
