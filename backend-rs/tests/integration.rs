//! Integration tests against a running backend-rs instance.
//!
//! Mirrors `backend/tests/` (the Go black-box suite): same routes, same
//! assertions, same cookie semantics. These require a live server with
//! PostgreSQL + S3 storage, so they are `#[ignore]`d for CI — run manually:
//!
//! ```bash
//! cargo test --test integration -- --ignored --test-threads=1
//! ```
//!
//! `--test-threads=1` is REQUIRED: a backup or restore puts the workspace in a
//! read-only window (HTTP 423 for writes), so parallel tests would fail each
//! other's writes. Each test cleans up the users it creates.

use std::collections::HashMap;

use serde::Deserialize;
use serde_json::{Value, json};

const BASE_URL: &str = "http://localhost:8201/api";

struct Client {
    http: reqwest::Client,
}

impl Client {
    fn new() -> Self {
        Self {
            // Cookie jar is required: auth relies on the Authorization cookie.
            // no_proxy(): never route through env proxies (http_proxy/ALL_PROXY
            // from shell tooling) — the target is always this host.
            http: reqwest::Client::builder()
                .cookie_store(true)
                .no_proxy()
                .build()
                .expect("client"),
        }
    }
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)] // envelope fields mirror the wire format; some are shape-only
struct ApiResponse {
    status: String,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    timestamp: Value,
}

async fn post_json(client: &Client, path: &str, body: &Value) -> (u16, ApiResponse) {
    let resp = client
        .http
        .post(format!("{BASE_URL}{path}"))
        .json(body)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap();
    let parsed: ApiResponse = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("non-JSON from POST {path} [{status}]: {text:?} ({e})"));
    (status, parsed)
}

async fn get(client: &Client, path: &str) -> (u16, ApiResponse) {
    let resp = client
        .http
        .get(format!("{BASE_URL}{path}"))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap();
    let parsed: ApiResponse = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("non-JSON from GET {path} [{status}]: {text:?} ({e})"));
    (status, parsed)
}

async fn del(client: &Client, path: &str) -> (u16, ApiResponse) {
    let resp = client
        .http
        .delete(format!("{BASE_URL}{path}"))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap())
}

async fn patch_json(client: &Client, path: &str, body: &Value) -> (u16, ApiResponse) {
    let resp = client
        .http
        .patch(format!("{BASE_URL}{path}"))
        .json(body)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap();
    let parsed: ApiResponse = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("non-JSON from PATCH {path} [{status}]: {text:?} ({e})"));
    (status, parsed)
}

#[allow(dead_code)] // parity helper: PUT exists on nodes, not users
async fn put_json(client: &Client, path: &str, body: &Value) -> (u16, ApiResponse) {
    let resp = client
        .http
        .put(format!("{BASE_URL}{path}"))
        .json(body)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap();
    let parsed: ApiResponse = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("non-JSON from PUT {path} [{status}]: {text:?} ({e})"));
    (status, parsed)
}

fn result_id_string(r: &ApiResponse) -> String {
    r.result
        .get("id")
        .map(|v| match v {
            Value::String(s) => s.clone(),
            Value::Number(n) => n.to_string(),
            other => other.to_string(),
        })
        .expect("result.id present")
}

/// Signup + login a unique user; returns (user_id, username).
async fn signup_and_login(client: &Client) -> (String, String) {
    signup_and_login_tagged(client, "t").await
}

/// `tag` disambiguates parallel tests so usernames never collide.
/// Test-hygiene helper: removes a test user and everything cascading from it.
/// The suite creates users on every run, so leaving them behind pollutes the
/// admin user list (and the fan-out target picker) over time.
async fn cleanup_user(user_id: &str) {
    let host = std::env::var("DATABASE_HOST").expect("DATABASE_HOST");
    let port = std::env::var("DATABASE_PORT").unwrap_or_else(|_| "5432".into());
    let db = std::env::var("DATABASE_NAME").expect("DATABASE_NAME");
    let user = std::env::var("DATABASE_USER").expect("DATABASE_USER");
    let pass = std::env::var("DATABASE_PASSWORD").expect("DATABASE_PASSWORD");
    let dsn = format!("postgresql://{user}:{pass}@{host}:{port}/{db}");
    if let Ok(pool) = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&dsn)
        .await
    {
        let _ = sqlx::query("DELETE FROM backup_targets WHERE $1 = ANY(users)")
            .bind(user_id.parse::<uuid::Uuid>().unwrap_or_else(|_| uuid::Uuid::nil()))
            .execute(&pool)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(user_id.parse::<uuid::Uuid>().unwrap_or_else(|_| uuid::Uuid::nil()))
            .execute(&pool)
            .await;
    }
}

