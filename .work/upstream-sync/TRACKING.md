# Upstream Sync Tracking — Smaug6739/Alexandrie

> fork 无法直接合并上游(Go → Rust 重写 + 全量改名);此文件追踪上游增量,逐 commit 分类处置。
> 规则:`git fetch upstream` 后把 `main..upstream/main` 逐条落表;前端改动 cherry-pick 适配 fork 品牌,Go 后端改动评估是否需要 Rust 对应实现,deps bump 不搬运(Rust 侧由 cargo 更新覆盖)。

## 状态

- **2026-11-09**:upstream remote 重建。⚠️ 上游仓已改回 **`Smaug6739/Alexandrie`**(原 `Smaug6739/Collectanea` 404,2026-11-09 实测)。merge-base = `429e2b9b`(fix(errors): new errors handling with correct 404 page),落后 5 commits。

## 待处置

| upstream commit | 类型 | 内容 | 处置 |
|---|---|---|---|
(空——待处置项清零后此表留白)

## 已处置

| upstream commit | 处置 | fork commit | 备注 |
|---|---|---|---|
| `a4dc87a7` | ✅ 移植 | `20c61946` | cherry-pick 干净;eslint 绿 |
| `33e4bb2d` | ✅ 移植 | `53dec342` | cherry-pick 干净(纯 CSS 属性序,无品牌串);stylelint 绿 |
| `97155bd8` | ⏭️ 不跟随 | — | fork 版本线自定 |
| `750f161f` | ⏭️ 不适用 | — | Go deps,Rust 重写后由 cargo 更新覆盖 |
| `15dba0ed` | ⏭️ 不适用 | — | 同上 |
