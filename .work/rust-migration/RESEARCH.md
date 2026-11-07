# Rust 后端技术栈调研 — Alexandrie

> 日期:2026-11-07。结论回填 issue [kamasylvia/Alexandrie#1](https://github.com/kamasylvia/Alexandrie/issues/1)。
> 范围:评估将 Go backend(gin + sqlx)重写为 Rust 的技术选型、parity 面、工作量与风险。**本报告只调研不实施**;go/no-go 由用户裁决。

## 1. 现状盘点(Go backend)

| 维度 | 现状 |
|---|---|
| 规模 | ~8,500 行 Go(不含 tests);52 条 HTTP 路由(`backend/router/routes/*.go`,6 个 route 文件) |
| 分层 | controllers(11)→ services(13)→ repositories(10,全部 SQL 在此);middleware:authentification / admin |
| Web 框架 | gin;`GIN_MODE`;CORS(gin-contrib/cors) |
| DB | sqlx + go-sql-driver/mysql;golang-migrate(embed SQL,启动自动 `m.Up()`);21 个 migration |
| ID | 自研 Snowflake(`pkg/snowflake`,types.Snowflake) |
| 认证 | JWT + cookie(COOKIE_DOMAIN/ALLOW_UNSECURE)、session 表、TOTP(pquerna/otp)、OIDC(自研 oidc/ + config)、backup codes、admin accounts 环境变量 |
| 对象存储 | minio-go v7(双 client:internal + presign signer);bucket 自举 + 公共读 policy + backup 生命周期;S3_* env(2026-11-07 起) |
| 邮件 | wneessen/go-mail + 2 个 HTML 模板(注册/重置密码) |
| 备份 | 用户级 zip(JSON 数据 + 对象存储文件流)→ 上传 `S3_BUCKET-backups`,presigned 下载,24h 生命周期 |
| 权限 | 自研 permissions 包(node 级 RBAC,role/accessibility) |
| 配置 | config.toml(go:embed,CONFIG_PATH 覆盖)+ 环境变量 |
| 测试 | `backend/tests/`:纯 HTTP 黑盒集成测试(打 localhost:8201),不 import DB 层 → **对后端语言中立,重写后可直接复用做回归** |

**关键观察**:
- API 面 = 前端契约(Nuxt 前端 + PWA)。重写**不动 frontend**,Rust 后端必须逐字节兼容 JSON 形状、路由、cookie 语义。
- `types.Snowflake` 的 JSON 序列化行为(字符串 vs 数字)必须先核实并在 Rust 侧保持一致——JS 侧 i64 精度敏感。**待核(重写第一批核实)**。
- SQL 全部集中在 repositories(~1,450 行)+ migrations → 方言迁移面收敛,利好重写。
- 集成测试语言中立 → 是重写回归的安全网(现覆盖 auth/user lifecycle/admin;建议重写前先补 nodes/resources 路由的用例)。

## 2. Web 框架:axum vs actix-web

| 维度 | axum | actix-web |
|---|---|---|
| 维护 | tokio 官方团队,生态顺位最高 | 独立社区,活跃但独立于 tokio 核心 |
| 中间件 | tower 标准(tower-http:CORS/trace/timeout 现成) | 自有 actor 模型中间件体系 |
| 性能 | 略逊(多线程 tokio worker) | 最快梯队(worker 单线程 runtime) |
| 社区共识(2025-2026) | **默认首选**,"team will actually maintain six months later" | 极限吞吐场景选它 |

**建议:axum**。Alexandrie 是自托管知识库,吞吐非瓶颈;tower/tokio 生态与维护性优先。tower-http 直接覆盖现有 gin 中间件(CORS、logger)。

## 3. 数据库层(PostgreSQL)

| 项 | 建议 | 说明 |
|---|---|---|
| 查询工具 | **Toasty**(2026-11-07 用户裁决:练手项目,直接上前沿) | tokio-rs 同门 ORM;模型即 struct,CRUD/关系走 ORM。**原评估为 sqlx(见下)**,权衡已记录:0.x 未达 1.0、复杂 SQL 需逃逸——用户知情后拍板 |
| raw SQL 逃逸层 | **sqlx 保留为依赖** | FTS(tsvector/ts_rank/ts_headline)、权限 CASE WHEN 排序、动态 IN、月度聚合 to_char、ON CONFLICT upsert 走 sqlx raw SQL;Toasty 管 CRUD/关系。**P0 spike 验证**:Toasty 0.x 能否接受「sqlx migrate 手管 schema」模式(模型映射到既有表,不强制自带迁移) |
| Migration | **sqlx migrate**(schema 主权不交给 Toasty 0.x) | embed + 启动自动跑,语义与 golang-migrate 等价;Toasty 迁移工具成熟后可再议交接 |
| 首次 schema | **全新 000001_init(pg 方言),不照搬 21 个 mysql migration 链** | 已定决策:全新空库无数据迁移;mysql 链中的历史弯路(改 bigint、加索引等)一次到位;上游后续 migration 再手工对齐 |
| 首次 schema | **全新 000001_init(pg 方言),不照搬 21 个 mysql migration 链** | 已定决策:全新空库无数据迁移;mysql 链中的历史弯路(改 bigint、加索引等)一次到位;上游后续 migration 再手工对齐 |
| 全文搜索 | `to_tsvector('simple', ...)` STORED 生成列 + GIN 索引;`websearch_to_tsquery` + `ts_rank` 排序;`ts_headline` 出 snippet(替代 mysql 的 LOCATE/SUBSTRING 截断,质量更好) | `'simple'` 配置对多语言(含 CJK)行为与 mysql FULLTEXT 相当(都不分词 CJK),不引入额外依赖 |
| pg 驱动 | sqlx 内置 postgres(tokio-postgres 系) | 无需额外驱动 |

## 4. 对象存储(S3)

| 项 | 建议 | 说明 |
|---|---|---|
| SDK | **aws-sdk-s3**(官方,支持自定义 endpoint + path-style,覆盖 RustFS/MinIO/Garage) | 与 minio-go 能力对齐:put/get/list/remove、presign(签名 URL)、bucket policy、lifecycle |
| 备选 | object_store(apache-arrow 系) | 更偏分析型负载;presign 与 policy 定制不如 aws-sdk-s3 直接 |
| 语义保留点 | 双 client(internal + public signer)、`S3_SECURE` 默认 true quirk、localhost 降级 HTTP、`S3_CA_PATH` 自定义 CA(reqwesttls 根证书注入)、`S3_INSECURE_TLS`(danger_accept_invalid_certs)、backup bucket 24h lifecycle | 逐条对照 `backend/app/minio.go` 重写 |

## 5. Parity 清单(逐模块)

| Go 现件 | Rust 对应 | 难度 | 备注 |
|---|---|---|---|
| router/routes/*.go(52 路由) | axum Router | 低 | 机械翻译;路径/方法/状态码逐条对齐 |
| middlewares/{authentification,admin}.go | axum middleware/extractor | 低 | JWT cookie 解析 + admin 校验 |
| controllers ×11 | handlers 模块 | 低-中 | 参数校验:gin+validator → validator crate 或手写 |
| services ×13 | service 层 | 中 | 业务逻辑 1:1;backup zip 并发流(files 并发下载打包)注意 tokio 语义 |
| repositories ×10(~1.45k 行 SQL) | Toasty 模型(CRUD)+ sqlx raw SQL(pg 方言,FTS/聚合/复杂排序) | 中 | `?`→`$n` 顺手;ON DUPLICATE KEY→ON CONFLICT;NOW 语义无(Go 侧生成时间戳,不变) |
| app/migrations(21 mysql 文件) | 全新 pg init migration | 中 | 见 §3 |
| app/minio.go + minio/resource/backup service 的 S3 面 | aws-sdk-s3 封装 | 中 | §4 语义保留点逐条过 |
| pkg/snowflake + types.Snowflake | snowflake crate 或移植;**序列化行为对齐** | 低 | 待核 JSON 形状 |
| permissions 包 | 移植(纯逻辑) | 低-中 | 算法型代码,单测可平移 |
| oidc/(discovery + flow)+ oidc_provider repo | openidconnect crate 或手写 | 中-高 | 多 provider 配置(OIDC_N_*)、session 关联;Go 版自研语义需逐条对 |
| auth(JWT/session/TOTP/backup codes) | jsonwebtoken + totp-rs + argon2/bcrypt(与 Go 现哈希**兼容**或强制重置) | 中-高 | **密码哈希算法兼容性是硬点**:换算法 = 现有用户全量重置;调研 go 侧实际算法(重写时核 config/模型) |
| emails/(go-mail + 2 模板) | lettre + 同模板 | 低 | |
| stats(月度聚合 SQL) | pg to_char 聚合 | 低 | |
| config.toml embed + env 覆盖 | rust-embed/config + figment 或手写 | 低 | 保留 CONFIG_PATH 行为 |

## 6. 工作量估算(阶段拆分)

单人 + agent 辅助,粗估:

| 阶段 | 内容 | 估时 |
|---|---|---|
| P0 | 决策前置:Snowflake JSON 序列化核实、密码哈希算法核实、集成测试补齐(nodes/resources/oidc 用例) | 0.5-1 天 |
| P1 | 骨架:axum + 配置 + DB 连接(sqlx migrate + Toasty 模型 spike)+ snowflake + logger + 健康路由;docker compose 切 pg | 1-2 天 |
| P2 | 认证域:auth/session/TOTP/OIDC/admin(users、auth、oidc 路由) | 3-4 天 |
| P3 | 内容域:nodes/permissions/resources/S3(CDN 上传、搜索、回收站) | 3-4 天 |
| P4 | 外围:stats/user_settings/backup zip/emails | 2-3 天 |
| P5 | 回归:集成测试全绿 + 双后端对照 diff(API 响应逐字段)+ docs 更新 + compose/CI 收口 | 2-3 天 |
| 合计 | | **12-17 天** |

策略:按域切换(非 big-bang),每阶段结束跑集成测试 + 前端手测该域;P2-P4 期间 Go 版保持可运行作为对照(同库双跑只读安全)。

## 7. 风险表

| 风险 | 影响 | 缓解 |
|---|---|---|
| API 契约漂移(JSON 字段名/形状/空值语义) | 前端隐性坏 | P5 双后端逐字段 diff;集成测试先行补齐 |
| Snowflake 序列化不一致 | ID 精度错乱 | P0 核实;Rust 侧自定义 Serialize |
| 密码哈希不兼容 | 现有用户无法登录 | P0 核实算法;不兼容则保留旧算法验证层(仅校验)或通知重置 |
| pg 全文搜索行为差异(相关度排序) | 搜索体验变化 | 'simple' + ts_rank 调参;验收用真实语料对比 |
| Toasty 0.x breaking(API/驱动) | 重写周期内依赖漂移 | 固定版本(`=0.x.y`);DAO 收敛在 repo 层,breaking 只波及该层;P1 spike 先行验证 schema 映射与 escape hatch,不通过则回退 sqlx 为主(决策点在 P1 末) |
| 自托管用户迁移成本(compose 变更 + 空库) | 丢数据预期管理 | issue #1 已声明 fresh DB;README 突出 BREAKING |
| OIDC 边缘 case(issuer discovery、key rotation) | SSO 登录坏 | 对照 Go 版手测各 provider;保留 OIDC_N 配置形状 |
| 上游同步断裂(backend 全换) | 吃不到上游后端修复 | 接受(fork 已定此路线);前端/docs 仍可跟上游 |

## 8. 结论与前置条件

**结论(2026-11-07 用户裁决):go,db 层用 Toasty + sqlx 逃逸层**。项目定位为练手,接受前沿依赖的成熟度风险;原 sqlx-only 建议(§3)及理由保留作历史记录。其余理由:
1. 重写窗口与 pg 切换合一,避免 Go 层白做 mysql 方言治理;
2. 后端规模收敛(8.5k 行、SQL 集中、52 路由),12-17 天量级可控;
3. Rust + pg + sqlx 组合对该形态(自托管单二进制、内存占用低)是长期收益;axum 为当前社区默认首选。

**前置条件(P0,2026-11-08 完成)**:

- [x] **`types.Snowflake` JSON 序列化 = 字符串**(`MarshalJSON` → `strconv.FormatUint`);Unmarshal 宽松(去引号 ParseUint,数字亦可)。→ Rust:`Serialize` 为 string,`Deserialize` 兼容 string+number。注:既有 `user_lifecycle_test.go` 曾强断言 `json.Number`(bigint 迁移前的旧断言,已过期必红),随 P0 修正。
- [x] **密码哈希 = bcrypt DefaultCost(10)**(user.service/auth.service;**backup codes 亦 bcrypt**)。→ Rust `bcrypt` crate 可直接校验 `$2a$/$2b$` 哈希,**现有用户零迁移**;Rust 侧新哈希维持 cost 10。
- [x] **集成测试补齐**:`tests/nodes_test.go`(doc 生命周期+search+未授权 401)、`tests/permissions_test.go`(读→写升级/撤销/邀请 join/非法输入)、`resources_test.go`(multipart 上传/更新/头像/删除);helpers 增 `NewClient/DoPut/DoPostMultipart/SignupAndLogin(唯一用户名,可重跑)/ResultIDString`。**编译过(`go test -c`)+ vet 过;运行时验证待人工**(需 mysql + minio + 活后端,本机均无)。
- [x] 用户 go/no-go 确认(2026-11-07 go,ORM = Toasty;回填 issue #1)
- [x] **P1 Toasty spike:PASS(2026-11-08,commit 4993e160)**——`#[derive(Model)]` + `#[table]/#[column]` 映射手管 schema;`#[key]` 无 `#[auto]` = 应用侧 Snowflake(i64 边界);`#[unique]` 生成 filter_by_/upsert_by_;`Builder::build()` 源码审查 = 零 DDL(仅能力校验 + 试连接),`push_schema` 不调用即共存;依赖摩擦一处已解(js-sys --precise 0.3.85);CRUD 生成面编译门过。**回退 sqlx 为主未触发**。运行时正确性(生成 SQL vs 既有表)= P2 首个活 pg 检查点

## 附:生态参考

- [Encore: Axum vs Actix Web vs Rocket in 2026](https://encore.dev/articles/axum-vs-actix-vs-rocket)
- [rustify.rs: Axum vs Actix-web 2026](https://rustify.rs/articles/rust-axum-vs-actix-web-2026)
- [r/rust: actix-web vs axum in 2025-2026](https://www.reddit.com/r/rust/comments/1ozt50s/actixweb_vs_axum_in_20252026)
- [sqlx 0.8(crates.io)](https://crates.io/crates/sqlx/0.8.1) — compile-time checked queries + 内置 migrate
- [rust-db/refinery](https://github.com/rust-db/refinery) — 备选 migration 工具
