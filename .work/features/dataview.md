# Feature Spec — Dataview 查询引擎

> 状态:待实施(随 Rust 重写;pg/jsonb/FTS 为地基)。跟踪:issue #5。需求源:用户 2026-11-07(在 #4 索引块之上追加通用 dataview 功能)。

## 需求

笔记内可写 dataview 查询块,动态列出/表格化满足条件的笔记,渲染可点击。

## 已定决策

| 项 | 决策 |
|---|---|
| 查询语言 | **DQL 子集**:`LIST` / `TABLE [col,...]` + `FROM <folder|"path"|#tag...>` + `WHERE <expr>` + `SORT <field> [ASC\|DESC]` + `LIMIT n`;块语法 ` ```dataview ` |
| 字段面 | 内置字段:`name`/`tags`/`description`/`role`/`created_timestamp`/`updated_timestamp`/`folder(path)` + `metadata.*`(JSONB 自定义键) |
| 执行位置 | **后端**:新端点 `POST /api/nodes/query`(DQL 文本 → 手写递归下降 parser → 参数化 SQL);权限 = actor 体系,只返回有权限的节点;**只读**,无任何写语义 |
| 渲染 | 前端注册 dataview 容器块:编辑态 CodeMirror 高亮(复用 `customBlocks.ts` 模式),阅读态渲染 LIST(行 = 可点击链接)与 TABLE(行首链接列) |
| 与 #4 index 块 | **index 块 = dataview 预设语法糖**(自动生成 `LIST FROM <本目录>` 块);引擎单点维护,#4 不再单独实现渲染 |

## DQL 子集语法草案(实施时定稿)

````
```dataview
TABLE name, updated_timestamp, metadata.status
FROM "Projects", #active
WHERE updated_timestamp > <时间表达式>
SORT updated_timestamp DESC
LIMIT 20
```
````

- `FROM`:双引号目录路径(含子树)与 `#tag`,逗号 = AND;不支持嵌套表达式(v1)
- `WHERE`:字段比较(`=`/`!=`/`>`/`<`/`contains()`/`startswith()`)与 AND/OR 组合
- `SORT`/`LIMIT`:直译 ORDER BY/LIMIT;LIMIT 硬顶 100
- 安全:parser 白名单字段与函数;参数化绑定;查询复杂度/深度上限;执行超时

## 开放问题(实施时裁决)

1. WHERE 相对时间(`-7d` 等 duration)首版是否支持
2. TASK 输出——Collectanea 无独立 task 概念 → v1 不做
3. `metadata.*` 索引策略:按需表达式 GIN vs 全列 jsonb GIN
4. 同 DQL 结果缓存 → v1 不缓存
5. dataview 结果被其他笔记引用/导出 → 不做
