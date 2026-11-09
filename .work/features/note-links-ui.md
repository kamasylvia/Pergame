# Design Draft — 软连接前端(NL-B 前端;note-links.md §4 落地设计)

> 状态:**已定稿并实施 ✅(2026-11-09)**。用户拍板:O1 采纳(order 列)、O2 采纳(两处标注)、O3 采纳(不做位置面包屑)、**O4 = 文件夹克隆做**。实施 commits:后端 `81fee759`、前端 `ab9dea10`;真浏览器走查全过(克隆/角标/移除/删除提示)。
> **Trilium Notes 实现对位**(官方 Docs wiki tree-concepts/cloning-notes,2026-11-09 查证):branch = (parentNoteId, noteId) 位置元组,clone = 同 note 多 branch、内容单份;**folder clone 子树天然跟随**(子 note 经自身 branch 挂 folder note 本体,与到达边无关——我们 children.parent_id 挂 folder 本体,TreeBuilder link 行共享子树,同构);树中标题右侧 **asterisk**,且多位置 note 的**所有**位置都显示;入口 = 右键「Clone note to…」+ Note Paths ribbon 列全部位置(后者我们后置);删除 = 位置感知(多位置删位置、唯位置删 note)——与我们主/从模型不同:我们 parent_id 兼内容锚,主位置不可"移除"只能真删,菜单按此实现(差异已记录);branch prefix(位置级别名)后置,spec §6.1 v1 跟随原名。

## 1. 现状关键事实(盘点结论)

- 树 = 纯客户端合成:`GET nodes/user/@me` 返回**扁平 node 数组**(每行单 `parent_id`),`IndexedCollection`(Map by id)→ `TreeBuilder` 递归建树(`app/helpers/TreeBuilder.ts:50-94`)。
- **后端 CTE 不输出第二个位置行**:link 位置由递归树 CTE 的 OR EXISTS 臂保证"子树查询可见",但返回行 `parent_id` 仍是主位置 → 前端拿不到"同 node 多位置"信息。spec §3 说"树接口无需新端点"在扁平全量接口上**不成立**(见决策 D1)。
- 若强行让接口对 link 输出重复 id 行,前端 `IndexedCollection.set()` 会以最后一行覆盖(byParent 重建),第二位置静默丢失(`app/helpers/IndexedCollection.ts:93-126`)。
- 其它吞位置点:`recomputeTags` 把孤儿 `parent_id` 改写为 `''`(`nodes.store.ts:96-98`)、`getAncestors`/`isDescendant`/`getDescendantIds` 全按单 parent 链走。
- 搜索/CommandCenter 全部 id 寻址(`/dashboard/docs/:id`),与位置无关 → **天然兼容,零改动**。
- 复用件:右键菜单 `Node/Action/ContextMenu.vue`(switch :63-98)、目录选择器 `AppSelectNode.vue`(递归树选,Permissions.vue 已用)、badge 先例(shared icon 仅 level===0,`SidebarItem.vue:23`)、删除确认 `Node/Modals/Delete.vue`。

## 2. 决策项(拍板点)

**D1 数据获取——需新增 `GET /nodes/links`(当前用户全量 links)**
spec 原判"无需新端点"基于树 CTE 已含位置;实测扁平接口不携带位置信息。轻端点一行 SQL(`SELECT id, parent_id, node_id FROM node_links nl JOIN nodes n ... WHERE n.user_id=$1`),前端启动时随 nodes 一起拉入 store。
*备选*:逐 node 查 `/nodes/{id}/links`——N+1,否决。

**D2 数据模型——links 为独立集合,不动 nodes**
store 增 `links: NodeLink[]`(id/parent_id/node_id,全 string)与 `linksByParent` 索引。`IndexedCollection`/`DB_Node` 不动(nodes 仍单 parent 单份内容)。

**D3 渲染合成——TreeBuilder 层注入位置,渲染行共享 node**
`getChildrenIds(parent)` 并入 `linksByParent[parent]` 的 node_id → 同一 node 在两个目录下各出一行(SidebarItem 复用)。展开态/选中态按 **node.id** 共享(同一笔记,展开即处处展开)。`treeMap` 键改复合 `${parentId}:${nodeId}`(仅树内部;对外 API `getNode(id)` 仍主位置)。`recomputeTags` 孤儿改写保持只作用于 nodes 主位置(link 行随 node 存在性自然失效,删 node 后 links 由 FK CASCADE 清)。

