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
