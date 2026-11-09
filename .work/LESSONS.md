# LESSONS

(空——新坑在此沉淀;格式:日期 + 现象 + 根因 + 规则)

## 2026-11-09 改名批次(macOS M2 机首跑)

- **Go 依赖拉取被墙**:本机(M2)`go build/vet` 拉 module 超时 → 单次 `GOPROXY=https://goproxy.cn,direct` 环境变量解决,不改全局 go env。
- **rustc 版本差**:backend-rs 依赖 toasty 0.11 需 rustc ≥1.95;本机 default stable=1.94.1(1.95.0 已装)→ `rustup override set 1.95.0`(backend-rs 目录,machine-local ~/.rustup settings,不进仓)。换机首跑 backend-rs 编译失败先查 rustc 版本。
- **bun install 重写 lockfile**:换机 bun 版本不同 → `bun install` 把 bun.lock 重写数千行(格式 churn)。改名/小改动批次禁收编此 churn:改完立即 `git checkout -- bun.lock`,只手工同步必要 token(如 package name 行);前端装依赖用 `bun install --registry https://registry.npmmirror.com`。
- **exFAT 卷 AppleDouble 垃圾**:仓库在外置 exFAT 卷,工具运行会生成 `.!NNNN!xxx` 前缀临时文件,git status 出现未跟踪垃圾 → 删除不入库。
- **BSD sed 多字节崩溃**:macOS sed 批量改中文文件报 `illegal byte sequence` → 一律 `LC_ALL=C sed -i ''`(纯 ASCII 替换模式对 UTF-8 安全)。
- **既有缺口(上游遗传,非改名引入)**:`backend/tests/helpers.go:227` 引用 `LoginAs`,而其定义在 `tests/auth_test.go`(测试编译单元才可见)→ `go build ./...` 在 tests 包失败;`go vet ./...` 与 `go test` 不受影响。待顺手修。
