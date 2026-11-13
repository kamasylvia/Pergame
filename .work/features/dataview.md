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

## 开放问题(2026-11-13 实施裁决)

1. WHERE 相对时间 ✅做:`-7d/-1M/-12h/-30m/-45s/-2w`(M=30 天);`now_ms - duration` 化为时间戳比较。
2. TASK 输出——v1 不做(Pergame 无 task 概念)。
3. `metadata.*` 索引:现有 `GIN(metadata jsonb_path_ops)` 够用,`->>` 比较数据量小无妨;v1 不加索引。
4. 结果缓存——v1 不缓存。
5. 被引用/导出——不做。

## 实施记录(2026-11-13)

- 后端:`src/dql.rs`(lexer+递归下降+planner,白名单字段/深度 32/LIMIT 硬顶 100)+`src/routes/dql.rs`(`POST /api/nodes/query`,仅本人节点,5s 超时,4096 字符上限)+`repo/nodes::{resolve_folder_path,dql_query}`(路径逐段解析,role 1/2;sqlx `AssertSqlSafe` 已审计)。
- 前端:`helpers/markdown/dataview.ts`(```dataview /```index 原捕获插件)+`helpers/dataview-render.ts`(mount 期 fetch 渲染,index 预设=`LIST FROM 当前目录`)+`ContentCompiled` 挂载+`markdown.scss` 样式。链接走 `data-internal-link` 复用路由跳转。
- 范围裁决:shared/public 跨用户查询 v1 不做(仅本人节点);tag 匹配容空格(`REPLACE(tags,' ','')`)。