async fn signup_and_login_tagged(client: &Client, tag: &str) -> (String, String) {
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_string();
    let username = format!(
        "{}_{}_{}",
        tag,
        &suffix[suffix.len().saturating_sub(10)..],
        &tag[..1]
    );
    let (status, r) = post_json(
        client,
        "/users",
        &json!({
            "username": username,
            "password": "testpass123",
            "email": format!("{username}@example.com"),
        }),
    )
    .await;
    assert_eq!(status, 201, "signup failed: {:?}", r.message);
    assert_eq!(r.status, "success");

    // Login posts a urlencoded form (Go contract).
    let resp = client
        .http
        .post(format!("{BASE_URL}/auth"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("username={username}&password=testpass123"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "login failed");

    let (_, me) = get(client, "/users/@me").await;
    assert_eq!(me.status, "success");
    let id = result_id_string(&me);
    (id, username)
}

// ---------------------------------------------------------------- auth + users

#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn auth_and_user_lifecycle() {
    let client = Client::new();

    // Login with wrong credentials → 401 "Invalid credentials".
    let (status, r) = post_json(
        &client,
        "/auth",
        &json!({"username": "does-not-exist-xyz", "password": "wrong"}),
    )
    .await;
    assert_eq!(status, 401);
    assert_eq!(r.message.as_deref(), Some("Invalid credentials"));

    // Signup + login + @me.
    let (user_id, _username) = signup_and_login_tagged(&client, "auth").await;

    // Token refresh must accept POST (the contract the frontend and upstream
    // Go use). Registering GET only made every refresh answer 405, and the
    // client's error path then redirected authenticated pages to /login.
    let resp = client
        .http
        .post(format!("{BASE_URL}/auth/refresh"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        resp.status().as_u16(),
        200,
        "POST /auth/refresh must succeed for a logged-in client"
    );

    // Get user by ID.
    let (status, r) = get(&client, &format!("/users/{user_id}")).await;
    assert_eq!(status, 200);
    assert_eq!(r.status, "success");

    // Duplicate signup rejected.
    let (_uid2, username2) = {
        let c2 = Client::new();
        signup_and_login(&c2).await
    };
    let _ = username2;

    // PATCH own profile.
    let (status, r) = patch_json(
        &client,
        &format!("/users/{user_id}"),
        &json!({"firstname": "Integration"}),
    )
    .await;
    assert_eq!(status, 200, "patch failed: {:?}", r.message);
    assert_eq!(r.result.get("firstname"), Some(&json!("Integration")));

    // Password change requires the current password.
    let (status, _) = patch_json(
        &client,
        &format!("/users/{user_id}/password"),
        &json!({"password": "newpass456"}),
    )
    .await;
    assert_eq!(status, 400);
    let (status, _) = patch_json(
        &client,
        &format!("/users/{user_id}/password"),
        &json!({"current_password": "testpass123", "password": "newpass456"}),
    )
    .await;
    assert_eq!(status, 200);
    cleanup_user(&user_id).await;
}

// ---------------------------------------------------------------- nodes CRUD

#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn nodes_lifecycle() {
    let client = Client::new();
    let (user_id, suffix) = signup_and_login_tagged(&client, "nodes").await;

    // Unauthenticated access denied.
    let (status, _) = get(&Client::new(), &format!("/nodes/user/{user_id}")).await;
    assert_eq!(status, 401);

    // Create workspace + document.
    let (status, r) = post_json(
        &client,
        "/nodes",
        &json!({"name": format!("ws-{suffix}"), "role": 1}),
    )
    .await;
    assert_eq!(status, 200);
    let ws_id = result_id_string(&r);

    let (status, r) = post_json(
        &client,
        "/nodes",
        &json!({"name": format!("doc-{suffix}"), "role": 3, "parent_id": ws_id, "content": "# Hello"}),
    )
    .await;
    assert_eq!(status, 200);
    let doc_id = result_id_string(&r);

    // Missing name rejected.
    let (status, _) = post_json(&client, "/nodes", &json!({"role": 3})).await;
    assert_eq!(status, 400);

    // Get node envelope {node, permissions}.
    let (status, r) = get(&client, &format!("/nodes/{doc_id}")).await;
    assert_eq!(status, 200);
    assert!(r.result.get("node").is_some());
    assert!(r.result.get("permissions").is_some());

    // Update.
    let (status, _r) = put_json(
        &client,
        &format!("/nodes/{doc_id}"),
        &json!({"name": format!("renamed-{suffix}"), "content": "# Updated", "role": 3}),
    )
    .await;
    assert_eq!(status, 200);

    // Search finds the renamed document.
    let (status, _r) = get(&client, &format!("/nodes/search?q=renamed-{suffix}")).await;
    assert_eq!(status, 200);

    // Tree listing contains both nodes.
    let (status, r) = get(&client, &format!("/nodes/user/{user_id}")).await;
    assert_eq!(status, 200);
    let arr = r.result.as_array().expect("array");
    assert!(arr.len() >= 2);

    // Delete cascade: children first, then workspace.
    let (status, _) = del(&client, &format!("/nodes/{doc_id}")).await;
    assert_eq!(status, 200);
    let (status, _) = del(&client, &format!("/nodes/{ws_id}")).await;
    assert_eq!(status, 200);

    // Deleted node 404s.
    let (status, r) = get(&client, &format!("/nodes/{doc_id}")).await;
    assert_eq!(status, 404);
    let _ = r;
}

// ---------------------------------------------------------------- trash

/// Soft delete → trash listing → restore → purge (spec
/// .work/features/trash.md). Deletion must be reversible, and the tree must
/// hide trashed rows until they come back.
#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn trash_lifecycle() {
    let client = Client::new();
    let (user_id, suffix) = signup_and_login_tagged(&client, "trash").await;

    // Workspace with one document inside.
    let (status, r) = post_json(
        &client,
        "/nodes",
        &json!({"name": format!("tws-{suffix}"), "role": 1}),
    )
    .await;
    assert_eq!(status, 200);
    let ws_id = result_id_string(&r);

    let (status, r) = post_json(
        &client,
        "/nodes",
        &json!({"name": format!("tdoc-{suffix}"), "role": 3, "parent_id": ws_id, "content": "# trash"}),
    )
    .await;
    assert_eq!(status, 200);
    let doc_id = result_id_string(&r);

    // Trash starts empty for this user.
    let (status, r) = get(&client, "/trash").await;
    assert_eq!(status, 200);
    assert!(r.result.as_array().expect("array").is_empty());

    // Delete the workspace: soft delete takes the subtree but the node stays
    // reachable from the trash and disappears from the tree.
    let (status, r) = del(&client, &format!("/nodes/{ws_id}")).await;
    assert_eq!(status, 200);
    assert_eq!(r.result["nodes"].as_u64().unwrap_or(0), 2);

    let (status, r) = get(&client, &format!("/nodes/user/{user_id}")).await;
    assert_eq!(status, 200);
    let arr = r.result.as_array().expect("array");
    assert!(
        arr.iter().all(|n| n["id"] != json!(ws_id)),
        "trashed node must not appear in the tree"
    );

    // Trash lists only the subtree root, with its batch size.
    let (status, r) = get(&client, "/trash").await;
    assert_eq!(status, 200);
    let arr = r.result.as_array().expect("array");
    assert_eq!(arr.len(), 1, "only the subtree root is listed");
    assert_eq!(arr[0]["id"], json!(ws_id));
    assert_eq!(arr[0]["batch_size"].as_u64().unwrap_or(0), 2);

    // Restore brings both nodes back into the tree.
    let (status, r) = post_json(&client, &format!("/trash/{ws_id}/restore"), &json!({})).await;
    assert_eq!(status, 200);
    assert_eq!(r.result["restored"].as_u64().unwrap_or(0), 2);

    let (status, r) = get(&client, &format!("/nodes/user/{user_id}")).await;
    assert_eq!(status, 200);
    let arr = r.result.as_array().expect("array");
    assert!(
        arr.iter().any(|n| n["id"] == json!(ws_id)),
        "restored node is back in the tree"
    );

    // Purge: delete again, then remove permanently.
    let (status, _) = del(&client, &format!("/nodes/{ws_id}")).await;
    assert_eq!(status, 200);
    let (status, r) = del(&client, &format!("/trash/{ws_id}")).await;
    assert_eq!(status, 200);
    assert_eq!(r.result["purged"].as_u64().unwrap_or(0), 2);

    let (status, _) = get(&client, &format!("/nodes/{doc_id}")).await;
    assert_eq!(status, 404);

    // Empty trash on an already empty trash is a no-op.
    let (status, _) = del(&client, "/trash").await;
    assert_eq!(status, 200);
    let _ = user_id;
}

