# Feature Spec — 笔记软连接(多目录出现,Trilium clone 对位)

> 状态:已登记待实施。跟踪:issue 待开(实施时建)。需求源:用户 2026-11-08(Trilium Next 对位功能)。
> 需求原文:同一个笔记可以出现在不同的目录下(软连接/clone)。

## 1. 模型决策(Trilium 对位)

Trilium:noteId(内容)+ branchId(树位置)分离,clone = 同一 noteId 多个 branch。
Pergame 现状:nodes 行 = 树位置 + 内容一体(id 即内容键,S3 路径、备份、搜索全挂 node.id)。

**选型:克隆表方案(轻量、不动内容存储)**
- 新表 `node_links`:`(id, parent_id, node_id, order, created_timestamp)`,UNIQUE(parent_id, node_id)。
- 原 `nodes.parent_id` 保留 = 主位置;`node_links` 行 = 额外出现位置(软连接)。
- 不拆 content 到独立 notes 表:S3 键、备份 zip、FTS、还原全部继续以主 node.id 为锚,零迁移。

否决方案:content 分离重构(Trilium 全对位)——动 S3 键/备份/搜索/还原四大件,收益不成比例。

## 2. 行为定义

| 场景 | 行为 |
|---|---|
| 「克隆到目录…」 | 前端在目标目录下创建 node_links 行;树中该位置渲染同一笔记(同一 node.id,同名引用) |
| 树查询 | list_all/list_shared CTE 增加 node_links 侧:`UNION SELECT n.* FROM node_links nl JOIN nodes n ON n.id = nl.node_id WHERE nl.parent_id IN (...)`;**环检测**:CTE 走 path 数组或 depth 上限,克隆指向祖先时拒绝(link 校验:目标目录不得是节点自身或其子孙) |
| 打开/编辑 | 任意位置打开同一 node.id,内容天然一致(单存储,无同步问题) |
| 删除 | 位置删除 = 删 node_links 行(该引用消失);**真删除** = 删 nodes 行(所有位置消失,进回收站语义)——前端区分「从此目录移除」与「彻底删除」 |
| FTS/搜索 | 天然单份,命中即该笔记(多位置不重复计) |
| 备份 zip | documents.json 含 node_links 表;还原按表重建(指向缺失 node 的行跳过+报告) |
| 权限 | 克隆目标目录须对该笔记有 ActionRead,对目标父目录有 ActionUpdate(复用现有守卫) |
| 计数/size | GetUserUploadsSize 等按 node 行计,克隆行不影响(单份内容) |

## 3. API

- `POST /nodes/{nodeId}/links` `{parent_id}` → 201 link(校验环 + 双向权限)
- `DELETE /nodes/{nodeId}/links/{parentId}` → 200(位置移除)
- `GET /nodes/{nodeId}/links` → 该笔记的全部出现位置(前端「显示所有位置」)
- 树接口无需新端点(list CTE 已含链接位置)

## 4. 前端

- 树渲染:同一 node.id 多次出现,图标加 clone 角标;CommandCenter/搜索跳转取任一位置
- 右键菜单:「克隆到…」(目录选择器)/「从此目录移除」/「彻底删除」三分
- 打开笔记时面包屑按当前树位置生成

## 5. 实施拆分

| 阶段 | 内容 | 估时 |
|---|---|---|
| NL-A | migration 0003(node_links)+ repo 查询(含环校验 CTE)+ 三端点 | 1 天 |
| NL-B | 树 CTE 扩展 + 前端树渲染/菜单 | 1-1.5 天 |
| NL-C | 备份 zip 含 links 表 + 还原 | 0.5 天 |

## 6. 开放问题(实施时定)

1. 克隆笔记在目标目录的显示名:跟随原笔记(改名联动)vs 独立别名 —— 建议 v1 跟随
2. node_links 行的 order/pin:支持与普通节点相同的排序位
3. public 分享链接是否对克隆位置生效:建议对整个笔记生效(按 node.id 判,已天然如此)
