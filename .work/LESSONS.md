# LESSONS

(空——新坑在此沉淀;格式:日期 + 现象 + 根因 + 规则)

## 2026-11-09 改名批次(macOS M2 机首跑)

- **Go 依赖拉取被墙**:本机(M2)`go build/vet` 拉 module 超时 → 单次 `GOPROXY=https://goproxy.cn,direct` 环境变量解决,不改全局 go env。
- **rustc 版本差**:backend-rs 依赖 toasty 0.11 需 rustc ≥1.95;本机 default stable=1.94.1(1.95.0 已装)→ `rustup override set 1.95.0`(backend-rs 目录,machine-local ~/.rustup settings,不进仓)。换机首跑 backend-rs 编译失败先查 rustc 版本。
- **bun install 重写 lockfile**:换机 bun 版本不同 → `bun install` 把 bun.lock 重写数千行(格式 churn)。改名/小改动批次禁收编此 churn:改完立即 `git checkout -- bun.lock`,只手工同步必要 token(如 package name 行);前端装依赖用 `bun install --registry https://registry.npmmirror.com`。
- **exFAT 卷 AppleDouble 垃圾**:仓库在外置 exFAT 卷,工具运行会生成 `.!NNNN!xxx` 前缀临时文件,git status 出现未跟踪垃圾 → 删除不入库。
- **BSD sed 多字节崩溃**:macOS sed 批量改中文文件报 `illegal byte sequence` → 一律 `LC_ALL=C sed -i ''`(纯 ASCII 替换模式对 UTF-8 安全)。
- **既有缺口(上游遗传,非改名引入)**:`backend/tests/helpers.go:227` 引用 `LoginAs`,而其定义在 `tests/auth_test.go`(测试编译单元才可见)→ `go build ./...` 在 tests 包失败;`go vet ./...` 与 `go test` 不受影响。待顺手修。

## 2026-11-09 OIDC 活测

- **Keycloak admin service account 权限边界**:master realm 的 client_credentials service account 可建/删 realm(201/204)但**不能**管 realm 内对象(client CRUD 403)。绕法 = **完整 realm import**:POST /admin/realms 一份含 clients[](secret/redirectUris)+ users[](credentials[])的 RealmRepresentation,一次带入全部对象,免逐对象管理权。
- **sqlx FromRow 列映射坑**:`#[derive(FromRow)]` 按字段名匹配列;`#[serde(rename)]` 只管序列化**不影响** sqlx。字段名≠列名(如 session_type←`type`)必须在字段上加 `#[sqlx(rename = "type")]`,否则运行期 decode 报错→500。`#[sqlx(default)]` 会把列缺失静默成 None(丢数据),partial SELECT 必须配别名(如 `"order" AS node_order`)而非靠 default 兜。
- **requests 会话不发 localhost host-only cookie**(domain 归一成 localhost.local 不匹配)→ 程序化测 cookie 鉴权用 curl 显式 `-H "Cookie: ..."` 或改用 127.0.0.1 别名,非后端 bug。
- **backend-rs 启动即自举 S3**:建桶 + pergame 设公共读 policy + pergame-backups 设 24h lifecycle(删桶后重跑会自动重建,幂等)。

## 2026-11-09 备份子系统批次

- **静默 replace 是最大返工源**:本批多处 `str.replace(old,new)` 因锚点不匹配而空转(registered 打印成功但实际未改),导致「明明改了却仍是旧行为」的连环排查(list_snapshots、download_snapshot、main.rs 路由注册)。**规则:所有批量文本替换必须带 `assert old in s`,改后立刻用 `grep -c` 验证目标串出现次数**。
- **两套写入路径必须共用校验**:v2 快照改为 manifest+blobs 后,`restore_from_zip` 引擎仍需 `manifest.json` 成员(格式门),组装时必须补上;新格式上线同时要放行 `MAX_FORMAT`。教训:改了存储格式先跑一遍「读回」路径,不要只验写入。
- **JSON 手写易错**:shell 侧用 sed/awk 生成 JSON 时必须转义 `\n \r \t " \\` 并删控制字符;漏掉换行会让 API 侧 500(错误信息「control character found while parsing」)。无 python 的镜像是常态,写 escape 函数后要有独立冒烟。
- **掩码写入的经典陷阱**:读时掩码 `***`、写时回填旧值——字符串重组的切片边界(`&line[q2..]` vs `[q2+1..]`)错了会每次 round-trip 多一个引号,而宽容的解析器不报错。**掩码往返必须有 assert 等值测试**(`unmask_content(masked) == original`)。
- **DR/备份类权限面**:全库还原能力只给 role=2 admin,且要在前端做二次确认;conf 里目的地密钥经 admin API 可写但读永远掩码。
- **本机无 pg_dump/rclone**:DR sidecar 端到端只能验到「job 协议 + 脚本被调用 + 错误回传」,真实 dump 必须在有 docker 的机器上验;`postgres:17-alpine` 做 sidecar 基础镜像可保证 pg_dump 与库版本匹配。

