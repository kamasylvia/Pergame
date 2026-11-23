//! MCP tool surface (spec `.work/features/mcp-server.md` § 工具集).
//!
//! Nine tools, all acting as the token owner through the normal permission
//! layer (`auth::guard`) — MCP is a second *entrance*, never a second policy:
//! - reads: `search_notes`, `list_children`, `read_note`, `read_resource`
//! - writes: `create_note`, `create_folder`, `update_note`, `move_node`,
//!   `delete_node`, `upload_resource`
//!
//! Writes reuse the REST semantics verbatim: creation validates the parent
//! with an Update check (falling back to the root), `delete_node` soft-deletes
//! into the trash, `move_node` only accepts a parent the caller may write and
//! refuses to move a node under itself or its own subtree. While a backup or
//! restore holds the workspace lock, writes answer -32023 and reads still work.

use rmcp::RoleServer;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{CallToolResult, ContentBlock, ServerCapabilities, ServerConfig};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, ServerHandler, tool, tool_handler, tool_router};
use serde::Deserialize;
use uuid::Uuid;

use crate::AppState;
use crate::auth::guard::{self, Actor, NodeAction};
use crate::repo::nodes as node_repo;

use super::{McpActor, actor_of, audit, locked_error, mcp_error};

/// MCP tool server (rmcp `ServerHandler`): 9 tools over the REST permission layer.
pub struct PergameMcp {
    state: AppState,
}

// ---------------------------------------------------------------- arguments

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
/// Arguments for `search_notes`.
pub struct SearchArgs {
    /// Full-text query (2+ characters; matches name, description, tags and
    /// content of documents).
    pub query: String,
    /// Maximum results (1-100, default 20).
    pub limit: Option<i32>,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
/// Arguments for `list_children`.
pub struct ListChildrenArgs {
    /// Folder id (workspace or category). Omit for the top level.
    pub folder_id: Option<String>,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
/// Arguments for `read_note`.
pub struct ReadNoteArgs {
    /// Node id.
    pub id: String,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
/// Arguments for `create_note`.
pub struct CreateNoteArgs {
    /// Parent folder id. Omit (or an unwritable id) to create at the top level.
    pub folder_id: Option<String>,
    /// Note title.
    pub name: String,
    /// Markdown source.
    pub content: Option<String>,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
/// Arguments for `update_note`.
pub struct UpdateNoteArgs {
    /// Node id.
    pub id: String,
    /// New markdown source (omit to leave unchanged).
    pub content: Option<String>,
    /// New title (omit to leave unchanged).
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
/// Arguments for `move_node`.
pub struct MoveNodeArgs {
    /// Node id to move.
    pub id: String,
    /// New parent folder id. Omit (or null) for the top level.
    pub new_folder_id: Option<String>,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
/// Arguments for `delete_node`.
pub struct DeleteNodeArgs {
    /// Node id. The subtree moves to the trash and stays restorable.
    pub id: String,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
/// Arguments for `read_resource`.
pub struct ReadResourceArgs {
    /// Resource (uploaded file) id.
    pub id: String,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
/// Arguments for `create_folder`.
pub struct CreateFolderArgs {
    /// Parent folder id. Omit (or an unwritable id) to create at the top level.
    pub folder_id: Option<String>,
    /// Folder name.
    pub name: String,
}

#[derive(Debug, Deserialize, rmcp::schemars::JsonSchema)]
/// Arguments for `upload_resource`.
pub struct UploadResourceArgs {
    /// Parent folder id. Omit (or an unwritable id) to upload at the top level.
    pub folder_id: Option<String>,
    /// File name with extension (drives text/code detection, same list as
    /// `read_resource`).
    pub filename: String,
    /// File content as UTF-8 text (no MCP-side size cap; quota gates apply).
    pub content: String,
    /// MIME type hint (default `application/octet-stream`).
    pub content_type: Option<String>,
}

// ---------------------------------------------------------------- helpers

fn parse_id(raw: &str) -> Result<Uuid, McpError> {
    Uuid::parse_str(raw.trim()).map_err(|_| {
        McpError::new(
            rmcp::model::ErrorCode::INVALID_PARAMS,
            format!("invalid id: {raw}"),
            None,
        )
    })
}

fn text<T: serde::Serialize>(value: &T) -> Result<CallToolResult, McpError> {
    let body = serde_json::to_string(value).map_err(|e| {
        McpError::new(
            rmcp::model::ErrorCode::INTERNAL_ERROR,
            format!("serialize failed: {e}"),
            None,
        )
    })?;
    Ok(CallToolResult::success(vec![ContentBlock::text(body)]))
}

impl PergameMcp {
    /// Build over shared app state (pools clone cheaply).
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    fn actor(&self, ctx: &RequestContext<RoleServer>) -> Result<(McpActor, Actor), McpError> {
        let mcp = actor_of(ctx)?;
        Ok((
            mcp,
            Actor {
                user_id: mcp.user_id,
                role: mcp.role,
            },
        ))
    }

