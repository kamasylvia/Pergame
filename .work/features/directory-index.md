# Feature Spec — 目录索引文件(index.md)

> 状态:**已实施(2026-11-13)→ 已移除(2026-11-18,存档)**。用户裁决:index v2 的两张表(children/recent)直接内联为 workspace/folder 容器视图的默认概览(`ViewSelection` overview 模式),index 文件失去存在意义,整个功能删除——migration `0013` drop 唯一索引 + 存量节点软删入回收站;DQL `DIRECT`/`id:`/`viewed_timestamp`/`node_views` 保留(dataview 引擎通用);普通文档的 `children`/`recent` 块仍可用,`index` 块语法移除。跟踪:issue #4。需求源:用户 2026-11-07(原文照录见下)。

## 需求(用户原文)

> 新建目录时,自动创建一个索引文件 `index.md`。该文件可以在设置项中配置 "文件名 (默认 index)",可见性(默认可见)。该文件会用 dataview 的形式列出目录下的子目录/子文件。如果在 dataview 中点击子目录/子文件,自动跳转。点击子目录会跳转到子目录的 `index.md`(会被用户在设置项中定义的名称重命名)。该文件只能在设置中进行重命名,不能单独重命名,设置中重命名后同时适用于已有索引文件和将来新创建的索引文件。

## 设计决策(已定稿)

| 项 | 设计 |
|---|---|
| 身份标记 | `nodes.metadata.index = true`(与文件名解耦);服务端签发,客户端 create/update 提交的该键被剥离(不可伪造) |
| 唯一性 | `migration 0010`:`UNIQUE INDEX idx_nodes_index_child ON nodes (parent_id) WHERE metadata->>'index'='true' AND deleted_at IS NULL` |
| 自动创建 | `POST /nodes` 建 role 1/2 成功后即建 index 子文档(content = 新模板见下,`order=-1`);响应体 `{node, index}` |
| 懒重建 | `GET /nodes/:id` 读到本人 role 1/2 且无 index 子 → 幂等补建(`auto_index` 字段回传);check-then-insert 竞态由唯一索引折回 |
| 文件名设置 | 偏好键 `indexFileName`(默认 `index`,≤50 字);`PUT /user/settings` 检测到变化 → 同请求内批量 UPDATE 存量(含回收站行),响应带 `index_renamed` 计数 |
| 可见性设置 | 偏好键 `displayIndexFiles`(默认 on):侧边栏树中显示/隐藏;隐藏只影响展示,不影响跳转与置顶 |
| index 块渲染 | ` ```index ` 复用 #5 引擎:预设 `LIST FROM <当前目录>`(整棵子树),前端二次过滤掉自身与其他 index 文件,目录行优先 + 图标前缀 |
| 点击跳转 | 文件 → 该文档;目录行 → 该目录 index(缺失则先 `GET /nodes/<folder>` 触发懒建再跳) |
| 改名保护 | 后端:index 行的 `name`/`parent_id`/`order`/`metadata` 在 PUT 中被冻结(内容编辑照常);移动 → 403;删除 → 403;MCP `update_note` 改名/-32003,`move_node`/-32003,`delete_node`/-32003 |
| 置顶 | `order=-1`;前端所有比较器(index 优先 tie-break)把 index 排在同级最前;UI 隐藏 pin/duplicate/delete 入口 |
| 还原 | 归档中同目录多个 index:首个保留标记,其余剥标记 + warning(不整事务回滚) |
| 迁移 | 不做全量回填;老目录首次读取时懒创建 |

## 开放问题(2026-11-13 用户裁决)

1. 可见性 → **树中置顶显示 + 设置可隐藏**(`displayIndexFiles`)。
2. index 块范围 → **整棵子树**(与既有预设一致),前端过滤 index 自身。
3. 删除与移动 → **都可禁止**(锁定);目录删除时随子树进回收站,还原带回。

## 模板 v2(2026-11-16,思源父目录内容对位)

自动模板不再是单个 ` ```index ` 块,而是两个预设块(均可点击跳转,渲染时按所在位置解析,重命名/移动不失效):

```markdown
## 目录

```children
```

## 最近查看

```recent
```
```

- ` ```children ` = 当前目录的直接子级(非递归,目录优先,过滤 index 自身);在父文档页则列该文档的直接子级。
- ` ```recent ` = 当前子树内按 `viewed_timestamp` 倒序的前 10 篇文档(`WHERE role = 3`,未看过的沉底)。
- `viewed_timestamp`:每用户每节点最后查看时间;`GET /nodes/:id` 成功读取时服务端 upsert(`node_views` 表,migration 0012);MCP 读取不记(程序化访问不污染)。
- 底层 DQL 同步扩展(供手写块用):`FROM "id:<uuid>"`(稳定引用)、`FROM ... DIRECT`(收敛直接子级)、`viewed_timestamp` 字段(SORT 恒 `NULLS LAST`;TABLE 可投影,行内回填)。
- 既有 ` ```index ` 预设保持递归子树不变(向后兼容)。

## 实施记录(2026-11-13)

- 后端:`repo/nodes::{is_index,get_index_child,rename_index_files}`;`routes/nodes::{DEFAULT_INDEX_NAME,index_template,index_file_name,index_name_from_general,ensure_index_child}`(create 自动建 + get 懒建);`routes/settings` 改名批处理;MCP 三写工具拒绝 index 操作;`restore/apply` 去重归档 index;`migration 0010_index_child.sql`。
- 前端:`helpers/node.ts::isIndexFile/findIndexChild`;`dataview-render` index 分支(过滤 + 目录优先 + 图标);`ContentCompiled` 目录行跳转(懒建);`ContainerView` 打开 index 按钮;`nodes.store` post/fetch 吸收 index、update/remove 本地锁;`useNodesSort`/`IndexedCollection`/`Tree::Item`/`public Tree`/`home` 全部 index 优先 + 置顶网格排除;`Metadata`/`MarkdownEditor`/`ContextMenu` 隐藏或禁用被锁字段;`TreeBuilder` 侧(index)按偏好剪枝;设置页 Documents 加文件名输入、Appearance 加显示开关;8 语种文案。
- 测试:`cargo test --lib` 128 绿(含 index 命名/标记/模板单测);集成新增 `directory_index_lifecycle`(自动创建/403 三件/改名批处理/新目录用新名),并修正既有 5 处受 index 影响的计数断言(trash/restore/DQL)。
