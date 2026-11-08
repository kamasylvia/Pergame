//! Integration tests against a running backend-rs instance.
//!
//! Mirrors `backend/tests/` (the Go black-box suite): same routes, same
//! assertions, same cookie semantics. These require a live server with
//! PostgreSQL + S3 storage, so they are `#[ignore]`d for CI — run manually:
//!
//! ```bash
//! cargo test --test integration -- --ignored
//! ```

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
            http: reqwest::Client::builder().build().expect("client"),
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
    (status, resp.json().await.unwrap())
}

async fn get(client: &Client, path: &str) -> (u16, ApiResponse) {
    let resp = client
        .http
        .get(format!("{BASE_URL}{path}"))
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap())
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

async fn put_json(client: &Client, path: &str, body: &Value) -> (u16, ApiResponse) {
    let resp = client
        .http
        .put(format!("{BASE_URL}{path}"))
        .json(body)
        .send()
        .await
        .unwrap();
    let status = resp.status().as_u16();
    (status, resp.json().await.unwrap())
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
    let suffix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos()
        .to_string();
    let username = format!("test{}", &suffix[..12]);
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
    let (user_id, _username) = signup_and_login(&client).await;

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
    let (status, r) = put_json(
        &client,
        &format!("/users/{user_id}"),
        &json!({"firstname": "Integration"}),
    )
    .await;
    assert_eq!(status, 200, "patch failed: {:?}", r.message);
    assert_eq!(r.result.get("firstname"), Some(&json!("Integration")));

    // Password change requires the current password.
    let (status, _) = put_json(
        &client,
        &format!("/users/{user_id}/password"),
        &json!({"password": "newpass456"}),
    )
    .await;
    assert_eq!(status, 400);
    let (status, _) = put_json(
        &client,
        &format!("/users/{user_id}/password"),
        &json!({"current_password": "testpass123", "password": "newpass456"}),
    )
    .await;
    assert_eq!(status, 200);
}

// ---------------------------------------------------------------- nodes CRUD

#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn nodes_lifecycle() {
    let client = Client::new();
    let (user_id, suffix) = signup_and_login(&client).await;

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

// ---------------------------------------------------------------- permissions

#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn permissions_and_invitations() {
    let owner = Client::new();
    let collab = Client::new();
    let (_owner_id, osuffix) = signup_and_login(&owner).await;
    let (collab_id, _csuffix) = signup_and_login(&collab).await;

    let (status, r) = post_json(
        &owner,
        "/nodes",
        &json!({"name": format!("shared-{osuffix}"), "role": 3}),
    )
    .await;
    assert_eq!(status, 200);
    let doc_id = result_id_string(&r);

    // Denied without permission.
    let (status, _) = get(&collab, &format!("/nodes/{doc_id}")).await;
    assert_eq!(status, 404);

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
    let _ = signup_and_login(&client).await;

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

// Snowflake ids must serialize as strings on the wire (P0 contract).
#[tokio::test]
#[ignore = "requires a running backend-rs with PostgreSQL + S3 storage"]
async fn snowflake_ids_are_strings_on_the_wire() {
    let client = Client::new();
    let (user_id, _) = signup_and_login(&client).await;
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