    /// Write tools refuse while a backup/restore holds the workspace lock.
    async fn ensure_writable(&self) -> Result<(), McpError> {
        if crate::locks::global().is_read_only().await {
            return Err(locked_error());
        }
        Ok(())
    }

    /// Parent validation identical to `POST /api/nodes`: a parent the actor
    /// may update is kept, anything else falls back to the top level.
    async fn safe_parent(&self, actor: &Actor, parent: Option<Uuid>) -> Option<Uuid> {
        let pid = parent?;
        let row = node_repo::get_by_id(&self.state.db, pid)
            .await
            .ok()
            .flatten();
        let allowed = match row {
            Some(p) => guard::check_node_action(&self.state.db, actor, &p, NodeAction::Update)
                .await
                .map(|l| l != guard::PERM_NONE)
                .unwrap_or(false),
            None => false,
        };
        allowed.then_some(pid)
    }

    async fn load(
        &self,
        actor: &Actor,
        id: Uuid,
        action: NodeAction,
    ) -> Result<crate::models::NodeRow, McpError> {
        guard::require_node_action(&self.state.db, actor, id, action)
            .await
            .map_err(mcp_error)
    }

    fn write_from_row(row: &crate::models::NodeRow) -> node_repo::NodeWrite {
        node_repo::NodeWrite {
            parent_id: row.parent_id,
            name: row.name.clone(),
            description: row.description.clone(),
            tags: row.tags.clone(),
            role: row.role,
            color: row.color,
            icon: row.icon.clone(),
            thumbnail: row.thumbnail.clone(),
            theme: row.theme.clone(),
            accessibility: row.accessibility,
            access: row.access,
            display: row.display,
            node_order: row.node_order,
            content: row.content.clone(),
            content_compiled: row.content_compiled.clone(),
            size: row.size,
            metadata: row.metadata.clone(),
        }
    }
}

// ---------------------------------------------------------------- tools

#[tool_router]
impl PergameMcp {
    #[tool(
        description = "Full-text search across the caller's documents (name, description, tags, content). Returns id, name, parent_id, relevance and a content snippet."
    )]
    async fn search_notes(
        &self,
        Parameters(args): Parameters<SearchArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (_, actor) = self.actor(&ctx)?;
        let limit = args.limit.unwrap_or(20).clamp(1, 100);
        let rows = node_repo::search(&self.state.db, actor.user_id, &args.query, true, limit)
            .await
            .map_err(|e| mcp_error(crate::error::ApiError::from(e)))?;
        text(&rows)
    }

    #[tool(
        description = "List the direct children of a folder (workspace or category), or the top level when folder_id is omitted. Returns id, name, role, parent_id, updated_timestamp and access."
    )]
    async fn list_children(
        &self,
        Parameters(args): Parameters<ListChildrenArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (_, actor) = self.actor(&ctx)?;
        let parent = match args.folder_id.as_deref() {
            Some(raw) if !raw.trim().is_empty() => {
                let id = parse_id(raw)?;
                // Reading a folder must be allowed before listing below it.
                self.load(&actor, id, NodeAction::Read).await?;
                Some(id)
            }
            _ => None,
        };

        // Both sources already encode visibility (owned tree + granted tree),
        // so no per-child permission round trip is needed.
        let mut rows = node_repo::list_all(&self.state.db, actor.user_id)
            .await
            .map_err(|e| mcp_error(crate::error::ApiError::from(e)))?;
        let shared = node_repo::list_shared(&self.state.db, actor.user_id)
            .await
            .map_err(|e| mcp_error(crate::error::ApiError::from(e)))?;
        let seen: std::collections::HashSet<Uuid> = rows.iter().map(|n| n.id).collect();
        rows.extend(shared.into_iter().filter(|n| !seen.contains(&n.id)));

        let children: Vec<serde_json::Value> = rows
            .iter()
            .filter(|n| n.parent_id == parent)
            .map(|n| {
                serde_json::json!({
                    "id": n.id,
                    "name": n.name,
                    "role": n.role,
                    "parent_id": n.parent_id,
                    "access": n.access,
                    "updated_timestamp": n.updated_timestamp,
                })
            })
            .collect();
        text(&serde_json::json!({ "parent_id": parent, "children": children }))
    }

