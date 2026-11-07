# GOAL-STATE — alexandrie-infra(fork 现代化改造)

## 1. 任务总览

- 目标:fork 侧现代化——S3 中立命名(已完成)、MySQL → PostgreSQL(Rust 重写承载)、markdown/插件/MCP 功能域建设。
- 结构:**双批次**。批次 A = 前端(纯前端,独立开工);批次 B = 重写 + 功能域(等用户开工令)。

## 2. 队列与状态

**已完成(2026-11-07 会话,详见 commit 历史)**

| # | 项 | 产物 |
|---|---|---|
| 1 | workspace 基建(AGENTS.md + .work/) | commit `2fc8f27e` |
| 2 | issue 登记(#1-#8) | origin |
| 3 | `MINIO_*` → `S3_*` 重命名全链 | commit `4fdd3279`(smoke 待人工) |
| 4 | Rust 技术栈调研(结论 go,Toasty) | `rust-migration/RESEARCH.md` |
| 5 | ob 插件兼容性盘点(结论:不做生态复用) | `obsidian-compat/RESEARCH.md` |
| 6 | 插件系统设计提案(**已批准**) | `plugin-system/DESIGN.md` §9 已定案;issue #8 |

**批次 A — 前端批次(待开工令,共 ≈ 8-11 天)**

| # | 项 | 跟踪 |
|---|---|---|
| A1 | 媒体播放器内置化(video-md) | issue #6;spec `features/markdown-parity.md` §2 |
| A2 | Joplin md 特性集对齐 | issue #7;spec 同上 §3 |
| A3 | M0 ob 语法兼容(全量四件,已采纳;properties schema 与 #5 同批设计) | 兼容盘点报告 §6 |

**批次 B — 重写批次(等用户开工令,共 ≈ 5-7 周)**

| # | 项 | 跟踪 |
|---|---|---|
| B1 | Rust 重写 + pg(Toasty + sqlx 逃逸)。**P0 完成(2026-11-08)**:Snowflake=字符串序列化、bcrypt(10,兼容 Rust 校验零迁移)、集成测试补齐(编译/vet 过,运行时待人工) | issue #1;`rust-migration/RESEARCH.md` §8 |
| B2 | MCP server(随 B1) | issue #3;spec `features/mcp-server.md` |
| B3 | 目录索引文件(随 B1) | issue #4;spec `features/directory-index.md` |
| B4 | Dataview 查询引擎(随 B1) | issue #5;spec `features/dataview.md` |
| B5 | 插件系统 PS-A..E(排 B1 主体后,与 B2-B4 合流;含 ReadItLater 移植) | issue #8;spec `plugin-system/DESIGN.md` |

**B6(Backup v2)已取消(2026-11-08)**:issue #9 关闭,设计存档 `features/backup-v2.md`;替代 = 运维件 `scripts/dr-backup.sh`(N pg 源 × M rclone 目的地,已 dry-run 验证,实机验证待人工)。

## 3. 续接点

- **批次 B 已开工,P0 完成(2026-11-08)**:核实结论入 RESEARCH.md §8;测试已写未跑(本机无 mysql/minio,待人工 `go test ./tests/...`)。
- **下一步 = 批次 B 的 P1**(axum 骨架 + 配置 + DB 连接 + pg init migration + Toasty spike + snowflake + logger),**待用户放行 P1**;批次 A 仍待开工令。
- 用户侧待办:GitHub secrets 改名(`S3_ACCESSKEY/SECRETKEY`);有 docker 的机器跑自托管 smoke + 集成测试。

## 4. 机制配置指针

- 编排/裁决:全局 `~/.zcode/AGENTS.md` §10-12;本任务暂未接入 `/matrix --review`。
- 项目约定:根 `AGENTS.md`(含决策记录表)。

## 5. Automation 登记

- 无(会话内同步实施;批次开工后若长跑可评估巡检 automation)。

## 6. 环境备忘

- origin = `git@github.com:kamasylvia/Alexandrie.git`(公开 fork);upstream = Smaug6739。
- compose backend = 本地构建(`build: ./backend`);上游预构建镜像不认 `S3_*`/pg。
- 本地开发对象存储:`make minio`(minio 二进制,9000/9001);compose 默认 rustfs。
- 本机(macOS M2)无可用 docker runtime(`/usr/local/bin/docker` 死链)。

## 7. 教训索引

- → `.work/LESSONS.md`