## 2026-11-09 回收站批次

- **软删必须切断「删除即清 S3」的耦合**:上游 `DeleteNode` 是「捞 role=4 资源 → 删 MinIO 对象 → DB 级联物理删」一条龙。改软删时若保留 S3 清理段,恢复回来的节点资源全指向已删对象。**规则:资源清理只允许存在于 purge/empty 路径**。
- **级联软删要用同一个时间戳做批次键**:`soft_delete_subtree` 给整棵子树写同一个 `deleted_at`,恢复时按该值取子集;否则父子批次无法区分,会出现「恢复了子节点但父节点还在回收站」的悬挂。恢复时若父节点仍在删除态(`deleted_at <> 本批`)须上提为根级。
- **回收站列表只列子树根**:条件 = 已删且「父节点不存在或父 `deleted_at` 不同」。少这一步会让删掉一个目录后列表出现整棵子树几十行。
- **配额统计故意不排除已删节点**:`uploads_size` 保留已删行(文件仍占 S3),否则用户可以「删除 → 绕过总量配额」。这是有意的例外,别在后续清理时"顺手"加上 `deleted_at IS NULL`。
- **i18n 8 个 locale 都被 `satisfies MessageSchema` 强约束**:新增翻译 key 必须同时补齐 en/de/fr/it/ko/uk/zh-CN/zh-TW 全部 8 份,漏一个就 typecheck 挂。生成脚本批量写 + 逐个接 index.ts 是可行路径(改后 `grep -c trash` 验)。
- **perfectionist/sort-objects 会拦 locale 文件**:`.ts` 语言包也在 eslint 管辖内,手写对象字面量天然不满足字母序 → 写完后必须跑 `bun run lint --fix`,否则 CI 红。
- **`definePageMeta({layout})` 只能引用真实 layout**:本仓 `frontend/app/layouts/` 只有 `public.vue`,dashboard 页走默认布局;写 `layout: 'dashboard'` 直接 typecheck 报错。

## 2026-11-09 DR 配置表单化

- **锚点缩进猜错 = 静默空转**(重演):批量插 key 的 `str.replace('    instanceDR: {\n', ...)` 因真实缩进是 2 空格而没匹配,脚本却因为 `assert 'instanceDR: {' in s` 通过了——**assert 检查的是「存在」不是「替换成功」**。必须改成 `assert replace_anchor in s` 或替换后 `assert s != before`。
- **Nuxt `I18nKey` 是类型生成物**:新 i18n key 写完必须 `bun run postinstall`(`nuxt prepare`)重生成 `.nuxt` 类型,否则 typecheck 报满屏「not assignable to I18nKey」而误判成代码错。
- **模板/脚本变量名不一致 typecheck 能抓**:`configExists`(模板) vs `confExists`(脚本)导致 `Property does not exist on ComponentPublicInstance`;这类错看着像组件类型问题,实为拼写。

## 2026-11-09 DR 表单活测(浏览器实测)

- **`backend-rs/.env` 是 dotenvy 的隐形配置源**:CWD 下 .env 会喂入旧值(collectanea 时代残留),显式 env 只覆盖已 export 的键——漏传 `DATABASE_NAME` 就静默用了 .env 的死库名。换机/换环境先查 `backend-rs/.env` 是否过时(已 2026-11-09 更新为 pergame 名 + Infisical 凭据)。
- **活环境提权的安全路径**:`ADMIN_ACCOUNTS` 启动时会降级所有不在名单内的 admin——对活库先 SQL 查现有 admin id,或确认库内无 admin 后再注册+直接 SQL UPDATE role=2,避免破坏真实实例。
- **Playwright checkbox 状态看 `el.checked` 属性而非 `getAttribute('checked')`**:Vue 绑定下 attribute 可能不存在但 property 为 true;uncheck 前先 evaluate 读真值。
- **前端 dev 联调最小配置**:`NUXT_PUBLIC_BASE_API=http://localhost:8201` + 后端 `FRONTEND_URL=http://localhost:3000`(CORS 白名单)+ `DR_CONF_DIR/DR_JOBS_DIR` 指向本地临时目录即可让 DR 配置页完全活起来,无需 docker sidecar。

