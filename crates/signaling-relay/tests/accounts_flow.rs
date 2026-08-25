use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use monoterminal_signaling_relay::{build_router_with_state, load_or_generate_signing_key, AppState, Database, PairingRateLimiter, SharedState};

static PORT_HINT: AtomicU64 = AtomicU64::new(0);

struct TestServer {
    base_url: String,
    _db_dir: tempfile::TempDir,
}

async fn spawn_server() -> TestServer {
    let db_dir = tempfile::tempdir().unwrap();
    let db_path = db_dir.path().join(format!(
        "accounts-{}.db",
        PORT_HINT.fetch_add(1, Ordering::Relaxed)
    ));

    let db = Arc::new(Database::new(&db_path).expect("db init"));
    let shared = Arc::new(SharedState {
        relay: AppState::new(),
        db,
        jwt_signing_key: Arc::new(load_or_generate_signing_key()),
        pairing_rate_limiter: PairingRateLimiter::default(),
    });

    let router = build_router_with_state(shared);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    TestServer {
        base_url: format!("http://{}", addr),
        _db_dir: db_dir,
    }
}

fn client() -> reqwest::Client {
    reqwest::Client::new()
}

fn extract_credential(body: &serde_json::Value) -> String {
    body.get("token").and_then(|v| v.as_str()).unwrap().to_string()
}

