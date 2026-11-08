# AGENTS.md — Alexandrie (fork) 工作区约定

> 面向 agent 的项目约定。全局约束见 `~/.zcode/AGENTS.md`;任务状态唯一事实源 = `.work/TODO.md` 与 `.work/alexandrie-infra/GOAL-STATE.md`。

## 项目概览

Alexandrie = 自托管知识库 + 扩展 Markdown 编辑器(Notion/Obsidian 替代品)。

- **本仓为 fork**:origin = `kamasylvia/Alexandrie`(公开),upstream = `Smaug6739/Alexandrie`。fork 自定义改动尽量保持可合并上游的形态(migration 编号不重排、不无谓重构上游文件)。
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
| 2026-11-08 | **重写后应用定名 Pergamon**(帕加蒙,古代第二图书馆;撞名已查仅 Typst 文献包)。改名落地范围(仓库/品牌/env/容器名)未定,实施时另立批次——env 变量与 compose 容器名属破坏性变更 |
| 2026-11-08 | **DR 两级定位**:实例级 = `pg_dump + S3 桶异地副本 + secrets` 三件套;应用内 Backup v2 zip **砍除**(issue #9 关闭,spec 存档 `features/backup-v2.md`)。多端备份 = 运维件 `scripts/dr-backup.sh`(N pg 源 × M rclone 目的地,retention/校验/dry-run);将来开放多用户或需单用户跨实例搬家时 v2 再立项 |
| 2026-11-07 | **对象存储环境变量 `MINIO_*` → `S3_*`**(存储中立命名);Go 读点与 compose/docs/workflow 同批改名 |
| 2026-11-07 | **compose 的 backend 服务本地构建**(`build: ./backend`):上游预构建镜像 `ghcr.io/smaug6739/alexandrie-backend` 不认 `S3_*`、且后续不支持 pg |
| 2026-11-07 | `.work/` 目录进 git(TODO/LESSONS/调研产物,公开仓,无敏感内容) |

## 环境变量命名约定

- 对象存储(backend 侧):`S3_ENDPOINT / S3_PUBLIC_URL / S3_SECURE / S3_INSECURE_TLS / S3_ACCESSKEY / S3_SECRETKEY / S3_BUCKET / S3_CA_PATH`。
- 对象存储(rustfs 服务侧):`RUSTFS_*(compose 内服务自身配置)`,backend 凭据经 `S3_ACCESSKEY/S3_SECRETKEY` 接 `${RUSTFS_ACCESS_KEY:-...}`。
- quirk(改名时语义原样保留):`S3_SECURE` 未设置时默认 **true**(仅显式非 `true` 值或 localhost/127.* 才走 HTTP);signer client 的 TLS 由 `S3_PUBLIC_URL` 的 scheme 决定,与 `S3_SECURE` 无关。

## 上游同步与提交

- commit:英文 Conventional Commits;公开仓,不 push 前先压缩(全局 §2.2)。
- 同步上游:`git fetch upstream` 后 rebase/merge;上游增量 migration 编号顺延,fork 侧不重排既有编号。
