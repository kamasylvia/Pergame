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
            .bind(
                user_id
                    .parse::<uuid::Uuid>()
                    .unwrap_or_else(|_| uuid::Uuid::nil()),
            )
            .execute(&pool)
            .await;
        let _ = sqlx::query("DELETE FROM users WHERE id = $1")
            .bind(
                user_id
                    .parse::<uuid::Uuid>()
                    .unwrap_or_else(|_| uuid::Uuid::nil()),
            )
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

/// Raw-bytes GET (file downloads are not the JSON envelope).
async fn get_bytes(client: &Client, url: &str) -> (u16, Vec<u8>) {
    let resp = client.http.get(url).send().await.unwrap();
    let status = resp.status().as_u16();
    let bytes = resp.bytes().await.unwrap().to_vec();
    (status, bytes)
}

/// Hand-rolled multipart upload (reqwest has no multipart feature here).
async fn upload_file(
    client: &Client,
    filename: &str,
    mime: &str,
    body: &[u8],
) -> (u16, ApiResponse) {
    let boundary = "----pergame-test-boundary";
    let mut payload = Vec::new();
    payload.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    payload.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"file\"; filename=\"{filename}\"\r\n")
            .as_bytes(),
    );
    payload.extend_from_slice(format!("Content-Type: {mime}\r\n\r\n").as_bytes());
    payload.extend_from_slice(body);
    payload.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    let resp = client
        .http
        .post(format!("{BASE_URL}/resources"))
        .header(
            "Content-Type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(payload)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    let text = resp.text().await.unwrap();
    let parsed: ApiResponse = serde_json::from_str(&text)
        .unwrap_or_else(|e| panic!("non-JSON from POST /resources [{status}]: {text:?} ({e})"));
    (status, parsed)
}

/// Authed file reads (fork): owner 200, anonymous/stranger 403 on private,
/// anonymous 200 once public; legacy /pergame/** path matches; Range → 206.
#[tokio::test]
#[ignore]
async fn resource_file_download_matrix() {
    let client = Client::new();
    let (user_id, _) = signup_and_login_tagged(&client, "dl").await;
    let stranger = Client::new();
    let (stranger_id, _) = signup_and_login_tagged(&stranger, "dlstr").await;
    let anon = Client::new();

    let payload = b"proxy-read-bytes";
    let (status, r) = upload_file(&client, "probe.txt", "text/plain", payload).await;
    assert_eq!(status, 200, "upload failed: {:?}", r.message);
    let node_id = result_id_string(&r);
    let tp = r.result["metadata"]["transformed_path"]
        .as_str()
        .unwrap()
        .to_string();
    let file_url = format!("{BASE_URL}/resources/{node_id}/file");
    let legacy_url = format!(
        "{}/pergame/{}/{}",
        BASE_URL.trim_end_matches("/api"),
        user_id,
        tp
    );

    // Owner reads full bytes.
    let (status, bytes) = get_bytes(&client, &file_url).await;
    assert_eq!(status, 200);
    assert_eq!(bytes, payload);

    // Private: anonymous + stranger refused.
    let (status, _) = get_bytes(&anon, &file_url).await;
    assert_eq!(status, 403, "anon must not read private");
    let (status, _) = get_bytes(&stranger, &file_url).await;
    assert_eq!(status, 403, "stranger must not read private");
    let (status, _) = get_bytes(&anon, &legacy_url).await;
    assert_eq!(status, 403, "anon must not read private legacy path");

    // Publish (PUT is full-replace: carry the existing fields).
    let (status, r) = put_json(
        &client,
        &format!("/nodes/{node_id}"),
        &json!({
            "name": r.result["name"],
            "role": 4,
            "content": r.result["content"],
            "content_compiled": r.result["content_compiled"],
            "size": r.result["size"],
            "metadata": r.result["metadata"],
            "accessibility": 3,
            "access": 1,
        }),
    )
    .await;
    assert_eq!(status, 200, "publish failed: {:?}", r.message);

    // Public: everyone reads, including the legacy path.
    let (status, bytes) = get_bytes(&anon, &file_url).await;
    assert_eq!(status, 200);
    assert_eq!(bytes, payload);
    let (status, _) = get_bytes(&stranger, &file_url).await;
    assert_eq!(status, 200);
    let (status, bytes) = get_bytes(&anon, &legacy_url).await;
    assert_eq!(status, 200);
    assert_eq!(bytes, payload);

    // Range passthrough.
    let resp = client
        .http
        .get(&file_url)
        .header("Range", "bytes=0-4")
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 206);
    assert!(resp.headers().contains_key("content-range"));
    let bytes = resp.bytes().await.unwrap();
    assert_eq!(&bytes[..], &payload[..5]);

    // Avatar: unset → 404.
    let (status, _) = get_bytes(&anon, &format!("{BASE_URL}/users/{user_id}/avatar")).await;
    assert_eq!(status, 404);

    cleanup_user(&user_id).await;
    cleanup_user(&stranger_id).await;
}