## 2026-11-09 DR 对位批次 D(1-5 全补)

- **rclone ≥ 1.76 不再从 `RCLONE_CONFIG_<name>_*` env 段解析 remote type**(`config show` 见 section 但 NewFs 报 "didn't find section in config file")——env-section 是老教程广传的模式,新版全静默失效。**修法 = 运行时生成私有 config 文件 + `export RCLONE_CONFIG=<file>`,全版本稳定**。这条直接判了原 dr-backup.sh 真机全链死刑——dry-run 从不碰 rclone,探测按钮首跑才暴露。教训:**凡是「只在真机才触发的路径」,上真机前必须有最小真值冒烟**(哪怕是假 key 打真端点)。
- **bash `read` 尾变量吞行**:`IFS='|' read -r _ name <<<"$rec"` 两变量版会把记录剩余全部塞进 `name`(read 给最后变量分配整行余量)。瘦身老解析函数时漏了占位 `_rest` → 远端名变成整条记录。规则:**read 变量数必须 ≥ 字段数,或末位显式 `_rest`**。
- **`$( )` 命令替换里的副作用不回传**:`base=$(dest_source_dir ...)` 在子 shell 设置的 `D_*` 父 shell 看不见,父 shell 随后读 `D_NAME` 拿到的是更早的陈旧值(日志里出现 A 目的地名字配 B 目的地路径)。规则:**凡子 shell 调用后要用 D_*,先在父 shell 显式 parse_dest**。
- **PROBE 行内嵌指纹不能含分隔符**:指纹按 pipe 分字段,指纹自身用 pipe 会破坏解析——外层协议字段分隔符要为嵌套数据改用逗号。
- **croner 2.x API**:`Cron::new(expr).parse()`,默认 5 字段标准模式(秒禁用、Vixie DOM/DOW OR),与 robfig ParseStandard 语义对齐;`find_next_occurrence(&last_check, false) <= now` 即「(last_check, now] 内有触发点」= 到期。
- **rclone crypt 的 size 校验必须跳过**:crypt 容器带 nonce+MAC,尺寸≠明文;校验逻辑要按「该目的地是否启用加密」分支,否则每次备份都误报 verify 失败。

## 2026-11-10 Siyuan issue #15 借鉴批次

- **namespace 最省力的落点是「并入 store prefix」**:三个 store(S3/WebDAV/PG)本来就有 prefix 字段,把 `namespace` 拼进 `effective_prefix()` 后上传/列表/剪枝/还原全部自动生效,零路径构造点改动。PG 没有 prefix 概念,单独在 store 内加一段 `ns_key` 并在 list 时剥回——**调用方永远只看到逻辑 key**,这是避免一条链路上「有时带前缀有时不带」的关键。
- **归属认领必须区分读写路径**:写路径(fan-out/定时)要求认领,读路径(还原/导出/管理)不要求。反过来会把灾难恢复卡在「先接管」上——恢复现场时恰恰最不该有多余的前置条件。
- **保存门禁用「服务端算 token 客户端回传」而非客户端自报**:`connection_token` = 连接字段的 sha256 前 12 字节(不含密钥/名称/保留/定时)。客户端测试拿到 token → 保存时回传 → 服务端重算比对,不一致 409。这样即使前端状态被绕过,后端仍然守住「测过才能存」。**token 里绝不含密钥**,否则等于把凭据哈希塞进前端。
- **`Object.assign` 到 reactive 不等于类型声明**:`form` 上 Object.assign 过 `schedule/region/provider`,但 `reactive({...})` 初值里没有 → typecheck 报 `Property does not exist`,而运行时因 Object.assign 又是好的。这类「运行时正常、类型报错」的坑要在初值里显式声明字段。
- **判据:借鉴前先问「这条约束在我们这里有没有作用对象」**。Siyuan 的还原身份重置针对 system.id/锁屏口令,而 Pergame 快照按 user_id 隔离且只含三类偏好设置——没有实例身份可重置。照搬会写出无对象的代码,不如在 spec 里记录判定理由。