**D4 删除语义三分(ContextMenu)**
- link 位置行上的菜单:`「从此目录移除」`(DELETE /nodes/{id}/links/{parentId})替换 `delete` 项;`彻底删除` 保留(红色次级,确认框文案标明"所有位置一起删除")。
- 主位置行:菜单不变(delete = 真删除);新增 `「克隆到…」`。
- 删除确认框:若该 node 有 link 位置,提示行加"此笔记另有 N 个克隆位置,将一并删除"。

**D5 克隆入口**
`ContextMenu.vue` 加「克隆到…」→ 复用 `AppSelectNode` 选目标目录(排除自身及其子孙——前端先检 `isDescendant` 快失败,后端 400 兜底)→ `POST /nodes/{id}/links {parent_id}` → 成功后增量 push 进 store.links + toast。**无既有 move-dialog,不新建通用 move 对话框**(拖拽已覆盖移动;克隆专属 picker 即可)。

**D6 角标 + 面包屑(v1 范围)**
- 角标:`SidebarItem` icon 处加 clone 角标(link 位置行 + 主位置行均标;level 不限),仿 shared icon 先例;新 icon 走 iconify 集(与现有一致)。
- 面包屑 v1 **只按主位置**(`getAncestors` 不动)。位置感知面包屑(路由 `?from=parent`)后置——开放问题 O3。
- 打开行为:任一位置点击 → 同一 `/dashboard/docs/:id` 路由,内容天然一致。

**D7 排序位——建议随本批加 migration 0004**
`node_links` 现无 `order` 列(0003;spec 开放问题 2)。树内克隆位排序 v1 用 `created_timestamp`(树排序器已按 order→name→created 兜底)。若要支持克隆位 pin(-1),加 `ALTER TABLE node_links ADD COLUMN "order" INT NOT NULL DEFAULT 0` + 排序器读取;**建议本批一并做**(改动极小,免二次迁移)。

## 3. 组件改动清单

| 文件 | 改动 |
|---|---|
| `stores/nodes.store.ts` | links 状态 + fetchLinks(`/nodes/links`)+ cloneTo/removeLink action |
| `helpers/TreeBuilder.ts` | getChildrenIds 并入 links;复合键;排序并入 link 行 |
| `composables/useNodesTree.ts` | treeMap 复合键适配;getAncestors 不动(v1) |
| `components/Sidebar/SidebarItem.vue` | clone 角标;link 行菜单项切换 |
| `components/Node/Action/ContextMenu.vue` | 克隆到…/从此目录移除 两项 |
| `components/Node/Modals/CloneTo.vue` | 新建(壳 = AppSelectNode + 确认) |
| `components/Node/Modals/Delete.vue` | 多位置提示行 |
| i18n ×8 | 新词条(clone/removeLink/multiPositionWarning) |

后端配套:D1 端点(`repo/links.rs` 加 list_by_user + route)、D7 migration 0004(若拍板)。

## 4. 开放问题(需拍板)

- **O1**:D7 是否本批加 order 列?(建议:加)
- **O2**:角标是否只在**非主位置行**显示?(建议:两处都标,所见即有克隆)
- **O3**:位置感知面包屑(`?from=`)是否 v1 要?(建议:不做,主位置够用)
- **O4**:克隆位置内的新建子节点——link 指向文件夹时子节点落在哪?(建议 v1:链接到文件夹时该位置**只读引用不展开**?或允许建子节点落到主位置子树——语义未定,**spec §6.3 只定了分享对 node.id 生效**;v1 建议限制:仅文档类 node 可克隆,文件夹克隆后置,规避子节点归属歧义)

## 5. 验收样例(实施后)

1. 目录 A 文档 → 右键「克隆到…」→ 选目录 B → B 下出现同名条目 + 角标;A 下原条目 + 角标。
2. B 位置打开 = A 位置打开(同 id,内容一致);编辑一处两处同步(单存储)。
3. B 位置「从此目录移除」→ B 消失,A 不动。
4. A 位置「删除」→ 确认框含"N 个克隆位置一并删除"→ 全消失。
5. 克隆到自己的子孙目录 → 前端即刻 toast 环拒绝(后端 400)。
6. `cargo test --test integration -- --ignored` 全绿 + 前端 lint 绿。