    #[tool(description = "Read one node: markdown content, name, role, parent_id and timestamps.")]
    async fn read_note(
        &self,
        Parameters(args): Parameters<ReadNoteArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (_, actor) = self.actor(&ctx)?;
        let node = self
            .load(&actor, parse_id(&args.id)?, NodeAction::Read)
            .await?;
        text(&serde_json::json!({
            "id": node.id,
            "name": node.name,
            "role": node.role,
            "parent_id": node.parent_id,
            "content": node.content,
            "description": node.description,
            "tags": node.tags,
            "access": node.access,
            "created_timestamp": node.created_timestamp,
            "updated_timestamp": node.updated_timestamp,
        }))
    }

    #[tool(
        description = "Read the text of an uploaded code/text resource (role-4 node): returns name, language and full UTF-8 content. Text is inferred from the file extension or a text/* mime; binary resources and bodies above 2 MiB are refused."
    )]
    async fn read_resource(
        &self,
        Parameters(args): Parameters<ReadResourceArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        const MAX_TEXT_BYTES: usize = 2 * 1024 * 1024;
        let (_, actor) = self.actor(&ctx)?;
        let node = self
            .load(&actor, parse_id(&args.id)?, NodeAction::Read)
            .await?;
        if node.role != 4 {
            return Err(McpError::new(
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "not a resource (use read_note for documents)",
                None,
            ));
        }
        let meta = node.metadata.clone().unwrap_or(serde_json::json!({}));
        let filename = meta
            .get("original_path")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(&node.name)
            .to_string();
        let filetype = meta
            .get("filetype")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        if !crate::utils::is_text_resource(&filename, filetype) {
            return Err(McpError::new(
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!("resource {filename} is not a text/code file"),
                None,
            ));
        }
        if let Some(size) = node.size
            && size as usize > MAX_TEXT_BYTES
        {
            return Err(McpError::new(
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!(
                    "resource larger than 2 MiB ({} bytes)",
                    node.size.unwrap_or(0)
                ),
                None,
            ));
        }
        let storage = self.state.storage.clone().ok_or_else(|| {
            McpError::new(
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "storage not configured",
                None,
            )
        })?;
        let bytes = match meta
            .get("content_sha256")
            .and_then(serde_json::Value::as_str)
        {
            Some(sha) => storage.get_blob(sha).await,
            None => {
                let key = meta
                    .get("transformed_path")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                storage.get_user_object(actor.user_id, &key).await
            }
        }
        // Storage errors are already ApiError; wrapping them again is a no-op
        // conversion and clippy denies it.
        .map_err(mcp_error)?;
        if bytes.len() > MAX_TEXT_BYTES {
            return Err(McpError::new(
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!("resource larger than 2 MiB ({} bytes)", bytes.len()),
                None,
            ));
        }
        let content = String::from_utf8(bytes).map_err(|_| {
            McpError::new(
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "resource is not valid UTF-8 text",
                None,
            )
        })?;
        let ext = crate::utils::file_ext(&filename);
        text(&serde_json::json!({
            "id": node.id,
            "name": node.name,
            "filename": filename,
            "language": if ext.is_empty() { serde_json::Value::Null } else { serde_json::Value::String(ext) },
            "content": content,
        }))
    }