## 2026-11-10 namespace 改为内核自动分配 ULID

- **用户可见的「作用域名」是多余的设计负担**:最初把 namespace 做成用户可填字段(字符集校验 + 创建后不可改的错误提示),而它的唯一用途是让多个实例在同一存储上隔离——这本来就是机器该决定的事。siyuan 的定稿是内核分配 ULID、前端完全不显示。**判据:一个字段如果用户无法据此做出更好的决策,就不该出现在表单里**。
- ULID 的确定性部分(前 10 位时间戳)让同批创建的 id 天然有序,调试时能一眼看出创建先后;随机部分 16 位足够避免同毫秒碰撞。
- 存量数据必须自愈:`namespace` 列引入前创建的行是空值,若不管,它们的对象会落进共享根作用域。在 `list_targets` 里做**幂等补发**(`WHERE namespace = ''`)比写一次性迁移脚本更稳——多实例部署时每台都会自我修复。

## 2026-11-12 pull 遗留清理(33 commits 横跨 UUIDv7/直写/边车删除/CDN)

- **破坏性重构必须带「残留扫描」收尾**:边车删除漏了 `.env.example` 3 处、`docs` 4 处、TODO 3 条;直写漏了 `storage.rs` 11 个 dead 方法 + `backup_bucket` 字段 + 模块注释。规则:删子系统后全仓 grep 其专属标识(服务名/env 前缀/桶名),代码+docs+compose+TODO 同批清。
- **i18n 新键只翻 3 语言会炸 typecheck**:`MessageSchema = typeof en` + 各语言 `satisfies` 要求键集全等,fallbackLocale 只救运行时不救类型。5 语言同缺 20 键,CI typecheck 全红。规则:加 en 键 = 同批给 8 语言补齐(机翻+标注待校),或改用 Partial 约束。另:回填脚本注意逗号——notif 块尾 `,,` 让 5 文件齐报 TS1136。
- **Nuxt `imports.dirs: ['stores/**']` 让 stores 下类型全局可用**:`ANode` 真身在 `stores/preferences/constants.ts`,CloneTo 从 `~/stores` barrel 显式 import 反而错( barrel 只 re-export 4 个模块)。用裸 `ANode` 即可,删显式 import。
- **`APIResult.result` 是 optional**:`makeRequest<T>` 成功也可能无 result,赋值给非空 ref 必须 `&& plan.result` 守卫。360/385 行用 `?.` 属同类已安全写法。
- **CI 只在 backend-rs/** 变更时跑 Rust 门**:fmt/clippy 红只在碰 Rust 文件的 push 上暴露;`drop(non-Drop)` warning 在 `-D warnings` 下即失败。本地门 = `cargo fmt --check && cargo clippy --all-targets && cargo test`。
- **`set_public_policy` 随 CDN 鉴权读失去存在理由**:桶保持 Private,启动时调 PutBucketPolicy 在 QuObjects 上每启一 warn。删函数 + docs 同批改(CDN_PROXY_TARGET 指向 backend)。

## 2026-11-12 MCP server 落地(rmcp 3.9 + axum)

- **rmcp 的 derive 需要 `schemars` 是直接依赖**:`#[derive(rmcp::schemars::JsonSchema)]` 展开后引用绝对路径 `schemars::…`,只靠 `rmcp::schemars` 重导出编译不过(E0433 ×7)。`cargo add schemars@1` 即可(rmcp 本身已用 1.2.x,无冲突)。
- **`#[tool_handler]` 生成的 `call_tool` 不读 `tool_router` 字段**:照抄 counter 例把 `ToolRouter<Self>` 存进 struct 会得 dead_code(clippy `-D warnings` 直接红)。只留 state 字段,`Self::tool_router()` 由宏自行调用。
- **读锁中间件只看 HTTP method**:MCP 全部是 POST,若不把 `/api/mcp` 加进 `locks::is_exempt`,备份期间连 `read_note` 都会被 423 挡掉。正确分层 = 中间件豁免 + 四个写工具自己查 `is_read_only()` 返回 JSON-RPC `-32023`。**判断「某 middleware 是否适用于新端点」先看它按什么条件放行**。
- **streamable HTTP 的 SSE 响应不一定以 `event:` 开头**:`initialize` 的帧是 `data: ` / `id: 0` / `retry: 3000` 打头。客户端解析要按「首个非空字节是否 `{`」判断裸 JSON vs SSE,不能按 `event:` 前缀。
- **`CallToolResult.isError` 总是序列化**(`false` 也在),断言「没有 isError」会假红;该断言 `== false`。
- **活体验证的最小隔离法**:NAS pg 上 `CREATE DATABASE pergame_mcp_test` + 本地二进制跑 migration + `--ignored` 单用例 + 用完 DROP DATABASE。生产桶名可复用(MCP 不落对象);**绝不复用生产库**——trash sweeper 会按保留期真删行。凭据走 `~/.agents/config.toml` 的 Infisical Machine Identity,本机需把 `PG_HOST`/`QUOBJECT_ENDPOINT` 的容器名换成 `qnap.elf-balance.ts.net`。
- **审计落点**:`logs` 表此前只有 schema 没有写入方;MCP 的 token 创建/吊销与四个写工具是第一批生产者,活测 SQL 可核(`mcp.tool.*` / `mcp.token.*`)。

## 2026-11-12 只读窗口实测(锁门活体验证)

- **验证「锁期禁止写」要自己制造锁窗口**:备份/还原才持锁,自然数据量下窗口太短不可测。做法 = 测试进程自己 `TcpListener::bind("127.0.0.1:0")`,接受连接后 sleep 4s 再回 500;后端把它当 target endpoint,备份写路径卡住 → 锁持续。断言完 `abort()` 掉所有 socket 任务,让在飞请求立即 RST,备份快速失败、锁确定性释放(否则要等 SDK 重试退避,分钟级)。
- **普通用户读不到 `/api/admin/dr-backup/status`**(`require_admin`)。探测锁态的通用信号 = 发一个**必定失败且无副作用**的 REST 写:`POST /api/nodes {"name":""}` → 未锁 400、锁中 423。零副作用 + 同源真相。
- **被测 target 的删除门禁会拒绝「不可达 target」**(无法证明已清空),所以测试清理别调 `DELETE /backup/targets/:id`;走 `cleanup_user` 的 `DELETE FROM backup_targets WHERE $uid = ANY(users)`。
- **target 名唯一约束**:重跑失败的用例会撞 `backup_targets_name_key`,测试里的 target 名要带时间戳后缀(和用户名同款)。
- 实测结论(2026-11-12,活 pg):备份期内 MCP `create_note` / `update_note` / `move_node` / `delete_node` 全部 `-32023` + `data.reason=backup_in_progress`,正文确实未变;`read_note` / `list_children` / `search_notes` 照常;REST 写同窗口 423;窗口关闭后写恢复。

## 2026-11-12 admin 路由去 dr-backup 名 + 一个真回归

- **`Router::merge` 会把被合并 router 的 fallback 一起带过来**:MCP 曾用 `Router::new().nest_service("/api/mcp", svc).layer(bearer)` 单独构造后 `merge` 进主 router → **所有未知 `/api/*` 路径变成 MCP 的 401**(本该 404)。正确做法 = 把 `nest_service` 直接写在主 router 链上(要在 `with_state` 之前),鉴权 layer 挂在全局但**自己按路径放行**(`is_mcp_path`),与 `read_only_middleware` 同款。
- **验证路由改名必须区分「路由不存在」与「路由存在但要求鉴权」**:未带凭证时两者都可能 401(全局 extractor 先跑)。有效信号 = 带普通用户 cookie 打 admin 路由:**403 = 路由在、门禁在**;404 = 路由没了。GET 打 POST-only 路由给 405,同属「路由存在」证据。
- **删子系统后残留的名字也是 bug 面**:`/api/admin/dr-backup/*` 在边车删除后仍存活 4 天,`/backup-users` 这种重复词也随之;改名 = 路由 + 模块文件 + 前端 3 处 + 测试 + 豁免表 + 文档注释一次性换,别留 alias(自己唯一的调用方,无兼容负担)。

## 2026-11-12 MCP 走前端同源代理 406(h3 丢 accept)

- **h3 1.15 的 `getProxyRequestHeaders` 把 `accept` 列入 `ignoredHeaders` 直接剥掉**,上游 fetch 于是只带 undici 默认 `*/*`。对普通 JSON 接口无感,但 `/api/mcp` 的 streamable HTTP **强制要求客户端同时接受 `application/json` 与 `text/event-stream`**,否则 406 `Client must accept both...`。生产部署正是「浏览器/客户端只连前端端口,backend 不发布」,所以 MCP 上线即挂 —— 只有把请求真的走一遍前端才发现。
- 修法:`proxyRequest(event, target, { headers: { accept: <原值> } })`(显式 opts.headers 覆盖 h3 的剥离)。
- 教训:**「本地直连后端 200」不等于「用户路径可用」**,验证要走真实入口(Nitro 代理 / 反代),尤其是依赖内容协商或流式的端点。
- 附带确认:UI 端到端可用 —— 建 token → 明文一次性展示 → 列表显示 `last_used`(MCP 调用后刷新)→ 吊销 → 同 token 立刻 401;`lint:css` 与 typecheck 仍全绿。

## 2026-11-12 本地进程连生产库前必查三件

- **`ADMIN_ACCOUNTS` 未设 = 把所有 admin 降级成 role 1**(`auth/admins.rs`:空名单 → 全部降级)。本地起一个连生产库的 backend 前,先 `SELECT id FROM users WHERE role=2` 并把名单传进 `ADMIN_ACCOUNTS`;生产若无 admin 才可省略。
- **本地跑 migration 会让该文件在生产库"冻结"**:`_sqlx_migrations` 记 checksum,之后改同一个 `.sql` 部署必失败,只能追加新文件。临时连生产库验证前想清楚这个代价。
- **sweeper 类后台任务会真删数据**:连生产库时用 `TRASH_RETENTION_DAYS=0` 关回收站清理;`SNAPSHOT_INTERVAL_HOURS` 不设即关;`backup_targets.schedule` 非空会触发 target 调度器(生产无 target 时安全)。
- 验证用的 token 该直插库(算 sha256 即可)而不是绕 UI;用完 `DELETE FROM api_tokens WHERE name='...'`,与本批笔记一起清,`logs` 里的 `mcp.tool.*` 审计行保留即可。
- **外置盘 + darwin 27.2:proc-macro dylib dlopen 失败**(`mis-aligned LINKEDIT string pool`):proc-macro dylib(`libsqlx_macros-*.dylib`)无法加载,错误链式传导成莫名 E0432(`pub use self::encode::Encode` 等未解析——真因是 derive 宏 dylib 崩了)。**终判(2026-11-13):与盘无关**——内置盘重链出完全相同的坏 dylib(同 hash 同 fileOffset);真因 = rustc 1.95.0 的 linker 在 macOS 27.2 新 dyld 下产出不合规 dylib。修法 = **rustup override set 1.100.0**(backend-rs 目录;1.100 链出的 dylib 合规,release 全量编译通过)。判别法:`python3 -c "import ctypes; ctypes.CDLL(dylib)"` 直接 dlopen 验证;先验盘、再验 dylib 内容、最后动工具链,别先入为主。
- **`.cargo` registry 在外置盘会被误伤排查**:sqlx 报 E0432 时曾疑 Resilio 同步损坏,实测 `.crate` 缓存 shasum 与 crates.io index 一致、tarball 内也无该文件——损坏假设排除,别先入为主;先验 checksum 再动手删。

## 2026-11-13 Rust 质量改进批次

- **纯 binary crate 跑不起 doc-tests**:`cargo test --doc` 报 `no library targets`。修法 = 抽 `src/lib.rs`(全部 `pub mod` + 注释) + `main.rs` 收敛为 wiring(`use pergame_rs::{...}`)。附带收益:单测与集成测同一份代码。
- **`#[forbid(unsafe_code)]` 与 test 内 scoped allow 互斥**:forbid 让任何 `allow(unsafe_code)` 直接编译错。正确形态 = crate 根 `#![deny(unsafe_code)]` + 每个 `#[cfg(test)]` mod 上 `#[allow(unsafe_code)]`(本仓 6 处:cookies x2/jwt/config/storage/owner,全是 env 突变)。
- **sqlx 0.9 删了 `Transaction` 的 `Executor` impl**:靠 `DerefMut` 到内层 Connection。owned `Transaction` 传 `&mut *tx`;借来的 `&mut Transaction` 必须 `&mut **tx`,否则 E0277(help 只列 PgConnection/PgListener,误导)。
- **存在性探测别用业务类型接**:`SELECT 1::bigint` 配 `Option<(Uuid,)>` 在 UUIDv7 批量改名中被顺手改错 → 环检查恒 500。规则:存在性查询一律 `Option<(i64,)>`。
- **还原 INSERT 必须父后子**:archive 数组顺序任意,子先父后直接撞 `nodes_parent_id_fkey`。修法 = 先全量 `parent_id=NULL` 插入,第二轮 `UPDATE` 挂父(trash 块同理有此风险,ON CONFLICT 救不了外键错,同 tx 照 abort)。
- **totp-rs 6 的 `Display` 不是 otpauth URL**:`totp.to_string()` 得 `digits: 6; step: 30; ...` 摘要,二维码扫不出。真 URL = `to_url()`(需 otpauth feature,已启用)。`qr_code_url` 字段名即契约,改 Totp 构造必须断言 URL 前缀。
- **集成测试随直写/门禁语义过期**:零 target 快照不持久化(列表恒空)+ test-before-save 门 + 还原 force 门,三处旧测试全挂。修法 = 测试自带 target(helper 走 probe-draft 拿 token 再建)+ 还原传 `{"force": true}` + 测后 `empty` 清对象。测试桶名 = `pergame-tgt-test`(非文档旧名)。
- **jsonwebtoken 默认 60s leeway**:过期测试用 -10 秒 TTL 必 flaky,用 -3600。
- **cookie 域空 label 不得加前导点**:`needs_leading_dot("a-.")` 曾为 true → Domain 永不匹配。proptest 抓到,修实现(≥2 非空 label 才加)。
- **拆文件代理会互相覆盖**:restore 代理写了半截 apply.rs 被 kill,残留 11 行头文件 + 错位 mod.rs。规则:并行拆分前先快照,收割时 `wc -l` + `cargo check` 逐个验,半成品从 git 恢复源文件手工重切。

