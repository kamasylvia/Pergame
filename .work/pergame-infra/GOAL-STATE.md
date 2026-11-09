# GOAL-STATE — pergame-infra(fork 现代化改造;2026-11-08 定名 Collectanea,2026-11-09 随仓库改名全量改为 Pergame;最终定名 Pergamena 延期执行,目录名 pergame-infra)

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

## 3. 续接点(2026-11-09 第二次换机落盘)

### 当前精确位置

主力全部收口,本地无未推提交、工作树干净(远端 HEAD 以 `git rev-parse origin/main` 现查为准,勿依赖本文硬编码 SHA)。备份子系统已按用户裁决完成完整分层:

| 层 | 载体 | 用途 | 状态 |
|---|---|---|---|
| 用户级快照(应用内) | v2 manifest + 内容寻址 blobs(`{uid}/manifests/`+`blobs[-e2e]/`),keep-10 + GC | 误删/误改恢复、可下载、可上传还原 | ✅ 全部实现 |
| 定时自动快照 | `SNAPSHOT_INTERVAL_HOURS` ticker,复用 run_backup | 无人值守的用户级保护 | ✅ 实现(默认关) |
| 实例级 3-2-1 异地 | dr-backup sidecar(pg_dump → 多 S3/WebDAV 目的地) | **主力备份目标** | ✅ 实现,待容器内实机验证 |
| 本地误删恢复 | 回收站 | 用户明确要求,替代 git 级版本历史 | ✅ 实现(2026-11-09) |
| git 级版本控制 | — | 用户明确否决,不再评估 | ❌ 不做 |

### 下一步动作(按序)

1. ~~**回收站机制**~~ ✅ **已完成(2026-11-09)**:`migration 0005` + repo 软删/恢复/彻底删/清空 + `/api/trash` 四端点 + 前端 `/dashboard/trash` + 保留期 sweeper(`TRASH_RETENTION_DAYS` 默认 30)。spec = `features/trash.md`。**待活环境验证**:集成用例 `trash_lifecycle`(`cargo test --test integration -- --ignored`)。
2. 容器内实机验证备份全链(本机无 docker):`docker compose up` → 设置 Instance DR 配 conf → 立即备份 / `-l` / `-R` 还原;同时验证应用内快照创建/还原/下载。
3. 部署 smoke(思源笔记 `01-CS/02-NAS/00-Docker/Pergame 自托管部署指南` 有 9 步清单,含新增的快照与 DR 步骤)。
4. GitHub secrets 改名 `MINIO_*` → `S3_ACCESSKEY/S3_SECRETKEY`(启用 Actions 部署时手动做)。

### 换机续接指引

- **编译/测试环境**:backend-rs 需 `rustup override set 1.95.0`(仓库根 `backend-rs` 目录,toasty 要求 ≥1.95);Go 侧拉依赖需 `GOPROXY=https://goproxy.cn,direct`;前端 `bun install --registry https://registry.npmmirror.com`(注意 bun 版本差异会重写 bun.lock,改完先 `git checkout -- bun.lock`)。
- **活环境**:NAS pg18(5432)+ QuObjects(8010),凭据在 Infisical DEVOPS 项目(`PG_HOST/PG_PORT/PG_USERNAME/PG_PASSWORD`, `QUOBJECT_ENDPOINT/ACCESS_KEY/SECRET_KEY`;QUOBJECT_ENDPOINT 值是容器内主机名,本机用需替换为 `qnap.elf-balance.ts.net`)。Keycloak(`KEYCLOAK_URL` :10002)可复用于 OIDC 测试。
- **集成套件**:`cargo test --test integration -- --ignored`(8 用例,需活后端 + `S3_ENDPOINT/S3_ACCESSKEY/S3_SECRETKEY/S3_BUCKET` 环境变量供直连断言)。
- **验证门**:`cargo test`(26 单测)、`cargo clippy --all-targets`(0)、`bun run lint` + `bun run lint:css`(0)、`go vet ./...` + `go build ./...`。
- **.work 布局**:本文件 + `TODO.md` + `LESSONS.md`;specs 在 `features/`(`backup-restore.md` 是备份子系统唯一正本,含 §1-12);上游追踪 `upstream-sync/TRACKING.md`。