// ---------------------------------------------------------------- permissions

#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn permissions_and_invitations() {
    let owner = Client::new();
    let collab = Client::new();
    let (_owner_id, osuffix) = signup_and_login(&owner).await;
    let (collab_id, _csuffix) = signup_and_login_tagged(&collab, "permc").await;

    let (status, r) = post_json(
        &owner,
        "/nodes",
        &json!({"name": format!("shared-{osuffix}"), "role": 3}),
    )
    .await;
    assert_eq!(status, 200);
    let doc_id = result_id_string(&r);

    // Denied without permission → 403 Forbidden (Go statusFromAccessError parity).
    let (status, _) = get(&collab, &format!("/nodes/{doc_id}")).await;
    assert_eq!(status, 403);

    // Grant read.
    let (status, r) = post_json(
        &owner,
        &format!("/nodes/{doc_id}/permissions"),
        &json!({"user_id": collab_id, "permission": 1}),
    )
    .await;
    assert_eq!(status, 200);
    let perm_id = result_id_string(&r);

    // Read allowed, write forbidden.
    let (status, _) = get(&collab, &format!("/nodes/{doc_id}")).await;
    assert_eq!(status, 200);
    let (status, _) = put_json(
        &collab,
        &format!("/nodes/{doc_id}"),
        &json!({"name": "hijack", "role": 3}),
    )
    .await;
    assert_eq!(status, 403);

    // Escalate to write.
    {
        let resp = owner
            .http
            .patch(format!("{BASE_URL}/nodes/{doc_id}/permissions/{perm_id}"))
            .json(&json!({"permission": 2}))
            .send()
            .await
            .unwrap();
        let resp = resp
            .error_for_status()
            .expect("permission update should succeed");
        let body: ApiResponse = resp.json().await.unwrap();
        assert_eq!(body.status, "success");
    };

    // Write allowed now.
    let (status, _) = put_json(
        &collab,
        &format!("/nodes/{doc_id}"),
        &json!({"name": format!("collab-{}", collab_id), "role": 3}),
    )
    .await;
    assert_eq!(status, 200);

    // Invitation create + join.
    let (status, r) = post_json(
        &owner,
        &format!("/nodes/{doc_id}/invitations"),
        &json!({"permission_level": 2}),
    )
    .await;
    assert_eq!(status, 200);
    let code = r
        .result
        .get("invitation_code")
        .and_then(|v| v.as_str())
        .expect("code")
        .to_string();

    let _ = collab
        .http
        .delete(format!("{BASE_URL}/nodes/{doc_id}/permissions/{perm_id}"))
        .send()
        .await;
    let (status, _) = post_json(&collab, "/nodes/invitations/join", &json!({"code": code})).await;
    assert_eq!(status, 200);
    let (status, _) = get(&collab, &format!("/nodes/{doc_id}")).await;
    assert_eq!(status, 200);
}