## 2026-11-13 配额与去重批次

- **`edit` 的 PUT N.=M 是替换不是插入**:多次 `PUT 12.=12` 顶掉整行模块声明(auth/response 丢失致全仓 E0432)。插入用 `PUT >N:` / `PUT <N:`;替换后立刻 `cargo check`。
- **sqlx INT4 不能解进 i64**:`refcount INT` 配 `Option<(i64,)>` 运行期 500。规则:INT 列一律 i32,RETURNING 同。
- **acquire 先+1 后 PUT 要回滚**:PUT 失败残留 refcount 行,重试因 refcount≠1 跳过 PUT → 行在物无。修法 = PUT 失败删 refcount=1 的行。
- **sqlx 0.9 `&mut Transaction` 无 Executor**:借来的 tx 传 `&mut **tx`(经 DerefMut 到 Connection);直接 `&mut *tx` E0277。
- **migration 落生产库即冻结**:0008/0009 已在生产库应用,勿改文件;调结构只能 0010+。
- **JWT role 在 token 里**:集成测试改 role 后必须重登,否则走旧身份(配额测试曾因此误测 admin 豁免)。
- **QuObjects 目录占位符**:`dir/` 零字节 key 在 empty/scan 中要显式排除,迁移后 prefix 下只剩占位符属正常,手动清。

## 2026-11-13 A2 Joplin 特性批次

- **markdown-it 插件生态多已停更**:multimd-table 4.2.3 调 `md.utils.assign`(13+ 已删)直接炸。修法 = use 前垫 `Object.assign`(语义同)。装旧插件先跑个最小渲染探针再接管线。
- **i18n 花括号是占位符**:文案里 `{"scale": 1.2}` 示例让 vue-i18n 编译炸 500。示例改无花括号写法(scale 1.2)。
- **`edit` PUT N.=M 是替换**:多次顶掉邻行(import/闭合括号)。插入用 `PUT >N:`;改完跑 typecheck(本批 typecheck 抓到 3 次顶掉)。
- **偏好开关实测要走 UI**:直接写 IndexedDB 会被后端 merge 覆盖。开关类验证 = 设置页点开关 → 同步后端 → 重载验渲染。
