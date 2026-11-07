# Feature Spec — 目录索引文件(index.md)

> 状态:待实施(随 Rust 重写)。跟踪:issue #4。需求源:用户 2026-11-07(原文照录见下)。

## 需求(用户原文)

> 新建目录时,自动创建一个索引文件 `index.md`。该文件可以在设置项中配置 "文件名 (默认 index)",可见性(默认可见)。该文件会用 dataview 的形式列出目录下的子目录/子文件。如果在 dataview 中点击子目录/子文件,自动跳转。点击子目录会跳转到子目录的 `index.md`(会被用户在设置项中定义的名称重命名)。该文件只能在设置中进行重命名,不能单独重命名,设置中重命名后同时适用于已有索引文件和将来新创建的索引文件。

## 设计决策(建议,实施时定稿)

| 项 | 设计 |
|---|---|
| 身份标记 | `nodes.metadata.index = true`(与文件名解耦,改名/查找都靠它) |
| 自动创建 | 后端建 folder 成功后自动建子 doc:name = 设置值,`metadata.index = true`,content = dataview 块模板 |
| 文件名设置 | `index_file_name`(默认 `"index"`,user_settings JSONB);**改名 = 批量 rename** 所有 `metadata.index=true` 节点(后台任务),已有+将来一致,搜索/树/引用全一致 |
| 可见性设置 | `index_file_visibility`(默认 visible)。开放问题:visible = 树中正常显示 vs 不显示(仅 dataview 跳转可达)?倾向 = 树中显示,flag 控制排序沉底;实施时与用户确认 |
| dataview 渲染 | 特殊 md 代码块(如 ` ```index `);前端渲染为可点击列表:子目录优先,显示 name/icon/updated;数据 = 现有 `list_children` API。**澄清(2026-11-07):dataview 非 Alexandrie 内置(全仓零命中),仅借 Obsidian Dataview 描述效果;实现 = markdown-it-container 注册自定义 `index` 块 + 渲染组件,编辑器侧 `customBlocks.ts` 加预览高亮** |
| 点击跳转 | 文件 → 该文件路由;目录 → 该目录的 index.md(**不存在则按需自动创建**,与新建逻辑同一函数) |
| 改名保护 | 前端隐藏 index 文件的改名入口;后端 rename 端点拒绝 `metadata.index=true`(除非经设置批量路径);删除行为开放问题(倾向:可删,目录被再次访问时按需重建) |
| **置顶(2026-11-07 补)** | index 文件**永远在所属目录置顶显示**:创建时 `order = -1`(复用现有 pin 机制);前端隐藏其"取消置顶"入口,后端对 `metadata.index=true` 拒绝 unpin/手动排序;置顶组内 tie-break(index vs 用户手动 pin 的其他文件)实施时定(倾向 index 再按名字/时间排最前) |
| 迁移 | 设置改名仅影响存量 index 文件名,不影响内容;已有目录无 index → 访问时懒创建(不做全量回填) |

## 开放问题(实施时裁决)

1. 可见性语义精确化(树中显示 vs 隐藏)
2. index 文件可否被用户删除/移动(建议:可删,懒重建;移动 = 拒绝,它属于父目录)
3. dataview 块参数(排序/显示列)是否开放给用户写(首版固定)
