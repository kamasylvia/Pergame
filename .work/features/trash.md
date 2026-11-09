# Feature Spec — 回收站(trash)

> 状态:已批准待实施。需求源:用户 2026-11-09 裁决(本地误删恢复走回收站,不做 git 级版本历史)。
> 替代关系:`features/backup-v2.md` 已取消;B 层「本地误删恢复」由本 spec 承载(见 `backup-restore.md` §分层表)。

## 1. 现状(为什么必须自建)

上游 Alexandrie **无回收站**。删除 = 不可逆物理删除,证据链:

| 层 | 位置 | 行为 |
|---|---|---|
| Schema | `backend/app/migrations/000010_nodes.up.sql` | 无 `deleted_at` 列;`nodes_parent_fk ... ON DELETE CASCADE` |
| Go repo | `backend/repositories/node.repository.go:369` | `DELETE FROM nodes WHERE id = ?` |
| Go service | `backend/services/node.service.go:260` | 先捞 role=4 后代资源 → **删 S3 对象** → DB 级联 |
| Rust 端口 | `backend-rs/src/routes/nodes.rs:366`、`repo/nodes.rs:344` | 逐字复刻同一逻辑 |

删一个文档 = 正文 + 全部子节点 + 所有附件对象一起消失。

## 2. 模型:软删 + 保留期

### 2.1 新列

```sql
ALTER TABLE nodes ADD COLUMN deleted_at BIGINT NULL;   -- epoch ms, NULL = 存活
CREATE INDEX idx_nodes_deleted ON nodes (deleted_at) WHERE deleted_at IS NOT NULL;
```

`trash_origin_parent` **不加**:恢复时若原父级也已删/已过期,由恢复逻辑判定是否上提为根级(见 §3.3),无需额外列。

### 2.2 保留期

配置项:`TRASH_RETENTION_DAYS`,env 读取,默认 **30**,`0` = 永不自动清理(仅手动)。

到期清理 = 后台 ticker(周期 `TRASH_SWEEP_INTERVAL_HOURS`,默认 24),对 `deleted_at < now - retention` 的行执行**物理删除 + S3 清理**。

### 2.3 级联语义

删除一个目录 → 其**整棵子树**同一批次打上同一个 `deleted_at`(递归标记)。这样:
- 恢复根节点时整棵子树一起回来(子树 `deleted_at` 值相同即可判定同批);
- 列表展示只在**子树根**出现一条(祖先也删了则不重复展示)。

实现:递归 CTE 取子树 id 集合 → `UPDATE nodes SET deleted_at = $ts WHERE id = ANY($ids) AND deleted_at IS NULL`。

## 3. 行为定义

| 场景 | 行为 |
|---|---|
| `DELETE /nodes/:id` | 改为**软删**(整棵子树同批)。**不动 S3、不动 node_links 行** |
| 树查询 | `list_all` / `list_shared` / `search` / `get_public` / `public_descendants` / `uploads_size` / `has_permission` 全部加 `deleted_at IS NULL` |
| `get_by_id` | 加 `deleted_at IS NULL`(存活语义);回收站内部另走 `get_by_id_any` |
| 权限守卫 | 已删节点对外表现 = 不存在(404),避免「删除后仍能通过权限接口探测」 |
| `node_links` | 指向已删节点的行在树查询侧过滤(join 时带 `n.deleted_at IS NULL`);行本身保留,恢复后自动复现 |
| 上传体积统计 | 已删节点仍占 S3 空间,计入 `uploads_size`(避免用户靠删除绕过配额),但**不计入**树/搜索 |
| 回收站列表 | 只列**子树根**(无已删祖先的已删节点),含 `deleted_at`、`name`、`role`、`size`、子树节点数 |

### 3.1 端点集

| 方法 | 路径 | 说明 |
|---|---|---|
| `GET` | `/api/trash` | 列表(仅本人;admin 可带 `?userId=` 查他人) |
| `POST` | `/api/trash/{nodeId}/restore` | 恢复该节点及其同批子树 |
| `DELETE` | `/api/trash/{nodeId}` | 彻底删除该节点及子树(**此时才删 S3 对象**) |
| `DELETE` | `/api/trash` | 清空回收站(全部彻底删除) |

权限:全部要求 **owner(App 上 `user_id == actor.user_id`)或 app admin**。已删节点不能再走 `require_node_action`(那会 404),改走 `require_trash_owner`(查 `get_by_id_any` + 校验 `user_id`)。

### 3.2 恢复规则

1. 目标节点必须 `deleted_at IS NOT NULL`,否则 400。
2. 取**同批子树**(`deleted_at` 值相等且为该节点的 `deleted_at`)→ 全部置 `deleted_at = NULL`。
3. **父级检查**:若 `parent_id IS NOT NULL` 且父节点仍 `deleted_at IS NOT NULL`(且不属于本批)→ 该节点**上提为根级**(`parent_id = NULL`),响应里带 `reparented: true`。理由:父级还在回收站里,挂着恢复会让节点不可见。
4. 恢复后 `updated_timestamp` 不变(恢复不是内容修改)。

### 3.3 彻底删除

1. 取子树 id 集合。
2. 收集 role=4 资源的 storage key → 删 S3 对象(失败仅记日志,与现有 Go parity 一致)。
3. `DELETE FROM nodes WHERE id = ANY($ids)` —— 级联带走 permissions / node_links / 子节点。
4. 只允许对已软删节点操作;对存活节点调此端点 = 400(强制先走普通删除)。

## 4. 前端

新建 `/dashboard/trash` 页(Nuxt 路由),sidebar 入口挂「回收站」:

- 列表:名称、类型图标、删除时间(相对时间)、大小;仅列子树根。
- 行操作:**恢复** / **彻底删除**(二次确认);顶部**清空回收站**(二次确认,提示不可逆)。
- 恢复后 toast 提示;若 `reparented = true` 额外提示「原父目录已在回收站,已移至根级」。
- store:`nodes.store.ts` 新增 `fetchTrash()` / `restoreNode(id)` / `purgeNode(id)` / `emptyTrash()`。
- 删除后本地状态:现有 `remove()` 逻辑不变(前端仍即时从树里摘掉),仅在调用回收站页时拉取真实列表。

## 5. 备份兼容

备份 zip(v2 manifest)序列化 `nodes` 表全列。新增 `deleted_at` 后:
- **导出**:已删节点**不导出**(快照是「可见内容」的备份,回收站不该污染)。
- **还原**:目标端节点 `deleted_at` 一律置 `NULL`。
- 引擎的列白名单需同步(见 `restore.rs` / `backup.rs` 的 nodes 列清单)。

## 6. 不做的事

- 不做 git 级内容版本历史(用户明确否决)。
- 不做「按用户保留期单独配置」(全局 env 即可)。
- 不做回收站内搜索(列表足够;节点多了再说)。
- 不做回收站节点的公开访问残留清理(node_links 行保留,恢复即复现)。

## 7. 验证清单

- [ ] `cargo test` 单测:软删隔离(树查询不含已删)、同批复原、父级已删时上提根级、彻底删除才清 S3
- [ ] `cargo clippy --all-targets` 0
- [ ] 集成用例(需活后端):删除→列表→恢复→树中复现;删除→彻底删→404
- [ ] 前端 `bun run lint` + `bun run lint:css` 0
- [ ] 迁移幂等:0005 在既有库上 `up` 通过(可空列 + 部分索引)
