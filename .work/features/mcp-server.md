# Feature Spec — MCP Server

> 状态:**已实现(2026-11-12)**,活环境验证见 `.work/TODO.md`。跟踪:issue #3。需求源:用户 2026-11-07。

## 需求

把 Pergame 笔记库暴露为 MCP server,AI agent(Claude/Cursor 等)可读写笔记。

## 已定决策

| 项 | 决策 |
|---|---|
| 实施时机 | 随 Rust 重写,后端内置模块(非独立进程) |
| 能力面 | **读写全量**(search/list/read + create/update/move/delete) |
| 传输 | streamable HTTP,端点 `/api/mcp`(axum 集成) |
| 鉴权 | 每用户 **API token**(设置页生成/吊销;`Authorization: Bearer`),不复用 session |
| SDK | 官方 Rust SDK `rmcp`(tokio 系,与 axum 同栈) |

## 工具集(2026-11-12 定稿)

- `search_notes(query, limit?)` — 全文搜索(pg FTS,role=3,owner 域,固定含 snippet)
- `list_children(folder_id?)` — 列子节点(空 = 根;owner + shared 合并;`{id,name,role,updated_timestamp,access}`)
- `read_note(id)` — 读 md 正文(`require_node_action` Read 放行即返 `content`,不限 role)
- `create_note(folder_id, name, content?)` — 建 document(role=3,`content_compiled=None`)
- `update_note(id, content?, name?)` — 改 content/name(非 owner 禁改归属字段,同 REST)
- `move_node(id, new_folder_id)`(根 = null) — 改 parent,复用 safe_parent 门
- `delete_node(id)` — 软删进回收站,响应 `{deleted_at, nodes}`

权限:MCP 调用 = 该 token 所属用户身份,复用现有 permissions/actor 体系(不是超权通道)。

## 实施定稿(2026-11-12 开工准备,证据见本节末)