### 最近完成的批次(供考古)

- **备份子系统 A/B/C**(2026-11-09,commits `8ed0d261`/`2d0d9d55`/`1d8b1c71`/`5dcec24a`/`736057b3`/`571ec174`/`9fe0c3d5`):
  - A:还原前自动安全快照 + 快照 memo(manifest+sidecar)+ 还原预检(`/preview` 双入口,返回 incoming/current/replaced/added/removed/文件碰撞)
  - B:存储 v2 = manifest + 内容寻址 blobs(跨快照去重)+ 可选 AES-256-GCM/scrypt 加密(`PERGAME_SNAPSHOT_PASSPHRASE`)+ legacy v1 zip 兼容 + 下载直出 + blob GC
  - C:实例 DR = sidecar 容器 + admin API(conf `***` 掩码/回填、job 文件协议)+ 前端 Instance DR 视图 + `SNAPSHOT_INTERVAL_HOURS` 定时快照
  - 走查中抓到的真 bug(已修+单测):掩码重组重复尾引号、sidecar JSON 转义漏换行、v2 组装漏 manifest.json 成员、引擎格式门未放行 2.0、`skipped_files` 反序列化缺 default、`@me` 解析拒 sentinel、backend-rs 无 CORS 层
- **更早批次**:改名落地(5 主题压缩推送,旧 SHA 映射见下)、软连接 NL-A/B/C、2FA/OIDC/邮件活测、S3 TLS、docs Rust 化、上游两 fix 移植

### 历史记录(旧续接点,勿据此判断现状)

- **push 压缩映射(2026-11-09 第一次)**:15 commits → 5 主题(`30c8ba06` 改名 / `b445b6fa` 上游追踪 / `100473d2` 活测修复 / `ad76b47d` TLS / `6e0abd8a` 软链端到端),`--force-with-lease` 推 origin。旧 SHA 考古走 reflog。
- **已完成(2026-11-08 夜)**:P0 ✅ → P1 ✅ 骨架+spike → P2 核心认证域 ✅(`19d0643c`,13/13 单测)。
- **改名落地 ✅**、**2FA ✅**、**OIDC ✅**、**邮件 ✅(用户确认收件)**、**S3 TLS ✅**、**docs Rust 化 ✅**、**软连接三阶段 ✅** —— 详见 queue 与 commit 历史。
- **运行时验证欠账**:Toasty 生成 SQL vs 0001 schema 实测(首个活 pg 时);Rust 集成套件已镜像 Go tests/ 主路径(8 用例)。

## 4. 机制配置指针

- 编排/裁决:全局 `~/.zcode/AGENTS.md` §10-12;本任务暂未接入 `/matrix --review`。
- 项目约定:根 `AGENTS.md`(含决策记录表)。

## 5. Automation 登记

- ~~`automation-bf0d4eba-37af-4f7f-ad51-82092acf29d8`「夜间接力:批次 B 自动推进(每45分钟)」~~ **已删除**(2026-11-08 用户指令「保活巡检停止」;此前先已 CronPauseSelf)。夜间共完成接力 12 轮,批次 B 代码面收口,详见队列。

## 6. 环境备忘

- origin = `git@github.com:kamasylvia/Pergame.git`(公开 fork);upstream = Smaug6739/Alexandrie(2026-11-09 实测改名回 Alexandrie);上游增量追踪 = `.work/upstream-sync/TRACKING.md`。
- compose backend = 本地构建(`build: ./backend`);上游预构建镜像不认 `S3_*`/pg。
- 本地开发对象存储:`make minio`(minio 二进制,9000/9001);compose 默认 rustfs。
- 活 dev 环境 = NAS:pg18(5432)+ QuObjects(8010)+ Keycloak(10002,master realm;admin client 凭证在 Infisical DEVOPS 项目 KEYCLOAK_*)。
- S3 桶改名(2026-11-09 完成,QuObjects):建 `pergame` / `pergame-backups`,删空桶 `Collectanea` / `Collectanea-backups`(均 0 对象,无数据迁移);pg 库改名用户同日完成。
- 本机(macOS M2)无可用 docker runtime(`/usr/local/bin/docker` 死链)。

## 7. 教训索引

- → `.work/LESSONS.md`
