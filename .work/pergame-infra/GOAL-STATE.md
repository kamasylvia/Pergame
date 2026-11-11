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
| 快照异地副本(fan-out) | 每用户 target(S3/WebDAV/外置 PG),快照后自动增量复制(密文原样) | 对象集异地副本 + 离线 zip 出口 | ✅ 实现并活测 |
| 离线 zip 出口 | portable 对象集格式(`meta.json`+`manifest.json`+`blobs/<sha256>`,条目名纯 ASCII) | 下载/上传还原/从 target 导出 | ✅ 实现并活测 |
| 本地误删恢复 | 回收站 | 用户明确要求,替代 git 级版本历史 | ✅ 实现(2026-11-09) |
| git 级版本控制 | — | 用户明确否决,不再评估 | ❌ 不做 |

### 下一步动作(按序)

1. ~~**回收站机制**~~ ✅ **已完成(2026-11-09)**:`migration 0005` + repo 软删/恢复/彻底删/清空 + `/api/trash` 四端点 + 前端 `/dashboard/trash` + 保留期 sweeper(`TRASH_RETENTION_DAYS` 默认 30)。spec = `features/trash.md`。**待活环境验证**:集成用例 `trash_lifecycle`(`cargo test --test integration -- --ignored`)。
2. **镜像发布 → NAS 部署**(2026-11-10 定型的部署模型):镜像由 GitHub Actions 在**发布 Release 时**构建推送 GHCR(`pergame-{backend,frontend,dr-backup}`,双架构),**NAS 只 pull 不 build**。**前置**:必须先在 GitHub 打一个 Release 触发 workflow,否则 `docker compose pull` 拉不到镜像(当前 GHCR 上还没有这三镜像)。
   - 已验证:backend 镜像本地构建后在真 compose 栈(pg+rustfs)里跑通 migration/建桶/health/注册登录/快照/portable zip 下载/还原。
   - **未验证**:前端镜像(GHCR 上不存在,本机 `bun install`+`nuxt build` 构建超时未完成)、dr-backup sidecar 镜像(同)。
3. 部署 smoke:按思源笔记 `Pergame 外部基础设施部署指南`(id `20261111085400-74zeey6`)的清单执行;NAS 上只需 `docker-compose.external.yml`(配置全内联,不需要 .env,不需要 clone 仓库)。旧版指南(id `20261110165052-x8mgczs`,自包含 5 服务 + .env 模型)已过时,2026-11-11 经用户确认删除。
4. GitHub secrets 改名 `MINIO_*` → `S3_ACCESSKEY/S3_SECRETKEY`(启用 Actions 部署时手动做)。
5. 前端走查补完:快照目标页(`?p=snapshotTargets`)的 CRUD/测试/同步/删除按钮交互(API 已全链活测,UI 只验到渲染与几何;IAB 的按钮 actionability 在本机会卡,需人工点一遍)。

### 换机续接指引

- **编译/测试环境**:backend-rs 需 `rustup override set 1.95.0`(仓库根 `backend-rs` 目录,toasty 要求 ≥1.95);Go 侧拉依赖需 `GOPROXY=https://goproxy.cn,direct`;前端 `bun install --registry https://registry.npmmirror.com`(注意 bun 版本差异会重写 bun.lock,改完先 `git checkout -- bun.lock`)。
- **活环境**:NAS pg18(5432)+ QuObjects(8010),凭据在 Infisical DEVOPS 项目(`PG_HOST/PG_PORT/PG_USERNAME/PG_PASSWORD`, `QUOBJECT_ENDPOINT/ACCESS_KEY/SECRET_KEY`;QUOBJECT_ENDPOINT 值是容器内主机名,本机用需替换为 `qnap.elf-balance.ts.net`)。Keycloak(`KEYCLOAK_URL` :10002)可复用于 OIDC 测试。
- **集成套件**:`cargo test --test integration -- --ignored --test-threads=1`(**必须串行**;10 用例)。需活后端 + `S3_ENDPOINT/S3_ACCESSKEY/S3_SECRETKEY/S3_BUCKET` + `DATABASE_*`(测试自清理与提权要用)+ `TARGET_TEST_BUCKET`(一个存在的空桶,fan-out 用例用)。
- **验证门**:`cargo test`(**49 单测**)、`cargo clippy --all-targets`(0)、`bun run lint` + `bun run lint:css`(0)、`go vet ./...` + `go build ./...`。
- **.work 布局**:本文件 + `TODO.md` + `LESSONS.md`;specs 在 `features/`(`backup-restore.md` 是备份子系统唯一正本,含 §1-12);上游追踪 `upstream-sync/TRACKING.md`。

### 最近完成的批次(供考古)