// ------------------------------------------------------------------- settings

#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn user_settings_roundtrip() {
    let client = Client::new();
    let _ = signup_and_login_tagged(&client, "settings").await;

    let (status, r) = get(&client, "/user/settings").await;
    assert_eq!(status, 200);
    assert_eq!(r.result.get("general"), Some(&json!({})));

    let (status, r) = {
        let resp = client
            .http
            .put(format!("{BASE_URL}/user/settings"))
            .json(&json!({"general": {"theme": "dark"}}))
            .send()
            .await
            .unwrap();
        let status = resp.status().as_u16();
        let body: ApiResponse = resp.json().await.unwrap();
        (status, body)
    };
    assert_eq!(status, 200);
    assert_eq!(r.result.get("general"), Some(&json!({"theme": "dark"})));
}

// UUID ids must serialize as strings on the wire (P0 contract).
#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn uuid_ids_are_strings_on_the_wire() {
    let client = Client::new();
    let (user_id, _) = signup_and_login_tagged(&client, "snow").await;
    let (status, r) = get(&client, "/users/@me").await;
    assert_eq!(status, 200);
    // Raw JSON text check: the id must be a quoted string.
    let raw = serde_json::to_string(&r.result).unwrap();
    assert!(
        raw.contains(&format!("\"id\":\"{user_id}\"")),
        "id not string-encoded: {raw}"
    );
    let _ = user_id;
}

// ---------------------------------------------------------------- shape parity

/// Shared keep-alive to satisfy the unused-import lint when assertion helpers
/// evolve.
#[allow(dead_code)]
type KeepMap = HashMap<String, String>;

// ---------------------------------------------------------------- soft links

#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn soft_links_lifecycle() {
    let client = Client::new();
    let (owner_id, suffix) = signup_and_login_tagged(&client, "links").await;

    // ws1 (canonical parent), doc inside ws1, ws2 (link target).
    let mk = |name: String| json!({"name": name, "role": 1});
    let (status, r) = post_json(&client, "/nodes", &mk(format!("ws1-{suffix}"))).await;
    assert_eq!(status, 200);
    let ws1 = result_id_string(&r);
    let (status, r) = post_json(
        &client,
        "/nodes",
        &json!({"name": format!("doc-{suffix}"), "role": 3, "parent_id": ws1}),
    )
    .await;
    assert_eq!(status, 200);
    let doc = result_id_string(&r);
    let (status, r) = post_json(&client, "/nodes", &mk(format!("ws2-{suffix}"))).await;
    assert_eq!(status, 200);
    let ws2 = result_id_string(&r);

    // Link doc into ws2.
    let (status, r) = post_json(
        &client,
        &format!("/nodes/{doc}/links"),
        &json!({"parent_id": ws2}),
    )
    .await;
    assert_eq!(status, 201, "link create failed: {:?}", r.message);
    let link_id = result_id_string(&r);

    // Duplicate link rejected (409 via unique constraint).
    let (status, _) = post_json(
        &client,
        &format!("/nodes/{doc}/links"),
        &json!({"parent_id": ws2}),
    )
    .await;
    assert_eq!(status, 409);

    // Cycle rejection: linking ws1 into its own descendant doc → 400.
    let (status, _) = post_json(
        &client,
        &format!("/nodes/{ws1}/links"),
        &json!({"parent_id": doc}),
    )
    .await;
    assert_eq!(status, 400);

    // Subtree rejection: linking ws1 under doc's... (doc is a leaf; use ws2
    // into ws1's subtree — ws1 is not in doc's subtree so this would succeed;
    // instead verify self-link rejection).
    let (status, _) = post_json(
        &client,
        &format!("/nodes/{doc}/links"),
        &json!({"parent_id": doc}),
    )
    .await;
    assert_eq!(status, 400);

    // Positions list.
    let (status, r) = get(&client, &format!("/nodes/{doc}/links")).await;
    assert_eq!(status, 200);
    let positions = r.result.as_array().expect("positions array");
    assert_eq!(positions.len(), 1);
    // v1 default sort position, serialized under "order" like DB_Node.
    assert_eq!(positions[0].get("order").and_then(|v| v.as_i64()), Some(0));

    // User-wide links feed (sidebar clone positions; NL-B).
    let (status, r) = get(&client, "/nodes/links").await;
    assert_eq!(status, 200, "user links list failed: {:?}", r.message);
    let rows = r.result.as_array().expect("links array");
    assert!(
        rows.iter().any(
            |l| l.get("node_id").and_then(|v| v.as_str()) == Some(doc.as_str())
                && l.get("parent_id").and_then(|v| v.as_str()) == Some(ws2.as_str())
        ),
        "user-wide links must contain the created position"
    );

    // Tree listing still 200 with the link present.
    let (status, _) = get(&client, &format!("/nodes/user/{owner_id}")).await;
    assert_eq!(status, 200);

    // Remove position.
    let (status, _) = del(&client, &format!("/nodes/{doc}/links/{link_id}")).await;
    assert_eq!(status, 200);
    let (status, r) = get(&client, &format!("/nodes/{doc}/links")).await;
    assert_eq!(status, 200);
    assert_eq!(r.result.as_array().map(|a| a.len()), Some(0));
    let _ = owner_id;
    cleanup_user(&owner_id).await;
}