    #[tool(
        description = "Create a document (markdown note) inside a folder; folder_id may be omitted for the top level."
    )]
    async fn create_note(
        &self,
        Parameters(args): Parameters<CreateNoteArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (mcp, actor) = self.actor(&ctx)?;
        self.ensure_writable().await?;

        let name = args.name.trim().to_string();
        if name.is_empty() {
            return Err(McpError::new(
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "name must not be empty",
                None,
            ));
        }
        let parent = match args.folder_id.as_deref() {
            Some(raw) if !raw.trim().is_empty() => {
                self.safe_parent(&actor, Some(parse_id(raw)?)).await
            }
            _ => None,
        };

        let id = crate::types::new_id();
        let now = node_repo::now_ms();
        let write = node_repo::NodeWrite {
            parent_id: parent,
            name: name.clone(),
            description: None,
            tags: None,
            role: 3, // document
            color: None,
            icon: None,
            thumbnail: None,
            theme: None,
            accessibility: None,
            access: 0,
            display: None,
            node_order: None,
            content: args.content,
            // No renderer here: the editor compiles on next open.
            content_compiled: None,
            size: None,
            metadata: None,
        };
        node_repo::insert(&self.state.db, id, actor.user_id, &write, now)
            .await
            .map_err(|e| mcp_error(crate::error::ApiError::from(e)))?;
        audit(
            &self.state.db,
            Some(actor.user_id),
            "mcp.tool.create_note",
            &format!("created node {id} (token {})", mcp.token_id),
        )
        .await;

        text(&serde_json::json!({
            "id": id,
            "name": name,
            "parent_id": parent,
            "created_timestamp": now,
        }))
    }
    #[tool(
        description = "Create a folder (category) inside a folder; folder_id may be omitted for the top level."
    )]
    async fn create_folder(
        &self,
        Parameters(args): Parameters<CreateFolderArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (mcp, actor) = self.actor(&ctx)?;
        self.ensure_writable().await?;

        let name = args.name.trim().to_string();
        if name.is_empty() {
            return Err(McpError::new(
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "name must not be empty",
                None,
            ));
        }
        let parent = match args.folder_id.as_deref() {
            Some(raw) if !raw.trim().is_empty() => {
                self.safe_parent(&actor, Some(parse_id(raw)?)).await
            }
            _ => None,
        };

        let id = crate::types::new_id();
        let now = node_repo::now_ms();
        let write = node_repo::NodeWrite {
            parent_id: parent,
            name: name.clone(),
            description: None,
            tags: None,
            role: 2, // folder
            color: None,
            icon: None,
            thumbnail: None,
            theme: None,
            accessibility: None,
            access: 0,
            display: None,
            node_order: None,
            content: None,
            content_compiled: None,
            size: None,
            metadata: None,
        };
        node_repo::insert(&self.state.db, id, actor.user_id, &write, now)
            .await
            .map_err(|e| mcp_error(crate::error::ApiError::from(e)))?;
        audit(
            &self.state.db,
            Some(actor.user_id),
            "mcp.tool.create_folder",
            &format!("created folder {id} (token {})", mcp.token_id),
        )
        .await;

        text(&serde_json::json!({
            "id": id,
            "name": name,
            "parent_id": parent,
            "created_timestamp": now,
        }))
    }


    #[tool(
        description = "Update a document's markdown content and/or title. Omitted fields stay unchanged. Requires write access."
    )]
    async fn update_note(
        &self,
        Parameters(args): Parameters<UpdateNoteArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (mcp, actor) = self.actor(&ctx)?;
        self.ensure_writable().await?;

        if args.content.is_none() && args.name.is_none() {
            return Err(McpError::new(
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "nothing to update: pass content and/or name",
                None,
            ));
        }
        let node = self
            .load(&actor, parse_id(&args.id)?, NodeAction::Update)
            .await?;

        let mut write = Self::write_from_row(&node);
        if let Some(name) = args.name {
            let name = name.trim().to_string();
            if name.is_empty() {
                return Err(McpError::new(
                    rmcp::model::ErrorCode::INVALID_PARAMS,
                    "name must not be empty",
                    None,
                ));
            }
            write.name = name;
        }
        if let Some(content) = args.content {
            write.content = Some(content);
            // Stale compiled HTML would render the old text until an editor
            // open; drop it so the next render is authoritative.
            write.content_compiled = None;
        }

        let now = node_repo::now_ms();
        node_repo::update(&self.state.db, node.id, &write, now)
            .await
            .map_err(|e| mcp_error(crate::error::ApiError::from(e)))?;
        audit(
            &self.state.db,
            Some(actor.user_id),
            "mcp.tool.update_note",
            &format!("updated node {} (token {})", node.id, mcp.token_id),
        )
        .await;

        text(&serde_json::json!({
            "id": node.id,
            "name": write.name,
            "parent_id": write.parent_id,
            "updated_timestamp": now,
        }))
    }

    #[tool(
        description = "Move a node to another folder (new_folder_id omitted or null = top level). Only the owner or an admin may move a node; a node cannot be moved into itself or its own subtree."
    )]
    async fn move_node(
        &self,
        Parameters(args): Parameters<MoveNodeArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (mcp, actor) = self.actor(&ctx)?;
        self.ensure_writable().await?;

        let node = self
            .load(&actor, parse_id(&args.id)?, NodeAction::Update)
            .await?;

        // REST keeps the old parent for non-owner editors; for an explicit
        // move request silently ignoring it would be worse than refusing.
        let level = guard::check_node_action(&self.state.db, &actor, &node, NodeAction::Update)
            .await
            .map_err(mcp_error)?;
        if node.user_id != actor.user_id && level < guard::PERM_OWNER {
            return Err(McpError::new(
                rmcp::model::ErrorCode(-32003),
                "only the owner can move this node",
                None,
            ));
        }
        let requested = match args.new_folder_id.as_deref() {
            Some(raw) if !raw.trim().is_empty() => Some(parse_id(raw)?),
            _ => None,
        };
        if let Some(target) = requested {
            if target == node.id {
                return Err(McpError::new(
                    rmcp::model::ErrorCode::INVALID_PARAMS,
                    "cannot move a node into itself",
                    None,
                ));
            }
            let subtree = node_repo::subtree_ids(&self.state.db, node.id)
                .await
                .map_err(|e| mcp_error(crate::error::ApiError::from(e)))?;
            if subtree.contains(&target) {
                return Err(McpError::new(
                    rmcp::model::ErrorCode::INVALID_PARAMS,
                    "cannot move a node into its own subtree",
                    None,
                ));
            }
        }
        let parent = self.safe_parent(&actor, requested).await;

        let mut write = Self::write_from_row(&node);
        write.parent_id = parent;
        let now = node_repo::now_ms();
        node_repo::update(&self.state.db, node.id, &write, now)
            .await
            .map_err(|e| mcp_error(crate::error::ApiError::from(e)))?;
        audit(
            &self.state.db,
            Some(actor.user_id),
            "mcp.tool.move_node",
            &format!(
                "moved node {} to {} (token {})",
                node.id,
                parent
                    .map(|p| p.to_string())
                    .unwrap_or_else(|| "root".into()),
                mcp.token_id
            ),
        )
        .await;

        text(&serde_json::json!({
            "id": node.id,
            "parent_id": parent,
            "updated_timestamp": now,
        }))
    }

    #[tool(
        description = "Upload a UTF-8 text/code file as a resource (role-4 node) under a folder. The filename extension must be a known code/text type (same list as read_resource); binaries are refused. Returns the resource id and its markdown embed URL."
    )]
    async fn upload_resource(
        &self,
        Parameters(args): Parameters<UploadResourceArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (mcp, actor) = self.actor(&ctx)?;
        self.ensure_writable().await?;

        let filename = args.filename.trim().to_string();
        if filename.is_empty() || filename.len() > 255 {
            return Err(McpError::new(
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "filename must be 1-255 characters",
                None,
            ));
        }
        let mime = args
            .content_type
            .unwrap_or_else(|| "application/octet-stream".to_string());
        if !crate::utils::is_text_resource(&filename, &mime) {
            return Err(McpError::new(
                rmcp::model::ErrorCode::INVALID_PARAMS,
                format!(
                    "{filename} is not a text/code file (known extension or text/* mime required)"
                ),
                None,
            ));
        }
        let bytes = args.content.as_bytes();
        let storage = self.state.storage.clone().ok_or_else(|| {
            McpError::new(
                rmcp::model::ErrorCode::INTERNAL_ERROR,
                "storage not configured",
                None,
            )
        })?;
        // Quota gates mirror POST /api/resources.
        let quota = crate::quota::for_user(&self.state.db, actor.user_id, actor.role).await;
        if let Some(max_file) = quota.max_file
            && bytes.len() as u64 > max_file
        {
            return Err(McpError::new(
                rmcp::model::ErrorCode::INVALID_PARAMS,
                "file too large or failed to get file",
                None,
            ));
        }
        if let Some(max_total) = quota.max_total {
            let total = node_repo::uploads_size(&self.state.db, actor.user_id)
                .await
                .map_err(|e| mcp_error(crate::error::ApiError::from(e)))?;
            if total as u64 + bytes.len() as u64 > max_total {
                return Err(McpError::new(
                    rmcp::model::ErrorCode::INVALID_PARAMS,
                    "total size of uploads exceeds the limit",
                    None,
                ));
            }
        }

        let parent = match args.folder_id.as_deref() {
            Some(raw) if !raw.trim().is_empty() => {
                self.safe_parent(&actor, Some(parse_id(raw)?)).await
            }
            _ => None,
        };

        // Node + blob layout mirrors POST /api/resources (code files never
        // take the imgbed branch, so no remote_url handling here).
        let id = crate::types::new_id();
        let ext: String = filename
            .rsplit_once('.')
            .map(|(_, e)| format!(".{e}"))
            .unwrap_or_default();
        let transformed_path = format!("{id}{ext}");
        let sha = crate::blobs::acquire(&self.state.db, &storage, bytes)
            .await
            .map_err(mcp_error)?;
        let metadata = serde_json::json!({
            "filetype": mime,
            "original_path": filename,
            "transformed_path": transformed_path,
            "content_sha256": sha,
        });
        let mut name = filename.clone();
        if name.len() > 50 {
            // Byte-boundary parity with REST, then trim to a char edge.
            name = name.chars().take(50).collect();
        }
        let write = node_repo::NodeWrite {
            parent_id: parent,
            name: name.clone(),
            description: None,
            tags: None,
            role: 4, // resource
            color: None,
            icon: None,
            thumbnail: None,
            theme: None,
            accessibility: Some(1),
            access: 0,
            display: None,
            node_order: None,
            content: Some(filename.clone()),
            content_compiled: Some(transformed_path.clone()),
            size: Some(bytes.len() as i64),
            metadata: Some(metadata),
        };
        let now = node_repo::now_ms();
        node_repo::insert(&self.state.db, id, actor.user_id, &write, now)
            .await
            .map_err(|e| mcp_error(crate::error::ApiError::from(e)))?;
        audit(
            &self.state.db,
            Some(actor.user_id),
            "mcp.tool.upload_resource",
            &format!("uploaded resource {id} ({filename}, token {})", mcp.token_id),
        )
        .await;

        text(&serde_json::json!({
            "id": id,
            "name": name,
            "filename": filename,
            "parent_id": parent,
            "size": bytes.len(),
            "embed_url": format!("/api/resources/{id}/file/{filename}"),
        }))
    }

    #[tool(
        description = "Delete a node and its subtree. This is a soft delete: everything lands in the trash and can be restored."
    )]
    async fn delete_node(
        &self,
        Parameters(args): Parameters<DeleteNodeArgs>,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        let (mcp, actor) = self.actor(&ctx)?;
        self.ensure_writable().await?;

        let node = self
            .load(&actor, parse_id(&args.id)?, NodeAction::Delete)
            .await?;
        let (deleted_at, nodes) =
            node_repo::soft_delete_subtree(&self.state.db, node.id, node_repo::now_ms())
                .await
                .map_err(|e| mcp_error(crate::error::ApiError::from(e)))?;
        audit(
            &self.state.db,
            Some(actor.user_id),
            "mcp.tool.delete_node",
            &format!(
                "trashed node {} ({nodes} nodes, token {})",
                node.id, mcp.token_id
            ),
        )
        .await;

        text(&serde_json::json!({ "deleted_at": deleted_at, "nodes": nodes }))
    }
}

#[tool_handler]
impl ServerHandler for PergameMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                "pergame",
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Pergame notes over MCP. Reads: search_notes, list_children, read_note, read_resource. \
             Writes (resolve to the API token's user under the normal permission rules): \
             create_note, create_folder, update_note, move_node, delete_node, upload_resource. \
             Deletion is a soft delete into the trash."
                    .to_string(),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_parse_trims_and_rejects() {
        let id = Uuid::now_v7();
        assert_eq!(parse_id(&format!("  {id}  ")).unwrap(), id);
        assert!(parse_id("nope").is_err());
        assert!(parse_id("").is_err());
    }

    #[test]
    fn text_wraps_json() {
        let out = text(&serde_json::json!({"a": 1})).unwrap();
        assert_eq!(out.content.len(), 1);
    }
}
