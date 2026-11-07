# TODO

## 数据缺口/待办

- [x] **S3_* 重命名落地**(2026-11-07):Go 读点 9 处、compose、workflow、ecosystem、.env.example ×2、docs S3 节、frontend 内嵌文档副本。已验证:go build/vet 过;`grep -rn "MINIO_"` 全仓零命中(i18n `landing.ts` 装饰性容器名、Makefile minio 二进制 env、AGENTS.md 决策记录除外);compose YAML 解析 + backend env S3 键断言过。本机无 docker runtime,`docker compose config`/up smoke 待人工(见下)。
- [x] **Rust 后端技术栈调研**(2026-11-07):`.work/rust-migration/RESEARCH.md`;结论 = go(附 P0 前置条件),待回填 issue #1。
- [ ] **Rust 后端重写 + PostgreSQL**(go 已裁决 2026-11-07):issue #1 跟踪;**db 层 = Toasty + sqlx 逃逸层**(练手项目直接上前沿,P1 spike 验证不过则回退 sqlx 为主);全新空库,无数据迁移;pg GIN + tsvector;P0 前置条件见报告 §8。
- [ ] **MCP server**(随重写):issue #3;读写全量工具、streamable HTTP `/api/mcp`、rmcp、每用户 API token;spec = `.work/features/mcp-server.md`。
- [ ] **目录索引文件**(随重写):issue #4;新建目录自动建 index、设置管文件名(批量改名)/可见性、永远置顶(order=-1 双端强制);**index 块 = #5 dataview 预设语法糖**;spec = `.work/features/directory-index.md`。
- [ ] **Dataview 查询引擎**(随重写):issue #5;DQL 子集(LIST/TABLE+FROM/WHERE/SORT/LIMIT)、内置字段+metadata JSONB、后端 `POST /api/nodes/query` 参数化 SQL、前端容器块渲染;spec = `.work/features/dataview.md`。

## 待人工验证

- [ ] 自托管 smoke(本机无 docker):`docker compose up --build` 后端正常启动(`S3_*` 链路)、注册/登录可用。
- [ ] GitHub repo secrets:`MINIO_ACCESSKEY`/`MINIO_SECRETKEY` 手动改名 `S3_ACCESSKEY`/`S3_SECRETKEY`(deploy_to_prod.yml 引用已改)。
- [ ] issue #1 回填调研结论(go + RESEARCH.md 链接)。

## 维护说明

- 项目约定与决策记录 → 根 `AGENTS.md`。
- 长程任务状态 → `.work/alexandrie-infra/GOAL-STATE.md`。
- 教训沉淀 → `.work/LESSONS.md`。