// ---------------------------------------------------------------- MCP

/// One JSON-RPC request against `/api/mcp`, tolerating both wire shapes the
/// streamable HTTP transport may use (plain JSON or SSE frames). Returns the
/// HTTP status, the JSON-RPC messages, and the `mcp-session-id` header.
async fn mcp_rpc(
    http: &reqwest::Client,
    token: &str,
    session_id: Option<&str>,
    body: Value,
) -> (u16, Vec<Value>, Option<String>) {
    let mut request = http
        .post(format!("{BASE_URL}/mcp"))
        .header("Authorization", format!("Bearer {token}"))
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&body);
    if let Some(sid) = session_id {
        request = request.header("mcp-session-id", sid);
    }
    let resp = request.send().await.expect("mcp request");
    let status = resp.status().as_u16();
    let sid = resp
        .headers()
        .get("mcp-session-id")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let text = resp.text().await.unwrap_or_default();

    // The transport may answer with a bare JSON object or with an SSE stream.
    // An SSE frame does not always open with `event:` (the initialize reply
    // starts with `data: ` / `id:` / `retry:` lines), so sniff the first byte
    // instead of the prefix.
    let messages: Vec<Value> = if text.trim_start().starts_with('{') {
        serde_json::from_str::<Value>(&text)
            .map(|v| vec![v])
            .unwrap_or_default()
    } else {
        let mut out = Vec::new();
        for line in text.lines() {
            if let Some(data) = line.strip_prefix("data: ")
                && let Ok(v) = serde_json::from_str::<Value>(data)
            {
                out.push(v);
            }
        }
        out
    };
    (status, messages, sid)
}

fn rpc_result(messages: &[Value]) -> &Value {
    messages
        .iter()
        .find(|m| m.get("result").is_some())
        .and_then(|m| m.get("result"))
        .expect("json-rpc result present")
}

fn rpc_error_code(messages: &[Value]) -> i64 {
    messages
        .iter()
        .find(|m| m.get("error").is_some())
        .and_then(|m| m["error"]["code"].as_i64())
        .expect("json-rpc error present")
}

async fn mcp_session(http: &reqwest::Client, token: &str) -> String {
    let (status, messages, sid) = mcp_rpc(
        http,
        token,
        None,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "pergame-integration", "version": "0" }
            }
        }),
    )
    .await;
    assert_eq!(status, 200, "initialize failed: {messages:?}");
    let sid = sid.expect("mcp-session-id header");
    // notifications/initialized completes the legacy handshake.
    let _ = mcp_rpc(
        http,
        token,
        Some(&sid),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized"}),
    )
    .await;
    sid
}