- **TZ 时区支持(2026-11-11,SiYuan fork UTC 显示教训移植)**:target cron 调度从硬编码 UTC 改为按进程时区匹配(`TZ` 环境变量,chrono Local 解码;不设 = UTC 容器默认)——`CronSpec::matches_utc` → `matches`(核心 `matches_in(ts, offset)` 可测,+8 时区语义单测;59 单测 + clippy 0)。两镜像补 tzdata(backend-rs Debian slim / sidecar alpine,否则 IANA 名解析不了静默回落 UTC);sidecar job `finished_at` 弃 `date -u` 改 `date -Iseconds`(跟 TZ,显示性质不动排序);DR 备份 stamp 目录本就 `date`(无 -u)跟 TZ,不动。存储层保持 UTC:`rfc3339_now()`(manifest)、快照 id(now_ms)、前端快照列表走 `last_modified` epoch → 浏览器本地,均不动(SiYuan 口径:标识符/存储格式不动,显示层跟时区)。docs `6.configurations-list.md` backend + sidecar 节各补 `TZ` 行;external compose 两处 TZ 提示注释。

- **compose 环境变量精简 + DR env bug(2026-11-11)**:`docker-compose.external.yml` 按「有代码默认值就不写」精简(170→135 行)——删 `BACKEND_PORT`(默认 8201)/`GIN_MODE`(Go 残留,Rust 版不读)/`COOKIE_DOMAIN`(默认 host-only,同源正好)/`FRONTEND_URL`(同源模式 CORS 不需要;配 SMTP 时须设,docs 已注)/`CONFIG_DISABLE_*`(默认 false)/`DATABASE_PORT/NAME`(默认 5432/pergame)/`S3_SECURE`(endpoint 带 scheme 时不参与)/`S3_BUCKET`(默认 pergame)/快照回收站 SMTP 四组(默认同效)/frontend 的 `NUXT_PUBLIC_BASE_API/BASE_CDN/CDN_ENDPOINT/BASE_URL/CONFIG_DISABLE_*`(nuxt.config 默认)/dr-backup 整块 env(镜像 ENV 自带默认)。**修真 bug**:两版 compose 的 backend 都挂 dr_conf/dr_jobs 卷但没设 `DR_CONF_DIR/DR_JOBS_DIR` → DR admin API 永远报 "sidecar not deployed",两版同补。docs `6.configurations-list.md` 补齐全量参数参考(快照/回收站节、同源反代与 frontend flags 节、DR sidecar 节、host-native S3 注记;删死变量 `API_URL`);TODO 记 frontend flags 断链缺口(`CONFIG_HIDE_NATIVE_LOGIN` 等 .env 有名 compose 未接)。

- **示例地址改 Tailscale 域名 + 部署笔记同步(2026-11-11)**:`docker-compose.external.yml` 三处示例地址(`FRONTEND_URL`/`S3_PUBLIC_URL`/`NUXT_PUBLIC_BASE_URL`)由占位 LAN IP `192.168.1.10` 改为 Tailscale 域名 `qnap.elf-balance.ts.net`;思源 external 部署笔记(20261111085400-74zeey6)的 compose 代码块以仓库文件全文重写 + 必改清单表行同步;GOAL-STATE 本条 smoke 指针从旧笔记(.env 模型)改指新笔记。旧笔记(20261110165052-x8mgczs)经用户确认删除(思源回收站可恢复)。

- **Siyuan issue #15 借鉴批次 E(2026-11-10)**:①`namespace` 作用域(`migration 0008`,并入 store prefix,创建后不可改)②`refs/owner.json` 归属认领(`INSTANCE_ID` 实例身份,写路径拒绝异主,`take-ownership` 显式接管)③保存前连接测试门禁(`probe-draft` + `connection_token` 服务端重算比对)。**判定不做**:还原身份重置——Pergame 快照按 user 隔离且不含实例身份/凭据,无作用对象。spec §14;两实例共用一桶的实机验证待人工。

