# TODO

## 数据缺口/待办

- [x] **S3_* 重命名落地**(2026-11-07):Go 读点 9 处、compose、workflow、ecosystem、.env.example ×2、docs S3 节、frontend 内嵌文档副本。已验证:go build/vet 过;`grep -rn "MINIO_"` 全仓零命中(i18n `landing.ts` 装饰性容器名、Makefile minio 二进制 env、AGENTS.md 决策记录除外);compose YAML 解析 + backend env S3 键断言过。本机无 docker runtime,`docker compose config`/up smoke 待人工(见下)。
- [x] **Rust 后端技术栈调研**(2026-11-07):`.work/rust-migration/RESEARCH.md`;结论 = go(附 P0 前置条件),待回填 issue #1。
- [ ] **Rust 后端重写 + PostgreSQL**(go 已裁决 2026-11-07):issue #1 跟踪;**db 层 = Toasty + sqlx 逃逸层**(练手项目直接上前沿,P1 spike 验证不过则回退 sqlx 为主);全新空库,无数据迁移;pg GIN + tsvector;P0 前置条件见报告 §8。

## 待人工验证

- [ ] 自托管 smoke(本机无 docker):`docker compose up --build` 后端正常启动(`S3_*` 链路)、注册/登录可用。
- [ ] GitHub repo secrets:`MINIO_ACCESSKEY`/`MINIO_SECRETKEY` 手动改名 `S3_ACCESSKEY`/`S3_SECRETKEY`(deploy_to_prod.yml 引用已改)。
- [ ] issue #1 回填调研结论(go + RESEARCH.md 链接)。

## 维护说明

- 项目约定与决策记录 → 根 `AGENTS.md`。
- 长程任务状态 → `.work/alexandrie-infra/GOAL-STATE.md`。
- 教训沉淀 → `.work/LESSONS.md`。
