# AGENTS.md — Pergame (fork) 工作区约定

> 面向 agent 的项目约定。全局约束见 `~/.zcode/AGENTS.md`;任务状态唯一事实源 = `.work/TODO.md` 与 `.work/pergame-infra/GOAL-STATE.md`。

## 项目概览

Pergame = 自托管知识库 + 扩展 Markdown 编辑器(Notion/Obsidian 替代品;名取帕加蒙,古代第二图书馆)。

- **本仓为 fork**:origin = `kamasylvia/Pergame`(公开),upstream = `Smaug6739/Alexandrie`(2026-11-09 实测上游已由 `Collectanea` 改回 `Alexandrie`)。fork 自定义改动尽量保持可合并上游的形态(migration 编号不重排、不无谓重构上游文件);2026-11-09 全量改名后 Go 层(module `pergame`)与品牌字符串已不可与上游直接合并,结构性约定(migration 编号顺延等)仍保留。
- 架构:`frontend/` Nuxt 3(PWA)+ `backend/` Go(gin + sqlx)+ `docs/` Nuxt 文档站 + docker compose 全家桶。
- 存储:MySQL 8(应用数据)+ S3 兼容对象存储(compose 默认 RustFS,兼容 MinIO/Garage 等)。

## 常用命令

```bash
make backend      # go run backend
make frontend     # nuxt dev (bun)
make documentation# docs 站 dev
make minio        # 本地 minio server(开发用,端口 9000)
make update       # 前端依赖更新 + go mod tidy
docker compose up -d          # 生产形态全栈(见下「镜像策略」)
docker compose -f docker-compose.dev.yml up   # 前后端 dev 容器(带挂载)
```

- 后端集成测试:`cd backend && go test ./tests/...`(需本地 8201 起着后端,纯 HTTP 黑盒)。
- SQL 只存在于 `backend/repositories/*.go` 与 `backend/app/migrations/*.sql`(golang-migrate,embed 进二进制,启动自动跑);`config.toml` 同样 go:embed,`CONFIG_PATH` 可覆盖。

## 决策记录