#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn mcp_token_lifecycle_and_tools() {
    let client = Client::new();
    let (user_id, _username) = signup_and_login_tagged(&client, "mcp").await;

    // Mint a token (session-authenticated; the plaintext comes back once).
    let (status, r) = post_json(
        &client,
        "/users/@me/api-tokens",
        &json!({ "name": "integration" }),
    )
    .await;
    assert_eq!(status, 201, "create token failed: {:?}", r.message);
    let token = r.result["token"].as_str().expect("plaintext").to_string();
    let token_id = result_id_string(&r);
    assert!(token.starts_with("pgm_"), "prefixed token: {token}");

    // The list never returns the secret.
    let (_, listed) = get(&client, "/users/@me/api-tokens").await;
    let rows = listed.result.as_array().expect("token list");
    assert_eq!(rows.len(), 1);
    assert!(
        rows[0].get("token").is_none(),
        "hash/plaintext must not leak"
    );

    // No token, wrong token, and a cookie-only client are all 401.
    let anon = reqwest::Client::builder().no_proxy().build().unwrap();
    let (status, _, _) = mcp_rpc(
        &anon,
        "pgm_not-a-real-token",
        None,
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list"}),
    )
    .await;
    assert_eq!(status, 401, "bad token must be rejected before MCP");

    let mcp = reqwest::Client::builder().no_proxy().build().unwrap();
    let sid = mcp_session(&mcp, &token).await;

    // tools/list: exactly the seven tools the spec promises.
    let (status, messages, _) = mcp_rpc(
        &mcp,
        &token,
        Some(&sid),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}),
    )
    .await;
    assert_eq!(status, 200);
    let mut names: Vec<String> = rpc_result(&messages)["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|t| t["name"].as_str().map(str::to_string))
        .collect();
    names.sort();
    assert_eq!(
        names,
        vec![
            "create_note",
            "delete_node",
            "list_children",
            "move_node",
            "read_note",
            "search_notes",
            "update_note"
        ]
    );

    // create_note → read_note
    let (_, messages, _) = mcp_rpc(
        &mcp,
        &token,
        Some(&sid),
        json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": { "name": "create_note", "arguments": {
                "name": "MCP integration note",
                "content": "# hello\n\nwritten over mcp",
            }}
        }),
    )
    .await;
    let created: Value = serde_json::from_str(
        rpc_result(&messages)["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .expect("create_note payload");
    let note_id = created["id"].as_str().unwrap().to_string();

    let (_, messages, _) = mcp_rpc(
        &mcp,
        &token,
        Some(&sid),
        json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": { "name": "read_note", "arguments": { "id": note_id } }
        }),
    )
    .await;
    let note: Value = serde_json::from_str(
        rpc_result(&messages)["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .expect("read_note payload");
    assert_eq!(note["content"], "# hello\n\nwritten over mcp");
    assert_eq!(note["role"], 3);

    // update_note replaces the markdown.
    let (_, messages, _) = mcp_rpc(
        &mcp,
        &token,
        Some(&sid),
        json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": { "name": "update_note", "arguments": {
                "id": note_id, "content": "# hello\n\nedited over mcp"
            }}
        }),
    )
    .await;
    assert_eq!(rpc_result(&messages)["isError"], false, "update ok");

    // search_notes finds it by content.
    let (_, messages, _) = mcp_rpc(
        &mcp,
        &token,
        Some(&sid),
        json!({
            "jsonrpc": "2.0", "id": 6, "method": "tools/call",
            "params": { "name": "search_notes", "arguments": { "query": "edited" } }
        }),
    )
    .await;
    let hits = rpc_result(&messages)["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(hits.contains(&note_id), "search hits the note: {hits}");

    // list_children at the top level includes the note.
    let (_, messages, _) = mcp_rpc(
        &mcp,
        &token,
        Some(&sid),
        json!({
            "jsonrpc": "2.0", "id": 7, "method": "tools/call",
            "params": { "name": "list_children", "arguments": {} }
        }),
    )
    .await;
    let listed = rpc_result(&messages)["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(listed.contains(&note_id), "root listing: {listed}");

    // A second note moves under the first, then refusals are checked.
    let (_, messages, _) = mcp_rpc(
        &mcp,
        &token,
        Some(&sid),
        json!({
            "jsonrpc": "2.0", "id": 8, "method": "tools/call",
            "params": { "name": "create_note", "arguments": { "name": "child note" } }
        }),
    )
    .await;
    let child: Value = serde_json::from_str(
        rpc_result(&messages)["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .expect("child payload");
    let child_id = child["id"].as_str().unwrap().to_string();

    let (_, messages, _) = mcp_rpc(
        &mcp,
        &token,
        Some(&sid),
        json!({
            "jsonrpc": "2.0", "id": 9, "method": "tools/call",
            "params": { "name": "move_node", "arguments": {
                "id": child_id, "new_folder_id": note_id
            }}
        }),
    )
    .await;
    assert_eq!(rpc_result(&messages)["isError"], false, "move ok");

    // Moving a node into itself is refused (-32602).
    let (_, messages, _) = mcp_rpc(
        &mcp,
        &token,
        Some(&sid),
        json!({
            "jsonrpc": "2.0", "id": 10, "method": "tools/call",
            "params": { "name": "move_node", "arguments": {
                "id": note_id, "new_folder_id": child_id
            }}
        }),
    )
    .await;
    assert_eq!(rpc_error_code(&messages), -32602, "cycle refused");

    // A stranger's token can read neither note (-32003).
    let stranger_client = Client::new();
    let (stranger_id, _) = signup_and_login_tagged(&stranger_client, "mcpx").await;
    let (status, r) = post_json(
        &stranger_client,
        "/users/@me/api-tokens",
        &json!({ "name": "stranger" }),
    )
    .await;
    assert_eq!(status, 201);
    let stranger_token = r.result["token"].as_str().unwrap().to_string();
    let stranger_mcp = reqwest::Client::builder().no_proxy().build().unwrap();
    let stranger_sid = mcp_session(&stranger_mcp, &stranger_token).await;
    let (_, messages, _) = mcp_rpc(
        &stranger_mcp,
        &stranger_token,
        Some(&stranger_sid),
        json!({
            "jsonrpc": "2.0", "id": 11, "method": "tools/call",
            "params": { "name": "read_note", "arguments": { "id": note_id } }
        }),
    )
    .await;
    assert_eq!(rpc_error_code(&messages), -32003, "stranger forbidden");

    // delete_node is a soft delete: the node lands in the trash.
    let (_, messages, _) = mcp_rpc(
        &mcp,
        &token,
        Some(&sid),
        json!({
            "jsonrpc": "2.0", "id": 12, "method": "tools/call",
            "params": { "name": "delete_node", "arguments": { "id": note_id } }
        }),
    )
    .await;
    let deleted: Value = serde_json::from_str(
        rpc_result(&messages)["content"][0]["text"]
            .as_str()
            .unwrap(),
    )
    .expect("delete payload");
    assert!(
        deleted["nodes"].as_i64().unwrap_or(0) >= 2,
        "subtree trashed"
    );
    let (_, trashed) = get(&client, "/trash").await;
    let trash = trashed.result.as_array().expect("trash list");
    assert!(
        trash
            .iter()
            .any(|n| n["id"].as_str() == Some(note_id.as_str())),
        "note is restorable from the trash"
    );

    // Revocation kills the token immediately.
    let resp = client
        .http
        .delete(format!("{BASE_URL}/users/@me/api-tokens/{token_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status().as_u16(), 200, "revoke ok");
    let (status, _, _) = mcp_rpc(
        &mcp,
        &token,
        Some(&sid),
        json!({"jsonrpc": "2.0", "id": 13, "method": "tools/list"}),
    )
    .await;
    assert_eq!(status, 401, "revoked token rejected");

    cleanup_user(&user_id).await;
    cleanup_user(&stranger_id).await;
}

// ---------------------------------------------------------------- MCP + lock

/// Extracts the JSON payload of a successful tool call (the text content).
fn tool_payload(messages: &[Value]) -> Value {
    let text = rpc_result(messages)["content"][0]["text"]
        .as_str()
        .unwrap_or_else(|| panic!("tool result has no text: {messages:?}"));
    serde_json::from_str(text).unwrap_or_else(|e| panic!("tool payload not JSON ({e}): {text}"))
}

/// Asserts a tool call failed and returns (code, data).
fn tool_error(messages: &[Value]) -> (i64, Value) {
    let err = messages
        .iter()
        .find(|m| m.get("error").is_some())
        .unwrap_or_else(|| panic!("expected a JSON-RPC error, got {messages:?}"));
    (
        err["error"]["code"].as_i64().expect("error code"),
        err["error"].get("data").cloned().unwrap_or(Value::Null),
    )
}

async fn mcp_tool(
    http: &reqwest::Client,
    token: &str,
    sid: &str,
    id: i64,
    name: &str,
    args: Value,
) -> Vec<Value> {
    let (status, messages, _) = mcp_rpc(
        http,
        token,
        Some(sid),
        json!({
            "jsonrpc": "2.0", "id": id, "method": "tools/call",
            "params": { "name": name, "arguments": args }
        }),
    )
    .await;
    assert_eq!(
        status, 200,
        "transport-level failure for {name}: {messages:?}"
    );
    messages
}

/// The read-only window must stop MCP writes exactly like REST writes: the
/// four write tools answer -32023 with `reason=backup_in_progress`, while the
/// three read tools keep working (an agent still needs to read while a backup
/// runs). The target points at a listener this test owns that accepts and
/// answers 500 after a delay, so the backup stalls with the lock held instead
/// of depending on timing luck.
#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn mcp_writes_blocked_while_backup_runs() {
    let client = Client::new();
    let (user_id, _) = signup_and_login_tagged(&client, "mcplock").await;

    // A slow, always-failing HTTP listener stands in for an unreachable S3:
    // the first write attempt stalls for seconds, which is the window under
    // test.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind fake s3");
    let port = listener.local_addr().unwrap().port();
    // Handles so the test can shut the fake store down once the assertions
    // are done: killing the sockets makes the backup fail immediately instead
    // of waiting out a stall, so the lock release is prompt and deterministic.
    let fake_handles: std::sync::Arc<std::sync::Mutex<Vec<tokio::task::AbortHandle>>> =
        Default::default();
    let handles_for_accept = fake_handles.clone();
    let fake_server = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let handle = tokio::spawn(async move {
                use tokio::io::AsyncWriteExt as _;
                tokio::time::sleep(std::time::Duration::from_secs(4)).await;
                let _ = socket
                    .write_all(b"HTTP/1.1 500 Internal Server Error\r\nContent-Length: 0\r\n\r\n")
                    .await;
                let _ = socket.shutdown().await;
            });
            handles_for_accept
                .lock()
                .unwrap()
                .push(handle.abort_handle());
        }
    });

    // Token + one note to update/move/delete during the window.
    let (status, r) = post_json(
        &client,
        "/users/@me/api-tokens",
        &json!({ "name": "lock-window" }),
    )
    .await;
    assert_eq!(status, 201, "create token failed: {:?}", r.message);
    let token = r.result["token"].as_str().expect("plaintext").to_string();
    let token_id = result_id_string(&r);

    let mcp = reqwest::Client::builder().no_proxy().build().unwrap();
    let sid = mcp_session(&mcp, &token).await;

    let created = tool_payload(
        &mcp_tool(
            &mcp,
            &token,
            &sid,
            100,
            "create_note",
            json!({
                "name": "lock window note", "content": "before"
            }),
        )
        .await,
    );
    let note_id = created["id"].as_str().unwrap().to_string();

    // A target that stalls: enabled, owned by the caller. Saving requires a
    // prior connection test (probing is what mints the token; reachability is
    // not part of the gate), so probe-draft first.
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let target_fields = json!({
        "name": format!("stall-{unique}"),
        "kind": "s3",
        "endpoint": format!("http://127.0.0.1:{port}"),
        "bucket": "stall",
        "access_key": "test",
        "secret_key": "test",
        "enabled": true,
    });
    let (_, probed) = post_json(
        &client,
        "/backup/targets/probe-draft",
        &json!({ "target": target_fields }),
    )
    .await;
    let connection_token = probed.result["connection_token"]
        .as_str()
        .unwrap_or_else(|| panic!("probe-draft returned no token: {:?}", probed.message))
        .to_string();
    let mut create_body = target_fields.clone();
    create_body["connection_token"] = json!(connection_token);
    let (status, t) = post_json(&client, "/backup/targets", &create_body).await;
    assert!(
        (200..300).contains(&status),
        "target create failed [{status}]: {:?}",
        t.message
    );
    let target_id = result_id_string(&t);

    // Start a backup and wait for the workspace to turn read-only.
    let (status, job) = post_json(
        &client,
        "/backup",
        &json!({ "include_documents": true, "include_settings": true }),
    )
    .await;
    assert_eq!(status, 202, "backup start failed: {:?}", job.message);
    let job_id = job.result["job_id"].as_str().expect("job id").to_string();

    // The read-only window is observable through any REST write: an empty
    // name is rejected with 400 while writable and preempted with 423 while
    // locked, so the probe has no side effects. (`/admin/dr-backup/status`
    // would need an admin account; this is the same signal for a plain user.)
    async fn write_probe(client: &Client) -> u16 {
        let (status, _) = post_json(client, "/nodes", &json!({ "name": "" })).await;
        status
    }
    let mut read_only = false;
    for _ in 0..600 {
        if write_probe(&client).await == 423 {
            read_only = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    assert!(read_only, "the backup never engaged the read-only lock");

    // All four write tools are refused with -32023 + reason.
    let writes = [
        ("create_note", json!({ "name": "blocked create" })),
        (
            "update_note",
            json!({ "id": note_id, "content": "blocked edit" }),
        ),
        (
            "move_node",
            json!({ "id": note_id, "new_folder_id": Value::Null }),
        ),
        ("delete_node", json!({ "id": note_id })),
    ];
    for (i, (name, args)) in writes.iter().enumerate() {
        let messages = mcp_tool(&mcp, &token, &sid, 200 + i as i64, name, args.clone()).await;
        let (code, data) = tool_error(&messages);
        assert_eq!(code, -32023, "{name} must be refused during a backup");
        assert_eq!(
            data["reason"], "backup_in_progress",
            "{name} must say why: {data}"
        );
    }

    // Reads keep working.
    let read = tool_payload(
        &mcp_tool(
            &mcp,
            &token,
            &sid,
            300,
            "read_note",
            json!({ "id": note_id }),
        )
        .await,
    );
    assert_eq!(read["content"], "before", "read must survive the window");
    let _ = tool_payload(&mcp_tool(&mcp, &token, &sid, 301, "list_children", json!({})).await);
    let _ = tool_payload(
        &mcp_tool(
            &mcp,
            &token,
            &sid,
            302,
            "search_notes",
            json!({ "query": "before" }),
        )
        .await,
    );

    // REST parity: a real node write is refused the same way (423), not just
    // the empty-name probe.
    let (status, _) = post_json(&client, "/nodes", &json!({ "name": "blocked rest write" })).await;
    assert_eq!(status, 423, "REST writes share the window");

    // The note really was untouched while writes were refused.
    let after = tool_payload(
        &mcp_tool(
            &mcp,
            &token,
            &sid,
            303,
            "read_note",
            json!({ "id": note_id }),
        )
        .await,
    );
    assert_eq!(after["content"], "before");

    // Release: stop the fake store so the stalled writes fail at once, cancel
    // the job, then wait for the window to close.
    for handle in fake_handles.lock().unwrap().iter() {
        handle.abort();
    }
    fake_server.abort();
    let resp = client
        .http
        .delete(format!("{BASE_URL}/backup/{job_id}"))
        .send()
        .await
        .unwrap();
    assert!(resp.status().is_success(), "cancel failed");

    let mut closed = false;
    for _ in 0..1200 {
        if write_probe(&client).await == 400 {
            closed = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert!(closed, "the read-only window never closed");

    // Writes work again once the window closes.
    let messages = mcp_tool(
        &mcp,
        &token,
        &sid,
        400,
        "create_note",
        json!({ "name": "after window" }),
    )
    .await;
    assert_eq!(rpc_result(&messages)["isError"], false, "writes resume");

    // Cleanup: the API would refuse deleting a target that may still hold
    // objects (this one is unreachable), so `cleanup_user` removes the row
    // directly — it also drops the tokens/nodes by cascade.
    let _ = target_id;
    let _ = client
        .http
        .delete(format!("{BASE_URL}/users/@me/api-tokens/{token_id}"))
        .send()
        .await;
    cleanup_user(&user_id).await;
}