- **部署模型定型:GHCR 构建 → NAS 拉取(2026-11-10,`e5eb262b`)**:**compose 不再本地 build**——backend/frontend/dr-backup 全部 `ghcr.io/kamasylvia/pergame-*:${PERGAME_TAG:-latest}`;release workflow 修掉「构建 `./backend`(Go 遗留树)」的错并补上 sidecar 镜像。**首次真跑 compose 暴露并修复 4 个只有运行时才现形的问题**:①运行时基底误用 `distroless/static`(sqlx 用 `tls-native-tls` → 动态链接 libssl,且缺 CA,永远起不来);②`gcr.io` 国内不可达(502),改同 registry 的 `debian-slim`;③运行时必须匹配 builder 的 Debian 版本(rust:1.95-slim = trixie/glibc 2.41,bookworm 运行时报 `GLIBC_2.38 not found`);④**`COOKIE_DOMAIN=localhost:8200`(compose 旧默认)产生非法 Domain `.localhost:8200`,认证 cookie 被丢弃 → 登录成功但所有操作 401**,已改默认值 + 代码 sanitize(剥 scheme/port/path,且单标签主机/IP 不加前导点)。**已实测**:真 compose 栈(pg+rustfs+backend 容器)跑通 migration/建桶/health/注册/登录/快照/portable zip 下载/还原。
- **多目标 fan-out + 锁 + portable zip(2026-11-10,`0fc5b2c4`→`6bdb75b6`→`73addc6b`→`e50bc2fb`→`aebfdc01`→`646654d6`→`901bc518`→`286d4b3e`→`ef108f0f`)**:
  - **P1 多目标**:`target/` 模块(S3/WebDAV/外置 PG 三实现 + 指纹身份去重 + 掩码回填 + deleteCheck/editCheck 门禁 + `empty` + 引用计数 prune/GC);**按用户** target(admin 勾选,全选=实例级);快照成功后**自动增量复制**(密文原样不解密);`GET /targets/:id/export/:snapshotId` 直接从 target 出包;前端 `?p=snapshotTargets` 管理页。
  - **P2 锁与只读**:备份共享锁/还原独占锁;中间件对工作区写请求 **423**(豁免 `/api/backup`、`/api/admin/dr-backup`、`/api/health`、`/api/auth`、`/api/users/@me`);`GET /admin/dr-backup/status` + 前端 `MaintenanceBanner` 提示只读。活测:还原中 status `read_only:true`、写请求 423、还原后恢复 200。
  - **P3**:5 段 cron 解析(`*`/`*/n`/`a-b`/`a-b/n`/逗号,dom|dow 同时受限按 cron 取 OR)+ 每分钟调度(错过不补跑、锁期间跳过)+ `backup_targets.schedule`(migration 0007);portable zip 三入口(下载/上传还原自动识别/从 target 导出)。
  - 走查抓到的真 bug(全部已修 + 有测试):**双前缀**(probe/empty/prune 把 row prefix 传给已自带 prefix 的 store → 对象数恒 0 → 门禁误放行删掉唯一异地副本)、**绝对 key 二次加前缀**(empty 报删成功其实没删)、**S3 `has()` 用字符串匹配 404**(SDK Display 只有 "service error" → 每次 replicate 中止)、**目录占位对象**(QuObjects 把 `.../blobs/` 物化成零字节 key → 门禁永远拒绝)、**INSERT/UPDATE 占位符错位**(schedule 落进 last_result、created_at 收到 text)、**migration 0006 被改动**(已应用迁移不可编辑,拆 0007)。
  - 测试建设:新增 `snapshot_targets_lifecycle`(创建/掩码/去重/增量/门禁/清空/删除全链)、测试自清理(`cleanup_user`)、`--test-threads=1` 强制(锁会互相 423)。
- **Toasty 铺向业务域(2026-11-10,`120caf4d`)**:新增 `NodeLink`/`Permission` 模型,links 4 个 + permissions 6 个 CRUD 改走 Toasty。**实测边界**:11 条 `WITH RECURSIVE` 留 sqlx(无 CTE 支持)、user-wide links 联表、邀请复合元组留 sqlx;`serde_json::Value` 无 `Field` impl 且 `#[document]` 不能带 `#[column]` → `user_settings` 留 sqlx。**用户裁决:到此为止**,剩余收益低于胶水成本。
- **DR 管理:表单化 + Siyuan 对位批次 D(2026-11-09,与对位同一压缩 commit 发布)**:①配置表单化——`dr_config.rs` JSON⇄bash conf、结构化端点、表单(原始 conf 降级折叠);②Siyuan kernel/backup 对位 5 项补齐——探测(`dr-backup.sh -p` + PROBE 行 + 前端 Test 按钮)、rclone crypt 每目的地加密、keep-last 保留、每目的地 cron(后端调度器,croner)、删除门禁+存储指纹+重复位置拒绝。**关键修复:rclone ≥1.76 不再解析 env-section remote → 改生成私有 config 文件**(真机全链依赖,探测首跑抓到)。spec §12-13;真机 crypt/cron/门禁分支待人工。
- **回收站(2026-11-09)**:`migration 0005` + 软删/恢复/彻底删/清空 + `/api/trash` 四端点 + `/dashboard/trash` 前端 + 保留期 sweeper。spec = `features/trash.md`。
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
- **Toasty 铺向业务域 ✅(2026-11-10,`120caf4d`)**:新增 `NodeLink`/`Permission` 模型;links 4 个 + permissions 6 个 CRUD 改走 Toasty。**边界实测**:11 条 `WITH RECURSIVE`、user-wide links 联表、邀请复合元组留 sqlx(Toasty 无 CTE);`serde_json::Value` 列无 `Field` impl、`#[document]` 不能带 `#[column]` → `user_settings` 留 sqlx,不强塞有损结构。验证:49/49 单测 + 集成 10/10。
- **运行时验证欠账**:Toasty 生成 SQL vs 0001 schema 实测(首个活 pg 时);Rust 集成套件已镜像 Go tests/ 主路径(10 用例)。

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
