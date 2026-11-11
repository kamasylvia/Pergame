# TODO

> 2026-11-07 重构:双批次结构。已完成事项入 commit 历史 + AGENTS.md 决策记录,不留在本文件。

## 批次 A — 前端批次(纯前端,独立于重写;**待开工令**)

- [ ] **媒体播放器内置化**(video-md → 内置):issue #6;`![alt](url.mp4...)` → video/audio renderer override + PDF 内嵌;0.5-1 天;spec = `features/markdown-parity.md` §2。
- [ ] **Joplin md 特性集对齐**:issue #7;breaks/typographer(带设置开关)/ins/multimd-table/deflist/abbr/embeds/ABC;约 5-6 天;spec 同上 §3。
- [ ] **M0 ob 语法兼容(全量四件,2026-11-07 采纳)**:wikilinks 解析(名字解析走搜索 API)、callout 渲染(复用现有容器 CSS)、properties/frontmatter 渲染(schema 与 #5 dataview `metadata.*` 同批设计)、导入改写;`![[embed]]`/transclusion 后置;3-5 天。

## 批次 B — 重写批次(Rust + pg + 功能域;**等用户开工令**)

- [ ] **Rust 后端重写 + PostgreSQL**:issue #1;db 层 = Toasty + sqlx 逃逸层(P1 spike 验证不过则回退 sqlx 为主);全新空库;pg GIN + tsvector。**P0 前置已完成(2026-11-08,见 `rust-migration/RESEARCH.md` §8)**;下一步 = P1(骨架 + Toasty spike),待开工令。
  - [ ] P0 集成测试运行时验证(待人工:需 mysql + minio + 活后端;`go test ./tests/...`)
- [ ] **MCP server**:issue #3;读写全量、streamable HTTP `/api/mcp`、rmcp、每用户 API token;spec = `features/mcp-server.md`。
- [ ] **目录索引文件**:issue #4;自动建 index、设置管名(批量改名)/可见性、永远置顶(order=-1 双端强制);index 块 = #5 预设;spec = `features/directory-index.md`。
- [ ] **Dataview 查询引擎**:issue #5;DQL 子集、内置字段+metadata JSONB、`POST /api/nodes/query`、前端容器块;spec = `features/dataview.md`。
- [ ] **自建插件系统**:issue #8(已批准);PS-A..E ≈ 3.5-4.5 周,排重写主体后;ReadItLater 移植 = 生态验证头牌;spec = `plugin-system/DESIGN.md`(§9 已定案)。

- [ ] **笔记软连接/克隆**(Trilium 对位):同笔记多目录出现;克隆表 node_links + 环校验 + 三端点 + 树 CTE 扩展 + 前端;规格 = `features/note-links.md`(已含模型决策:克隆表方案,否决 content 分离重构)。

- [ ] **改名 Pergamena**(2026-11-09 定,替代现名 Pergame;**仅决定,不执行**):执行时全层落地——crate/binary、JWT/TOTP issuer、S3 桶与库名默认值、compose 容器、frontend/docs 品牌、GitHub 仓改名;破坏性,单独批次。


## 回收站(用户裁决 2026-11-09:本地误删恢复走回收站,不做 git 级版本)
- [x] **回收站机制** ✅(2026-11-09):软删 + 保留期 + 恢复/彻底删/清空 + 树查询过滤 + 前端视图。spec = `features/trash.md`。
  - 上游 Alexandrie 无回收站(物理 `DELETE FROM nodes` + 级联 + 删 S3 对象),本机实测确认。

## 待人工验证

> **2026-11-10 成熟度评估**:可开始人工测试,定位为「开发验证测试」而非验收测试。
> 后端 80 路由 / 18.5k 行 Rust / 218 Vue / 7 migration;61 单测 + 10 集成全绿;
> 认证/2FA/OIDC/节点/软链接/回收站/快照/还原/fan-out/只读锁均已对真实 pg + 对象存储活测。
> 部署步骤 = 思源 `Pergame 外部基础设施部署指南`(id 20261111085400-74zeey6);旧版指南已删。

**三个拦验收不拦测试的缺口:**

1. **`docker compose` 从未真正跑起来过**(最大未知)——全部验证都是 `cargo run` + `bun dev` 直连 NAS 的 pg/对象存储;镜像构建、6 服务编排(含 dr-backup sidecar)、卷挂载、healthcheck 依赖全为纸面设计。**测这个优先**。
2. **dr-backup 真实备份从未跑成**——本机无 `pg_dump`/`rclone`,只验到 job 协议 + 脚本被调用 + 错误回传;真实 dump→上传→`-l`→`-R` 一次未走。
3. **前端部分交互只有 API 验证**——快照目标页按钮点击、回收站页面只验到渲染与几何(本机 IAB actionability 卡),需手动点一遍。
4. **frontend flags 的 `.env`→compose 断链**(2026-11-11 发现):`CONFIG_HIDE_NATIVE_LOGIN`/`CONFIG_OIDC_PROVIDER_AUTO_REDIRECT` 在 `.env.example` 与 docs 有名,但自包含 compose 未映射成 `NUXT_PUBLIC_CONFIG_HIDE_NATIVE_LOGIN`/`NUXT_PUBLIC_CONFIG_OIDC_PROVIDER_AUTO_REDIRECT` → 设了不生效;OIDC-only 部署会踩。修法 = compose frontend 块补两行映射。

**明确未实现(按批次,非 bug)**:批次 A 全部(媒体播放器 / Joplin md 特性 / Obsidian 语法兼容)、MCP server、目录索引、Dataview、插件系统、Pergamena 改名。

### 建议测试顺序

- [ ] **1. compose 起全栈**:`docker compose up -d --build` → 6 服务全 healthy → `curl :8201/api/health` 返回 ok。首次需按文档 §5 处理前端镜像 fallback(取消 build 注释或手动打 tag)。
- [ ] **2. 核心 CRUD**:注册 → 登录 → 建 workspace → 写文档 → 传附件(验 CDN 链路 `/pergame/` → RustFS)。
- [ ] **3. 回收站**:删文档 → `/dashboard/trash` 看到 → 恢复 → 回树。
- [ ] **4. 快照全链**:建快照 → 列表 → 下载 zip(验 `meta.json`/`manifest.json`/`blobs/`)→ 改数据 → 还原(验二次确认统计 + 只读提示条)→ 上传该 zip 回灌。
- [ ] **5. 异地目标**:建 target(S3)→ Test(可达 + 对象数)→ Sync now → 目标桶出现 `manifests/`+`blobs/` → 有对象时 Delete 被拒 → Empty 后才可删。
- [ ] **6. 实例 DR**:配置 dr-backup.conf → Back up now → 看输出;真实目的地跑 `-l` / `-R`。
- [ ] **7. 进阶**:2FA(enroll/verify/recovery)、OIDC(Keycloak 环境已有)、SMTP 邮件。

### 旧条目

- [ ] 自托管 smoke(本机无 docker):`docker compose up --build` 后端正常启动(`S3_*` 链路)、注册/登录可用。改名后默认值已切 `pergame`(DB/桶/CDN 路径),首次起库即新名。
- [ ] GitHub repo secrets:`MINIO_ACCESSKEY`/`MINIO_SECRETKEY` 手动改名 `S3_ACCESSKEY`/`S3_SECRETKEY`(deploy_to_prod.yml 引用已改)。
- [ ] `scripts/dr-backup.sh` 实机验证(需 pg_dump + rclone + 真实目的地):dry-run 全链路 + 一次真实 dump/copy/prune;conf 复制自 `dr-backup.conf.example`(chmod 600)。conf 里库名/桶前缀已随改名切 `pergame-*`。还原用 `-R <source> [-t <stamp>]`(非交互加 `-y`;`-l` 列快照)。
- [ ] push 51+ commits 到 origin(kamasylvia/Pergame)——待拍板(公开仓 push 前可按全局规则压缩重组)。(2026-11-09:首 15 个已压缩为 5 主题 commits 并推上 origin/main `6e0abd8a`;此后增量照常。)

## 维护说明

- 项目约定与决策记录 → 根 `AGENTS.md`。
- 批次/续接点 → `pergame-infra/GOAL-STATE.md`;教训 → `LESSONS.md`;spec → `features/`、`plugin-system/`、`rust-migration/`、`obsidian-compat/`。
- issue 索引:#1 pg · #2 S3 改名(已实施) · #3 MCP · #4 索引 · #5 dataview · #6/#7 批次 A · #8 插件系统。

## 备份子系统(2026-11-10 三批全落地)

- [ ] **实机验证**:有 docker 的机器上跑 `docker compose up` → Instance DR 配 conf → 立即备份 / `-l` / `-R`;前端异地方向(settings → Offsite snapshot targets)建 target 跑一次真实异地复制。

## 换机续接(2026-11-09)

- 续接点唯一正本 = `.work/pergame-infra/GOAL-STATE.md` §3(当前精确位置 / 下一步动作 / 环境指引 / 验证门 / 最近批次)。
- 环境重建三件套:`rustup override set 1.95.0`(backend-rs)、`GOPROXY=https://goproxy.cn,direct`、`bun install --registry https://registry.npmmirror.com`。