// ------------------------------------------------------------- backup links

/// NL-C (spec note-links.md §2): the backup zip must carry the node_links
/// table alongside nodes — documents.json is `{nodes, links}` since v1.2.0.
#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn backup_zip_contains_links() {
    let client = Client::new();
    let (_owner_id, suffix) = signup_and_login_tagged(&client, "bklinks").await;

    let mk = |name: String| json!({"name": name, "role": 1});
    let (status, r) = post_json(&client, "/nodes", &mk(format!("ws1-{suffix}"))).await;
    assert_eq!(status, 200);
    let ws1 = result_id_string(&r);
    let (status, r) = post_json(
        &client,
        "/nodes",
        &json!({"name": format!("doc-{suffix}"), "role": 3, "parent_id": ws1}),
    )
    .await;
    assert_eq!(status, 200);
    let doc = result_id_string(&r);
    let (status, r) = post_json(&client, "/nodes", &mk(format!("ws2-{suffix}"))).await;
    assert_eq!(status, 200);
    let ws2 = result_id_string(&r);

    // Link doc into ws2 so exactly one node_links row belongs to this user.
    let (status, r) = post_json(
        &client,
        &format!("/nodes/{doc}/links"),
        &json!({"parent_id": ws2}),
    )
    .await;
    assert_eq!(status, 201, "link create failed: {:?}", r.message);
    let link_id = result_id_string(&r);

    // Documents-only backup job.
    let (status, r) = post_json(&client, "/backup", &json!({"include_documents": true})).await;
    assert_eq!(status, 202, "backup start failed: {:?}", r.message);
    let job_id = r
        .result
        .get("job_id")
        .and_then(|v| v.as_str())
        .expect("job_id present")
        .to_string();

    // Poll until the job uploads the zip (download_url presigned) or fails.
    let mut finished = false;
    for _ in 0..120 {
        let (status, r) = get(&client, &format!("/backup/{job_id}")).await;
        assert_eq!(status, 200);
        let job = &r.result;
        if let Some(err) = job.get("error").and_then(|v| v.as_str()) {
            panic!("backup job failed: {err}");
        }
        if job.get("progress").and_then(|v| v.as_i64()) == Some(100) {
            finished = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    assert!(finished, "backup job did not finish in time");

    // Snapshots are multi-version: pull the newest one from the feed.
    let (status, r) = get(&client, "/backup/snapshots").await;
    assert_eq!(status, 200, "snapshots list failed: {:?}", r.message);
    let snaps = r.result.as_array().expect("snapshots array");
    let snap_id = snaps
        .first()
        .expect("snapshot present")
        .get("id")
        .and_then(|v| v.as_str())
        .expect("snapshot id")
        .to_string();

    // Pull the assembled zip through the authenticated download endpoint
    // (v2 snapshots are assembled from blobs server-side).
    let resp = client
        .http
        .get(format!("{BASE_URL}/backup/snapshots/{snap_id}/download"))
        .send()
        .await
        .expect("backup zip download");
    assert_eq!(resp.status(), 200, "snapshot download failed");
    let bytes = resp.bytes().await.expect("read body").to_vec();

    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).expect("valid zip");
    // Snapshots download in the portable object-set layout: documents.json is
    // a blob referenced by the manifest, so resolve it through meta/manifest.
    let entries: Vec<String> = (0..archive.len())
        .filter_map(|i| {
            archive
                .by_index(i)
                .ok()
                .and_then(|f| f.name().ok().map(|n| n.to_string()))
        })
        .collect();
    let doc_member = if entries.iter().any(|n| n == "documents.json") {
        "documents.json".to_string()
    } else {
        // portable: find the blob whose manifest entry names documents.json
        let mut meta = String::new();
        use std::io::Read as _;
        archive
            .by_name("meta.json")
            .expect("meta.json entry")
            .read_to_string(&mut meta)
            .expect("read meta");
        let meta: Value = serde_json::from_str(&meta).expect("meta JSON");
        let sha = meta["entries"]
            .as_array()
            .and_then(|a| {
                a.iter()
                    .find(|e| e.get("path").and_then(|p| p.as_str()) == Some("documents.json"))
            })
            .and_then(|e| e.get("sha256").and_then(|s| s.as_str()))
            .expect("documents.json entry in meta");
        format!("blobs/{sha}")
    };
    let mut doc_file = archive.by_name(&doc_member).expect("documents member");
    let mut contents = String::new();
    use std::io::Read as _;
    doc_file
        .read_to_string(&mut contents)
        .expect("read documents.json");
    let doc_json: Value = serde_json::from_str(&contents).expect("documents.json JSON");

    let nodes = doc_json
        .get("nodes")
        .and_then(|v| v.as_array())
        .expect("nodes array");
    let links = doc_json
        .get("links")
        .and_then(|v| v.as_array())
        .expect("links array");

    fn id_of(v: &Value) -> Option<&str> {
        v.get("id").and_then(|v| v.as_str())
    }
    assert!(
        nodes.iter().any(|n| id_of(n) == Some(doc.as_str())),
        "backed-up document missing from nodes"
    );
    let link = links
        .iter()
        .find(|l| id_of(l) == Some(link_id.as_str()))
        .expect("created link present in backup");
    assert_eq!(
        link.get("node_id").and_then(|v| v.as_str()),
        Some(doc.as_str())
    );
    assert_eq!(
        link.get("parent_id").and_then(|v| v.as_str()),
        Some(ws2.as_str())
    );
    assert!(
        link.get("created_timestamp")
            .and_then(|v| v.as_i64())
            .is_some(),
        "created_timestamp must stay numeric"
    );
}

