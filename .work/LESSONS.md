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
