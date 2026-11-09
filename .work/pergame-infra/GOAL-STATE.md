# GOAL-STATE — pergame-infra(fork 现代化改造;2026-11-08 定名 Collectanea,2026-11-09 随仓库改名全量改为 Pergame,目录名 pergame-infra)

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

**改名落地(2026-11-09)✅**:Collectanea → Pergame 全量(brand + go.mod/crate `pergame-rs`/容器名/DB 与桶默认值/JWT issuer/前端运行时标识/logo 资产 `Pergame-logo-*`/链接;上游 alexandrie 遗留清理为 fork 身份)。验证:go vet/build、cargo check + 20/20 单测、eslint、grep 闸门全绿。

## 3. 续接点(2026-11-09 换机落盘)

- **换机续接指引**:backend-rs 可 `cargo run` 直连 NAS pg18(5432)+ QuObjects(8010),凭据在 Infisical DEVOPS 项目(PG_*/QUOBJECT_*);集成套件 `cargo test --test integration -- --ignored`(5+1 用例全绿);本仓 main 已推远程(2026-11-09)。
- **待续**:① 软连接 NL-C(备份 zip 含 links 表)+ 前端树渲染(建议设计稿);② 2FA 全链路自测(可程序化);③ OIDC/邮件实测(待 provider/SMTP);④ S3 TLS delta;⑤ docs 站 Rust 化。
- 旧续接点(2026-11-08/09 夜间):

- **已完成(2026-11-08 夜)**:P0 ✅ → P1 ✅(骨架+spike) → **P2 核心认证域 ✅(19d0643c,13/13 单测)**。
- ~~**新增批次候选:Collectanea 改名落地**~~ **✅ 已落地(2026-11-09,全量 + fork 身份清理,用户拍板;见 AGENTS.md 决策记录 2026-11-09 行)**。
- **下一步(按序)**:① 软连接 NL-B 后端完成 ✅(本轮:CTE link 臂并入递归项 OR EXISTS、活验证 6/6 含 soft_links 生命周期);前端部分待设计确认;② NL-C 备份 zip 含 links;③ 2FA 全链路自测(可程序化,无需扫码);④ OIDC/邮件待 provider/SMTP;⑤ push(待拍板);dr-backup 去重 ✅(375c423b)。② 活环境剩余手测(2FA 扫码/OIDC/邮件实测);③ docs 站 deploy 文档 Rust 化;④ S3 TLS delta;⑤ push + 镜像发布(待拍板)。原「全部待用户/活环境」清单并入以上。① 活环境联调:compose up(postgres 已切,994a2af0)→ 跑 `cargo test --test integration -- --ignored` + Go tests/ + 手测 OIDC/2FA/backup;② push + 镜像发布工作流(用户拍板);③ S3 CA_PATH/INSECURE_TLS(已记录 delta)。
- **运行时验证欠账**(首个活 pg 时):Toasty 生成 SQL vs 0001 schema 实测;Rust 版集成测试镜像 Go tests/ auth 用例。
- 环境限制:本机无 docker/pg/mysql → 编译+单测级验证。批次 A 待开工令;GitHub secrets 改名待用户手动。

## 4. 机制配置指针

- 编排/裁决:全局 `~/.zcode/AGENTS.md` §10-12;本任务暂未接入 `/matrix --review`。
- 项目约定:根 `AGENTS.md`(含决策记录表)。

## 5. Automation 登记

- ~~`automation-bf0d4eba-37af-4f7f-ad51-82092acf29d8`「夜间接力:批次 B 自动推进(每45分钟)」~~ **已删除**(2026-11-08 用户指令「保活巡检停止」;此前先已 CronPauseSelf)。夜间共完成接力 12 轮,批次 B 代码面收口,详见队列。

## 6. 环境备忘

- origin = `git@github.com:kamasylvia/Pergame.git`(公开 fork);upstream = Smaug6739。
- compose backend = 本地构建(`build: ./backend`);上游预构建镜像不认 `S3_*`/pg。
- 本地开发对象存储:`make minio`(minio 二进制,9000/9001);compose 默认 rustfs。
- 本机(macOS M2)无可用 docker runtime(`/usr/local/bin/docker` 死链)。

## 7. 教训索引

- → `.work/LESSONS.md`