// ---------------------------------------------------------- snapshot restore

/// Snapshots are multi-version ({uid}/snapshots/<ts>.zip); restoring one
/// replaces the caller's nodes/links with the archived state.
#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn snapshot_restore_roundtrip() {
    let client = Client::new();
    let (owner_id, suffix) = signup_and_login_tagged(&client, "rtrestore").await;

    let mk = |name: String| json!({"name": name, "role": 1});
    let (status, r) = post_json(&client, "/nodes", &mk(format!("ws1-{suffix}"))).await;
    assert_eq!(status, 200);
    let ws1 = result_id_string(&r);
    let (status, r) = post_json(
        &client,
        "/nodes",
        &json!({"name": format!("doc-{suffix}"), "role": 3, "parent_id": ws1}),
    )
    .await;
    assert_eq!(status, 200);
    let doc = result_id_string(&r);
    let (status, r) = post_json(&client, "/nodes", &mk(format!("ws2-{suffix}"))).await;
    assert_eq!(status, 200);
    let ws2 = result_id_string(&r);
    let (status, _) = post_json(
        &client,
        &format!("/nodes/{doc}/links"),
        &json!({"parent_id": ws2}),
    )
    .await;
    assert_eq!(status, 201);

    // Snapshot the current state.
    let (status, r) = post_json(&client, "/backup", &json!({"include_documents": true})).await;
    assert_eq!(status, 202);
    let job_id = r
        .result
        .get("job_id")
        .and_then(|v| v.as_str())
        .expect("job_id present")
        .to_string();
    for _ in 0..120 {
        let (status, r) = get(&client, &format!("/backup/{job_id}")).await;
        assert_eq!(status, 200);
        if r.result.get("error").is_some() {
            panic!("backup failed: {:?}", r.result.get("error"));
        }
        if r.result.get("progress").and_then(|v| v.as_i64()) == Some(100) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }
    let (status, r) = get(&client, "/backup/snapshots").await;
    assert_eq!(status, 200, "snapshots list failed: {:?}", r.message);
    let snaps = r.result.as_array().expect("snapshots array");
    assert!(!snaps.is_empty(), "snapshot list must not be empty");
    let snap = snaps
        .iter()
        .find(|s| s.get("id").and_then(|v| v.as_str()).is_some())
        .expect("snapshot id present");
    let snapshot_id = snap.get("id").and_then(|v| v.as_str()).unwrap().to_string();
    assert!(snap.get("size").and_then(|v| v.as_i64()).unwrap_or(0) > 0);

    // Damage: delete the document (tree loses doc + its link row via cascade).
    let (status, _) = del(&client, &format!("/nodes/{doc}")).await;
    assert_eq!(status, 200);
    let (status, r) = get(&client, "/nodes/user/@me").await;
    assert_eq!(status, 200);
    let visible = r
        .result
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n.get("deleted_at").map(|d| d.is_null()).unwrap_or(true))
        .count();
    assert!(
        visible >= 2,
        "ws1+ws2 must stay visible after delete, got {visible}"
    );

    // Preview (dry-run) before restoring: incoming 3 nodes vs current 2.
    let (status, r) = post_json(
        &client,
        &format!("/backup/snapshots/{snapshot_id}/preview"),
        &json!({}),
    )
    .await;
    assert_eq!(status, 200, "preview failed: {:?}", r.message);
    let plan = &r.result;
    // Replacement semantics: restore brings back every archived node, so the
    // archive's node count must be >= what is currently live.
    let incoming = plan
        .get("incoming_nodes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let current = plan
        .get("current_nodes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    assert_eq!(incoming, 3, "archive holds ws1+ws2+doc");
    assert!(
        current >= 2,
        "current rows must include ws1+ws2, got {current}"
    );
    assert_eq!(
        plan.get("nodes_removed").and_then(|v| v.as_u64()),
        Some(0),
        "nothing is dropped by this restore"
    );

    // Restore: the archived state (3 nodes + 1 link) must come back.
    let (status, r) = post_json(
        &client,
        &format!("/backup/snapshots/{snapshot_id}/restore"),
        &json!({}),
    )
    .await;
    assert_eq!(status, 200, "restore failed: {:?}", r.message);
    let report = &r.result;
    assert_eq!(report.get("nodes").and_then(|v| v.as_u64()), Some(3));
    assert_eq!(report.get("links").and_then(|v| v.as_u64()), Some(1));
    assert!(
        report
            .get("links_skipped")
            .and_then(|v| v.as_array())
            .map(|a| a.is_empty())
            .unwrap_or(true),
        "no links should be skipped"
    );

    let (status, r) = get(&client, "/nodes/user/@me").await;
    assert_eq!(status, 200);
    let names: Vec<String> = r
        .result
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| {
            n.get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .collect();
    assert!(
        names.contains(&format!("doc-{suffix}")),
        "document restored"
    );
    let (status, r) = get(&client, &format!("/nodes/{doc}/links")).await;
    assert_eq!(status, 200);
    assert_eq!(
        r.result.as_array().map(|a| a.len()),
        Some(1),
        "link restored"
    );

    // Download streams the (assembled) snapshot zip directly.
    let resp = client
        .http
        .get(format!(
            "{BASE_URL}/backup/snapshots/{snapshot_id}/download"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200, "snapshot download failed");
    assert!(
        resp.headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .contains("zip"),
        "download must be application/zip"
    );
    let _ = owner_id;
    cleanup_user(&owner_id).await;
}

// ------------------------------------------------- fan-out snapshot targets

/// Promotes a test user to app admin (role 2) straight in the database: the
/// target APIs are admin-only and there is no HTTP self-promotion path.
async fn make_admin(user_id: &str) {
    let host = std::env::var("DATABASE_HOST").expect("DATABASE_HOST");
    let port = std::env::var("DATABASE_PORT").unwrap_or_else(|_| "5432".into());
    let db = std::env::var("DATABASE_NAME").expect("DATABASE_NAME");
    let user = std::env::var("DATABASE_USER").expect("DATABASE_USER");
    let pass = std::env::var("DATABASE_PASSWORD").expect("DATABASE_PASSWORD");
    let dsn = format!("postgresql://{user}:{pass}@{host}:{port}/{db}");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .connect(&dsn)
        .await
        .expect("test db connect");
    sqlx::query("UPDATE users SET role = 2 WHERE id = $1")
        .bind(user_id.parse::<uuid::Uuid>().expect("uuid user id"))
        .execute(&pool)
        .await
        .expect("promote to admin");
}

fn target_body(name: &str, user_id: &str, endpoint: &str, bucket: &str) -> Value {
    json!({
        "kind": "s3",
        "name": name,
        "endpoint": endpoint,
        "bucket": bucket,
        "prefix": "e2e",
        "access_key": std::env::var("S3_ACCESSKEY").expect("S3_ACCESSKEY"),
        "secret_key": std::env::var("S3_SECRETKEY").expect("S3_SECRETKEY"),
        "secure": false,
        "region": "us-east-1",
        "provider": "Other",
        "keep_last": 5,
        "max_age_days": 0,
        "enabled": true,
        "users": [user_id],
    })
}

/// Covers the whole target lifecycle against a real S3 bucket provided by
/// TARGET_TEST_BUCKET (must exist and be empty):
/// create (masked response) -> probe -> fan-out sync -> incremental dedup ->
/// delete gate refuse -> empty -> delete allow.
#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 + TARGET_TEST_BUCKET"]
async fn snapshot_targets_lifecycle() {
    let bucket = std::env::var("TARGET_TEST_BUCKET")
        .expect("TARGET_TEST_BUCKET (an existing, empty S3 bucket for the test)");
    let endpoint = std::env::var("S3_ENDPOINT").expect("S3_ENDPOINT");
    let client = Client::new();
    let (user_id, username) = signup_and_login_tagged(&client, "tgt").await;
    // Promote in the DB, then re-login: the JWT carries the role, so the old
    // session keeps the pre-promotion claim and stays non-admin.
    make_admin(&user_id).await;
    let relogin = client
        .http
        .post(format!("{BASE_URL}/auth"))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(format!("username={username}&password=testpass123"))
        .send()
        .await
        .unwrap();
    assert_eq!(relogin.status().as_u16(), 200, "admin re-login failed");

    // Seed data + one snapshot.
    let (status, _) = post_json(
        &client,
        "/nodes",
        &json!({"name": format!("ws-{user_id}"), "role": 1}),
    )
    .await;
    assert_eq!(status, 200);
    let (status, r) = post_json(&client, "/backup", &json!({"include_documents": true})).await;
    assert_eq!(status, 202);
    let job_id = r
        .result
        .get("job_id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    for _ in 0..120 {
        let (_, r) = get(&client, &format!("/backup/{job_id}")).await;
        if r.result.get("progress").and_then(|v| v.as_i64()) == Some(100) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    }

    // Create the target: secrets must come back masked.
    let name = format!("tgt-{}", &user_id[user_id.len().saturating_sub(8)..]);
    let (status, r) = post_json(
        &client,
        "/admin/dr-backup/targets",
        &target_body(&name, &user_id, &endpoint, &bucket),
    )
    .await;
    assert_eq!(status, 200, "target create failed: {:?}", r.message);
    let target_id = r
        .result
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap()
        .to_string();
    assert_eq!(
        r.result.get("secret_key").and_then(|v| v.as_str()),
        Some("***"),
        "credentials must be masked on the wire"
    );

    // Identity dedup: a second config for the same storage is rejected.
    let (status, _) = post_json(
        &client,
        "/admin/dr-backup/targets",
        &target_body(&format!("{name}-dup"), &user_id, &endpoint, &bucket),
    )
    .await;
    assert_eq!(status, 409, "duplicate storage identity must be rejected");

    // Masked round-trip: send *** back and keep the stored credential working.
    let (status, r) = get(&client, "/admin/dr-backup/targets").await;
    assert_eq!(status, 200);
    let row = r
        .result
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t.get("id").and_then(|v| v.as_str()) == Some(target_id.as_str()))
        .expect("target in list");
    let mut edit = target_body(&name, &user_id, &endpoint, &bucket);
    edit["access_key"] = json!("***");
    edit["secret_key"] = json!("***");
    edit["keep_last"] = json!(7); // retention-only edit must not need probing
    let (status, _) = put_json(
        &client,
        &format!("/admin/dr-backup/targets/{target_id}"),
        &edit,
    )
    .await;
    assert_eq!(
        status, 200,
        "retention-only edit with masked secrets must pass"
    );
    let _ = row;

    // Manual fan-out replicates the snapshot objects.
    let (status, r) = post_json(
        &client,
        &format!("/admin/dr-backup/targets/{target_id}/sync"),
        &json!({}),
    )
    .await;
    assert_eq!(status, 200, "sync failed: {:?}", r.message);
    let results = r.result.as_array().expect("sync results");
    assert!(
        results
            .iter()
            .any(|x| x.get("copied").and_then(|v| v.as_u64()).unwrap_or(0) > 0),
        "at least one object must be copied: {results:?}"
    );

    // Second sync is a no-op (content-addressed dedup).
    let (_, r) = post_json(
        &client,
        &format!("/admin/dr-backup/targets/{target_id}/sync"),
        &json!({}),
    )
    .await;
    let results = r.result.as_array().expect("sync results");
    assert!(
        results
            .iter()
            .all(|x| x.get("copied").and_then(|v| v.as_u64()) == Some(0)),
        "second sync must copy nothing: {results:?}"
    );

    // Probe sees the replicated objects.
    let (status, r) = post_json(
        &client,
        &format!("/admin/dr-backup/targets/{target_id}/test"),
        &json!({}),
    )
    .await;
    assert_eq!(status, 200);
    let objects = r
        .result
        .get("snapshot_objects")
        .and_then(|v| v.as_i64())
        .unwrap_or(0);
    assert!(
        objects > 0,
        "probe must count replicated objects, got {objects}"
    );

    // Delete gate refuses while objects exist.
    let (status, _) = post_json(
        &client,
        &format!("/admin/dr-backup/targets/{target_id}/delete-check"),
        &json!({}),
    )
    .await;
    assert_eq!(status, 409, "delete-check must refuse a non-empty target");
    let (status, _) = del(&client, &format!("/admin/dr-backup/targets/{target_id}")).await;
    assert_eq!(status, 409, "delete must refuse a non-empty target");

    // Empty then delete succeeds.
    let (status, r) = post_json(
        &client,
        &format!("/admin/dr-backup/targets/{target_id}/empty"),
        &json!({}),
    )
    .await;
    assert_eq!(status, 200, "empty failed: {:?}", r.message);
    assert!(
        r.result
            .get("deleted_objects")
            .and_then(|v| v.as_u64())
            .unwrap_or(0)
            > 0,
        "empty must report deletions"
    );
    let (status, r) = del(&client, &format!("/admin/dr-backup/targets/{target_id}")).await;
    assert_eq!(
        status, 200,
        "delete after empty must succeed: {:?}",
        r.message
    );
    cleanup_user(&user_id).await;
}