| 日期 | 决策 |
|---|---|
| 2026-11-07 | **数据库 MySQL → PostgreSQL 不在 Go 层做**,随 Rust 后端重写一并落地(全新空库,不做数据迁移)。调研报告:`.work/rust-migration/RESEARCH.md` |
| 2026-11-07 | **Rust 重写 db 层 = Toasty ORM + sqlx 逃逸层**(用户裁决:练手项目,直接上前沿技术;schema 主权留 sqlx migrate,P1 spike 验证不过则回退 sqlx 为主) |
| 2026-11-07 | **附件/资源主存 = S3,pg 只管结构化数据**(评估过二进制进 pg、S3 仅备份 → 否决:保 CDN 直出、免 Range 流式实现;存储层 trait 抽象保留为将来选项) |
| 2026-11-08 | **重写后应用定名 Collectanea**(帕加蒙,古代第二图书馆;撞名已查仅 Typst 文献包)。改名落地范围(仓库/品牌/env/容器名)未定,实施时另立批次——env 变量与 compose 容器名属破坏性变更 |
| 2026-11-09 | **项目改名落地 Collectanea → Pergame**(仓库与目录已先行改名,项目内全量跟进;用户拍板「全量改 + 上游遗留清理为 fork 身份」):brand 字符串、go module `pergame`、crate/二进制 `pergame-rs`、compose 容器/网络/镜像 tag、DB/桶/凭据默认值、CDN 路径 `/pergame/`、JWT/TOTP issuer、前端 theme id/IndexedDB/`PergameError`、logo 资产 `Pergame-logo-*`(修好 `Collectanea-logo-*` 断链)、fork 面向链接 → `kamasylvia/Pergame`。保留:LICENSE 上游版权、`alexandrie-hub.fr` 域名端点、FUNDING.yml 上游赞助。Breaking:JWT 需重登、DB/桶/CDN 路径默认值变更(既有 volume/桶重建或手工改名)、前端本地偏好重置、Go 层上游可合并性作废 |
| 2026-11-08 | **DR 两级定位**:实例级 = `pg_dump + S3 桶异地副本 + secrets` 三件套;应用内 Backup v2 zip **砍除**(issue #9 关闭,spec 存档 `features/backup-v2.md`)。多端备份 = 运维件 `scripts/dr-backup.sh`(N pg 源 × M rclone 目的地,retention/校验/dry-run);将来开放多用户或需单用户跨实例搬家时 v2 再立项 |
| 2026-11-07 | **对象存储环境变量 `MINIO_*` → `S3_*`**(存储中立命名);Go 读点与 compose/docs/workflow 同批改名 |
| 2026-11-07 | **compose 的 backend 服务本地构建**(`build: ./backend`):上游预构建镜像 `ghcr.io/smaug6739/alexandrie-backend` 不认 `S3_*`、且后续不支持 pg |
| 2026-11-07 | `.work/` 目录进 git(TODO/LESSONS/调研产物,公开仓,无敏感内容) |
| 2026-11-24 | **懒加载翻转:init 只拉 roots,全量 `nodes/user/@me` 退役**:store `fetch()` 改 id-only,目录分页(100/页)+ 祖先链/相邻/扁平/统计/标签/元搜索走服务端;列表页 flat 上限 1000(总数诚实显示 `shown/total`),文件夹骨架上限 2000/role,服务端防环兜底;写后 `refreshDir` 回填可见页;MCP `list_children` 分页(limit/offset+total),activity 加 root/created 供团队洞察。详见 TODO 懒加载批次 |
| 2026-11-24 | **目录树吸纳 siyuan-auto-directory 替代概览目录表**:`node_refs` 内链索引(migration 0014,写时 reconcile + 回填,自链/资源/legacy 跳过)+ 双向 refs 查询(批量 100);目录树(嵌套懒载/临时排序/置顶/标签/时间/计数),拖拽 reorder(1024-gap order)/move-into 确认框(`directoryConfirmMove` 偏好),行多选批量删,行内引用 + referenced-by(去重已在下方子孙)。最近浏览表保留;Quick directory 浮层不做(与 CommandCenter 重叠)。详见 TODO 目录树批次 |
| 2026-11-18 | **代码文件 = 资源 + 代码块渲染 + MCP 直读**:上传门按**扩展名**(`utils::CODE_EXTS`,浏览器对代码文件 mime 不可靠);md 引用用带名路由 `/api/resources/{id}/file/{name}`(渲染器按扩展名分派),`html5MediaPlugin` 出 `.code-embed` 占位符、前端 hydrator 拉≤256 KiB UTF-8 文本渲染 hljs 代码块(复用 copyCodePlugin 复制/折叠),失败回退链接;MCP `read_resource` 工具(2 MiB + UTF-8 拒二进制)。前后端扩展名表互为镜像须同批改。思源 fork 只资产化不渲染,本批为对位补齐 |
| 2026-11-18 | **目录索引文件移除,v2 表格内联为容器概览**(index 文件 2026-11-13 实施,存活 5 天):点击 workspace/folder 直接展示 children + recent 两张表(`ViewSelection` 新 `overview` 模式,默认视图;表格/网格/高级/看板保留)。后端 ensure_index_child/锁定(403 ×3 + MCP -32003 ×3)/设置批量改名/`auto_index` 全删;migration `0013` drop `idx_nodes_index_child` + 存量 index 节点软删入回收站(共享 batch stamp 对齐 trash 语义)。**保留**:DQL `DIRECT`/`"id:<uuid>"`/`viewed_timestamp`、`node_views` 表(0012)、`record_view`、普通文档的 `children`/`recent` 块;`index` 块语法移除。设置项 `indexFileName`/`displayIndexFiles` 删除 |
| 2026-11-14 | **附件读加速 = 服务端盘缓存 + HTTP 缓存语义**(不做客户端):内容寻址 blob 落后端本地盘(`ATTACHMENT_CACHE_DIR`,LRU 限额),命中直接流式(含 Range);响应带 `Cache-Control: private, max-age=31536000, immutable` + `ETag "sha256-<sha>"` + `If-None-Match` → 304。放弃图片直连 S3(presigned),与「私有桶、读走后端鉴权」冲突 |
| 2026-11-14 | **附件 Cache-Control 修正:`immutable` → `no-cache`(审阅修复)**:附件 URL 稳定(`/api/resources/:id/file`,重传后 id 不变),长 `max-age`/`immutable` 使浏览器 HTTP cache 新鲜期内直接回旧字节不发请求,前端 SW 失效(Cache Storage)救不了底下 fetch 命中 HTTP cache → 重传后最长 1 年旧图。改 `private, no-cache` + 保留 ETag:SW 命中 0 网络,SW miss 一次 304(~13ms)。同批:sha 加 hex 校验(防路径穿越)、多 range 不透传 S3(忽略退全量)、启动清扫 crash 残留 tmp。avatar 维持不进盘缓存(URL 带 `?v=` busting,可变对象不适用) |
| 2026-11-12 | **附件读走后端代理,不再公开桶**(QuObjects 不支持 PutBucketPolicy/ACL,桶恒 Private):`GET /api/resources/:id/file`(owner/perm/公开门三层鉴权,Range 透传)+ `GET /api/users/:id/avatar`(公开)+ `/pergame/:uid/:file` 旧 embeds 兼容;前端直链 + `crossorigin="use-credentials"`,PDFViewer `withCredentials`;compose CDN 代理改指 backend。附带修 `public_descendants` 的 `ORDER BY "order"` 别名 bug |

## 环境变量命名约定

- 对象存储(backend 侧):`S3_ENDPOINT / S3_PUBLIC_URL / S3_SECURE / S3_INSECURE_TLS / S3_ACCESSKEY / S3_SECRETKEY / S3_BUCKET / S3_CA_PATH`。
- 对象存储(rustfs 服务侧):`RUSTFS_*(compose 内服务自身配置)`,backend 凭据经 `S3_ACCESSKEY/S3_SECRETKEY` 接 `${RUSTFS_ACCESS_KEY:-...}`。
- quirk(改名时语义原样保留):`S3_SECURE` 未设置时默认 **true**(仅显式非 `true` 值或 localhost/127.* 才走 HTTP);signer client 的 TLS 由 `S3_PUBLIC_URL` 的 scheme 决定,与 `S3_SECURE` 无关。

## 上游同步与提交

- commit:英文 Conventional Commits;公开仓,不 push 前先压缩(全局 §2.2)。
- 同步上游:`git fetch upstream` 后 rebase/merge;上游增量 migration 编号顺延,fork 侧不重排既有编号。上游增量追踪 = `.work/upstream-sync/TRACKING.md`(Go→Rust 重写后无法直接合并,逐 commit 分类:前端 cherry-pick 适配、Go 修复评估 Rust 对应实现、deps bump 不搬运)。