async fn signup(server: &TestServer, email: &str, password: &str) -> serde_json::Value {
    client()
        .post(format!("{}/api/signup", server.base_url))
        .json(&serde_json::json!({"email": email, "password": password}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

#[tokio::test]
async fn signup_login_and_authenticated_route_work() {
    let server = spawn_server().await;

    let signup_resp = client()
        .post(format!("{}/api/signup", server.base_url))
        .json(&serde_json::json!({"email": "alice@example.com", "password": "hunter22"}))
        .send()
        .await
        .unwrap();
    assert_eq!(signup_resp.status(), 200);
    let signup_body: serde_json::Value = signup_resp.json().await.unwrap();
    let cred_a = extract_credential(&signup_body);
    assert_eq!(signup_body["user"]["email"], "alice@example.com");

    let login_resp = client()
        .post(format!("{}/api/login", server.base_url))
        .json(&serde_json::json!({"email": "alice@example.com", "password": "hunter22"}))
        .send()
        .await
        .unwrap();
    assert_eq!(login_resp.status(), 200);
    let login_body: serde_json::Value = login_resp.json().await.unwrap();
    let cred_b = extract_credential(&login_body);

    for credential in [cred_a, cred_b] {
        let resp = client()
            .get(format!("{}/api/computers", server.base_url))
            .bearer_auth(&credential)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), 200);
    }
}

#[tokio::test]
async fn duplicate_signup_is_rejected() {
    let server = spawn_server().await;
    signup(&server, "bob@example.com", "password1").await;

    let resp = client()
        .post(format!("{}/api/signup", server.base_url))
        .json(&serde_json::json!({"email": "bob@example.com", "password": "password2"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);
}

#[tokio::test]
async fn login_with_wrong_password_is_rejected() {
    let server = spawn_server().await;
    signup(&server, "carol@example.com", "correcthorse").await;

    let resp = client()
        .post(format!("{}/api/login", server.base_url))
        .json(&serde_json::json!({"email": "carol@example.com", "password": "wrongpassword"}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

#[tokio::test]
async fn pairing_code_link_and_list_flow() {
    let server = spawn_server().await;
    let signup_body = signup(&server, "dave@example.com", "password1").await;
    let credential = extract_credential(&signup_body);

    let code_resp = client()
        .post(format!("{}/api/pairing-codes", server.base_url))
        .json(&serde_json::json!({"peer_id": "peer-dave"}))
        .send()
        .await
        .unwrap();
    assert_eq!(code_resp.status(), 200);
    let code_body: serde_json::Value = code_resp.json().await.unwrap();
    let code = code_body["code"].as_str().unwrap().to_string();

    let link_resp = client()
        .post(format!("{}/api/link", server.base_url))
        .bearer_auth(&credential)
        .json(&serde_json::json!({"code": code, "name": "Dave's Desktop"}))
        .send()
        .await
        .unwrap();
    assert_eq!(link_resp.status(), 200);

    let list_resp = client()
        .get(format!("{}/api/computers", server.base_url))
        .bearer_auth(&credential)
        .send()
        .await
        .unwrap();
    assert_eq!(list_resp.status(), 200);
    let list_body: serde_json::Value = list_resp.json().await.unwrap();
    let computers = list_body["computers"].as_array().unwrap();
    assert_eq!(computers.len(), 1);
    assert_eq!(computers[0]["peer_id"], "peer-dave");
    assert_eq!(computers[0]["online"], false);
    assert_eq!(computers[0]["name"], "Dave's Desktop");
}

#[tokio::test]
async fn consumed_or_expired_code_is_rejected() {
    let server = spawn_server().await;
    let signup_body = signup(&server, "erin@example.com", "password1").await;
    let credential = extract_credential(&signup_body);

    let code_resp = client()
        .post(format!("{}/api/pairing-codes", server.base_url))
        .json(&serde_json::json!({"peer_id": "peer-erin"}))
        .send()
        .await
        .unwrap();
    let code_body: serde_json::Value = code_resp.json().await.unwrap();
    let code = code_body["code"].as_str().unwrap().to_string();

    // First link consumes the code.
    let first = client()
        .post(format!("{}/api/link", server.base_url))
        .bearer_auth(&credential)
        .json(&serde_json::json!({"code": code}))
        .send()
        .await
        .unwrap();
    assert_eq!(first.status(), 200);

    // Reusing the same (now-consumed) code must fail.
    let second = client()
        .post(format!("{}/api/link", server.base_url))
        .bearer_auth(&credential)
        .json(&serde_json::json!({"code": code}))
        .send()
        .await
        .unwrap();
    assert_eq!(second.status(), 404);

    // A code that was never issued also fails.
    let unknown = client()
        .post(format!("{}/api/link", server.base_url))
        .bearer_auth(&credential)
        .json(&serde_json::json!({"code": "000000"}))
        .send()
        .await
        .unwrap();
    assert_eq!(unknown.status(), 404);
}

#[tokio::test]
async fn linking_peer_already_owned_by_another_user_is_rejected() {
    let server = spawn_server().await;

    let owner = signup(&server, "frank@example.com", "password1").await;
    let owner_cred = extract_credential(&owner);

    let code_resp = client()
        .post(format!("{}/api/pairing-codes", server.base_url))
        .json(&serde_json::json!({"peer_id": "peer-shared"}))
        .send()
        .await
        .unwrap();
    let code_body: serde_json::Value = code_resp.json().await.unwrap();
    let code = code_body["code"].as_str().unwrap().to_string();

    let link_resp = client()
        .post(format!("{}/api/link", server.base_url))
        .bearer_auth(&owner_cred)
        .json(&serde_json::json!({"code": code}))
        .send()
        .await
        .unwrap();
    assert_eq!(link_resp.status(), 200);

    // Someone else requests a fresh code for the SAME peer_id and tries to link it.
    let intruder = signup(&server, "gina@example.com", "password1").await;
    let intruder_cred = extract_credential(&intruder);

    let code_resp2 = client()
        .post(format!("{}/api/pairing-codes", server.base_url))
        .json(&serde_json::json!({"peer_id": "peer-shared"}))
        .send()
        .await
        .unwrap();
    let code_body2: serde_json::Value = code_resp2.json().await.unwrap();
    let code2 = code_body2["code"].as_str().unwrap().to_string();

    let conflict_resp = client()
        .post(format!("{}/api/link", server.base_url))
        .bearer_auth(&intruder_cred)
        .json(&serde_json::json!({"code": code2}))
        .send()
        .await
        .unwrap();
    assert_eq!(conflict_resp.status(), 409);
}

#[tokio::test]
async fn delete_computer_removes_it_and_rejects_other_users() {
    let server = spawn_server().await;
    let user_a = signup(&server, "henry@example.com", "password1").await;
    let cred_a = extract_credential(&user_a);
    let user_b = signup(&server, "irene@example.com", "password1").await;
    let cred_b = extract_credential(&user_b);

    let code_resp = client()
        .post(format!("{}/api/pairing-codes", server.base_url))
        .json(&serde_json::json!({"peer_id": "peer-henry"}))
        .send()
        .await
        .unwrap();
    let code_body: serde_json::Value = code_resp.json().await.unwrap();
    let code = code_body["code"].as_str().unwrap().to_string();

    let link_resp = client()
        .post(format!("{}/api/link", server.base_url))
        .bearer_auth(&cred_a)
        .json(&serde_json::json!({"code": code}))
        .send()
        .await
        .unwrap();
    let link_body: serde_json::Value = link_resp.json().await.unwrap();
    let computer_id = link_body["computer"]["id"].as_i64().unwrap();

    // User B (not the owner) cannot delete it.
    let forbidden = client()
        .delete(format!("{}/api/computers/{}", server.base_url, computer_id))
        .bearer_auth(&cred_b)
        .send()
        .await
        .unwrap();
    assert_eq!(forbidden.status(), 404);

    // Owner deletes it successfully.
    let ok = client()
        .delete(format!("{}/api/computers/{}", server.base_url, computer_id))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap();
    assert_eq!(ok.status(), 200);

    // Second GET no longer shows it.
    let list_resp = client()
        .get(format!("{}/api/computers", server.base_url))
        .bearer_auth(&cred_a)
        .send()
        .await
        .unwrap();
    let list_body: serde_json::Value = list_resp.json().await.unwrap();
    assert_eq!(list_body["computers"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn authenticated_routes_require_authorization_header() {
    let server = spawn_server().await;

    let list_resp = client()
        .get(format!("{}/api/computers", server.base_url))
        .send()
        .await
        .unwrap();
    assert_eq!(list_resp.status(), 401);

    let link_resp = client()
        .post(format!("{}/api/link", server.base_url))
        .json(&serde_json::json!({"code": "123456"}))
        .send()
        .await
        .unwrap();
    assert_eq!(link_resp.status(), 401);
}

#[tokio::test]
async fn pairing_code_rate_limit_kicks_in_after_five_requests() {
    let server = spawn_server().await;

    let mut last_status = reqwest::StatusCode::OK;
    for _ in 0..6 {
        let resp = client()
            .post(format!("{}/api/pairing-codes", server.base_url))
            .json(&serde_json::json!({"peer_id": "peer-rate-limited"}))
            .send()
            .await
            .unwrap();
        last_status = resp.status();
    }
    assert_eq!(last_status, 429);
}
