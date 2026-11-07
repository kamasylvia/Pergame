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
| B1 | Rust 重写 + pg(Toasty + sqlx 逃逸)。**P0 ✅(2026-11-08)**;**P1 ✅(2026-11-08,commits c9fae7c0/4993e160)**:backend-rs 骨架(axum+config+snowflake+0001 pg schema+health)、Toasty spike **PASS**(编译门 + 共存审查;运行时验证移 P2 首查点) | issue #1;`rust-migration/RESEARCH.md` §8 |
| B2 | MCP server(随 B1) | issue #3;spec `features/mcp-server.md` |
| B3 | 目录索引文件(随 B1) | issue #4;spec `features/directory-index.md` |
| B4 | Dataview 查询引擎(随 B1) | issue #5;spec `features/dataview.md` |
| B5 | 插件系统 PS-A..E(排 B1 主体后,与 B2-B4 合流;含 ReadItLater 移植) | issue #8;spec `plugin-system/DESIGN.md` |

**B6(Backup v2)已取消(2026-11-08)**:issue #9 关闭,设计存档 `features/backup-v2.md`;替代 = 运维件 `scripts/dr-backup.sh`(N pg 源 × M rclone 目的地,已 dry-run 验证,实机验证待人工)。

## 3. 续接点

- **已完成(2026-11-08 夜)**:P0 ✅ → P1 ✅(骨架+spike) → **P2 核心认证域 ✅(19d0643c,13/13 单测)**。
- **下一步(按序,接力轮 7 起)**:① P2 尾欠账:① ~~P2.5~~ ✅(f52262a5:RequestCtx + ADMIN_ACCOUNTS,16/16 单测);② ~~PATCH /users~~ ✅(本轮,含 /password 分支);③ ~~TOTP 域~~ ✅(3260cfe5:2fa request/confirm/verify/disable + backup codes + RecoveryCode 模型);④ ~~P3 内容域~~ ✅(cd68c4b3:nodes CRUD/树 CTE/共享/公开子树/FTS OR-tsquery+ts_rank+ts_headline/守卫含祖先属主兜底;S3 清理挂 P3.5);⑤ P2 尾欠账:POST /auth/logout/all、OIDC 域(controller/oidc + oidc_provider repo + link flow)、邮件域(request-reset/reset-password,P4 归口亦可)。
- **运行时验证欠账**(首个活 pg 时):Toasty 生成 SQL vs 0001 schema 实测;Rust 版集成测试镜像 Go tests/ auth 用例。
- 环境限制:本机无 docker/pg/mysql → 编译+单测级验证。批次 A 待开工令;GitHub secrets 改名待用户手动。

## 4. 机制配置指针

- 编排/裁决:全局 `~/.zcode/AGENTS.md` §10-12;本任务暂未接入 `/matrix --review`。
- 项目约定:根 `AGENTS.md`(含决策记录表)。

## 5. Automation 登记

- `automation-bf0d4eba-37af-4f7f-ad51-82092acf29d8`「夜间接力:批次 B 自动推进(每45分钟)」(2026-11-08 建):读本文件续接点 → 实施下一项 → commit-per-task;重大决策记「待裁决」并自停;额度耗尽按重置时间自接力。

## 6. 环境备忘

- origin = `git@github.com:kamasylvia/Alexandrie.git`(公开 fork);upstream = Smaug6739。
- compose backend = 本地构建(`build: ./backend`);上游预构建镜像不认 `S3_*`/pg。
- 本地开发对象存储:`make minio`(minio 二进制,9000/9001);compose 默认 rustfs。
- 本机(macOS M2)无可用 docker runtime(`/usr/local/bin/docker` 死链)。

## 7. 教训索引

- → `.work/LESSONS.md`