| 项 | 定稿 |
|---|---|
| rmcp 版本/features | `rmcp 3.9.0`,features `transport-streamable-http-server`(拉起 `server`/`macros`/`schemars`/`server-side-http`/`tower`/`transport-worker`) |
| 服务类型 | `StreamableHttpService<PergameMcp, LocalSessionManager>`,factory 闭包 `move \|\| Ok(PergameMcp { state })`(AppState Clone,每 session 一份) |
| 挂载 | `main.rs` Router `.nest_service("/api/mcp", svc)` + bearer middleware(`from_fn_with_state`,simple_auth 例同款);**此分支 exclusive**:所有请求走 token,不认 cookie JWT——客户端发 cookie 忽略,不报错 |
| actor 注入 | middleware 验 token → 查 `api_tokens` → `req.extensions_mut().insert(McpActor{user_id, role, token_id})`;tool 内 `ctx.extensions.get::<http::request::Parts>()` 再取 `McpActor`(rmcp 官方注入路径,tower.rs `// inject request part to extensions`) |
| token 形态 | 不透明随机串 `pgm_` + base64url(32B),**库中只存 `sha256_hex`(对应 `token_hash`)**;明文仅生成时返回一次;格式校验失败 → 查库前直接 401(与 JWT `"Bad access token."` 同文案) |
| token 管理端点 | `GET/POST /api/users/@me/api-tokens`(列表脱敏/创建返明文),`DELETE /api/users/@me/api-tokens/{id}`;走 cookie `AuthUser`,**不受 MCP 鉴权**;创建/吊销 + 每次 MCP 工具写操作记 `logs`(`mcp.token.*` / `mcp.tool.*`) |
| 锁语义 | 复用全局锁,**`/api/mcp` 加入 `locks::is_exempt`**(实现时修正:middleware 只按 HTTP method 判读,而 MCP 全部是 POST,不豁免会把只读工具也挡掉)→ 守卫下沉到服务内:`create/update/move/delete` 四工具先调 `locks::global().is_read_only()` → MCP 错误 `-32023` + `data.reason=backup_in_progress`(与 middleware 同语义,不同载体:JSON-RPC error 而非 HTTP 423);三读工具直通 |
| delete 语义 | `delete_node` = 软删进回收站(`soft_delete_subtree`,与 REST DELETE 同唯一写路径),无二次确认参数——可恢复性即安全网,响应回 `deleted_at` + `nodes` 计数 |
| move 语义 | 复用 `update_node` 的 safe_parent 门(目标父需 Update 权限否则落根);非 owner **显式拒绝**(-32003,而非 REST 的静默保留旧父——显式请求被静默忽略对 agent 更糟);**加环校验**(实现时补):目标父 == 自身或 ∈ `subtree_ids(自身)` → -32602。REST 无此校验,因为它只改字段;MCP 暴露了「移动」这一显式动作给 LLM,静默造出不可达子树不值得省这 3 行 |
| search 范围 | 复用 `repo::search`(仅 role=3 + `deleted_at IS NULL`,owner 域);`include_content=true` 固定(给 snippet) |
| read 范围 | `require_node_action(..., Read)` 放行即读,`content` 原样返(含 role 1/2/4,不限 document);不存在/无权限 → JSON-RPC -32002(404)/-32003(403)(见下映射) |
| list_children | `parent_id` 为空 = 根;owner 域 + `list_shared` 合并(REST `get_user_nodes` + `get_shared_nodes` 同款),每项 `{id,name,role,updated_timestamp,access}`。**不逐项 guard**:两个来源本身已按可见性过滤(owned 树 + 被授权的树),逐项 `check_node_action` 只会变成 N+1;给定 `folder_id` 时先对父做一次 Read guard |
| create 默认 | `role=3 document`, `content_compiled=None`(REST 由前端产 HTML;MCP 无渲染器,置空让编辑器下次打开重编译);name 必填空则 -32602。`update_note` 改 content 时同样把 `content_compiled` 置 None——否则会渲染旧文本直到手动重开 |
| 错误映射 | `ApiError::BadRequest→-32602`,`Unauthorized→-32001`,`Forbidden→-32003`,`NotFound→-32002`,`Conflict→-32009`,锁 423→`-32023`(message 含 `reason: backup_in_progress`),`Internal→-32603`;message 原样透传 ApiError 文案 |
| 速率限制 | **P1 不做**(单用户/家庭实例,token 即能力;滥用面 = token 泄露 → 靠吊销);审计靠 logs 行 |
| 前端 | settings 新 `_views/api-tokens.vue`(`?p=apiTokens`,Security 邻位):生成(命名+复制一次)/列表(last_used/created)/吊销;复用 `snapshot-targets.vue` 的掩码 + 复制交互 |
| migration | `0007_api_tokens.sql`(顺延当前最大 0006):`api_tokens(id UUIDv7 PK, user_id FK CASCADE, name TEXT NOT NULL, token_hash TEXT UNIQUE NOT NULL, created_timestamp, last_used_timestamp NULL)` + `idx_api_tokens_user`(UNIQUE 已自带 hash 索引,不另建);last_used 每次鉴权成功 UPDATE(与 sessions.last_refresh 同策略) |
| 测试 | 单测:token 形态与哈希稳定性/错误码映射表/bearer 前缀解析;集成(`tests/integration.rs` 新用例,串行套件):token 生命周期 + 七工具全链 + 越权 403 + 吊销后 401 |

证据链(2026-11-12 实测):

- rmcp `examples/servers/src/counter_streamhttp.rs`:`StreamableHttpService::new(factory, LocalSessionManager, config)` + `nest_service("/mcp", service)` 即 axum 集成正统形态。
- rmcp `examples/servers/src/simple_auth_streamhttp.rs`:bearer middleware + `nest_service` 同款组合,直接对标本 spec 鉴权决策。
- rmcp `tower.rs:2216-2222` + `1450-1480` 文档:transport 把 `http::request::Parts` 注入 `RequestContext.extensions`,axum `Extension`/middleware 塞的 state 经 `parts.extensions` 可取——actor 注入有官方路径,无需 patch rmcp。
- `cargo add rmcp@3.9.0 --features transport-streamable-http-server --dry-run` 解析干净(11 features,无版本冲突)。
- `locks.rs:164-173`:middleware 只放行 GET/HEAD/OPTIONS,而 MCP 全走 POST → 必须把 `/api/mcp` 加入豁免表,锁门下沉到四个写工具(见「锁语义」行)。
- `routes/nodes.rs:367-382`:REST DELETE 已是软删唯一写路径,MCP 跟随即零新语义。

## 开放问题(2026-11-12 已裁决,见上表)

- ~~token 存储:users 表新列 vs 独立 `api_tokens` 表~~ → **独立表**(多 token + 吊销 + last_used),migration 0007。
- ~~速率限制/审计日志~~ → **速率限制不做;审计记 `logs` 表**(`mcp.token.*` 创建/吊销、`mcp.tool.*` 四写操作)。
- ~~delete 是否要求二次确认参数~~ → **不要**;软删可恢复,响应回计数即确认。

