# Feature Spec — MCP Server

> 状态:待实施(随 Rust 重写)。跟踪:issue #3。需求源:用户 2026-11-07。

## 需求

把 Pergame 笔记库暴露为 MCP server,AI agent(Claude/Cursor 等)可读写笔记。

## 已定决策

| 项 | 决策 |
|---|---|
| 实施时机 | 随 Rust 重写,后端内置模块(非独立进程) |
| 能力面 | **读写全量**(search/list/read + create/update/move/delete) |
| 传输 | streamable HTTP,端点 `/api/mcp`(axum 集成) |
| 鉴权 | 每用户 **API token**(设置页生成/吊销;`Authorization: Bearer`),不复用 session |
| SDK | 官方 Rust SDK `rmcp`(tokio 系,与 axum 同栈) |

## 工具集草案(实施时定稿)

- `search_notes(query, limit?)` — 全文搜索(pg FTS)
- `list_children(folder_id?)` — 列子节点(含 id/name/role/updated)
- `read_note(id)` — 读 md 正文
- `create_note(folder_id, name, content?)`
- `update_note(id, content?, name?)`
- `move_node(id, new_folder_id)`(根 = null)
- `delete_node(id)` — 走现有权限/级联删除语义

权限:MCP 调用 = 该 token 所属用户身份,复用现有 permissions/actor 体系(不是超权通道)。

## 开放问题(实施时裁决)

- token 存储:users 表新列 vs 独立 `api_tokens` 表(倾向独立表,支持多 token + 撤销)
- 速率限制/审计日志(logs 表记 mcp 调用?)
- delete 是否要求二次确认参数(dry_run flag)
