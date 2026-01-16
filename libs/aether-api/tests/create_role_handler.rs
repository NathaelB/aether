use std::{future::IntoFuture, sync::Arc};

use aether_api::{
    args::{Args, AuthArgs, DatabaseArgs, LogArgs, ServerArgs},
    handlers::roles::role_routes,
    state::AppState,
};
use aether_auth::{Identity, User};
use aether_core::{
    AetherService,
    auth::ports::MockAuthService,
    organisation::OrganisationId,
    role::ports::MockRoleRepository,
    role::{Role, RoleId},
    test_mocks,
};
use aether_permission::Permissions;
use axum::{
    Router,
    extract::Request,
    http::{StatusCode, header::AUTHORIZATION},
    middleware::{Next, from_fn},
};
use chrono::Utc;
use reqwest::Client;
use serde_json::json;
use uuid::Uuid;

fn build_state() -> AppState {
    let args = Args {
        log: LogArgs::default(),
        db: DatabaseArgs::default(),
        auth: AuthArgs {
            issuer: "http://localhost:8888/realms/aether".to_string(),
        },
        server: ServerArgs::default(),
    };
    let pool =
        sqlx::PgPool::connect_lazy("postgres://aether:aether@localhost/aether").expect("pool");
    let service = AetherService::new(pool);

    AppState {
        args: Arc::new(args),
        service,
    }
}

fn make_identity(roles: Vec<String>) -> Identity {
    Identity::User(User {
        id: "user-1".to_string(),
        username: "user".to_string(),
        email: None,
        name: None,
        roles,
    })
}

fn auth_mock(identity: Identity) -> MockAuthService {
    let mut auth = MockAuthService::new();
    let identity_clone = identity.clone();
    auth.expect_get_identity().times(1).returning(move |_| {
        let identity = identity_clone.clone();
        Box::pin(async move { Ok(identity) })
    });
    auth
}

fn build_router(
    state: AppState,
    auth: Option<Arc<MockAuthService>>,
    role_repo: Option<Arc<MockRoleRepository>>,
) -> Router {
    let router = role_routes(state.clone()).with_state(state);
    if auth.is_none() && role_repo.is_none() {
        return router;
    }

    router.layer(from_fn(move |req: Request, next: Next| {
        let auth = auth.clone();
        let role_repo = role_repo.clone();
        async move {
            let response = next.run(req);
            match (auth, role_repo) {
                (Some(auth), Some(role_repo)) => {
                    test_mocks::scope_auth_service(auth, async move {
                        test_mocks::scope_role_repository(role_repo, response).await
                    })
                    .await
                }
                (Some(auth), None) => test_mocks::scope_auth_service(auth, response).await,
                (None, Some(role_repo)) => {
                    test_mocks::scope_role_repository(role_repo, response).await
                }
                (None, None) => response.await,
            }
        }
    }))
}

async fn run_request<F, Fut, T>(router: Router, f: F) -> T
where
    F: FnOnce(Client, String) -> Fut,
    Fut: std::future::Future<Output = T>,
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener");
    let addr = listener.local_addr().expect("addr");
    let base_url = format!("http://{}", addr);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let server = axum::serve(listener, router).with_graceful_shutdown(async {
        let _ = shutdown_rx.await;
    });
    let server = server.into_future();

    let client = Client::new();
    let client_task = async move {
        let result = f(client, base_url).await;
        let _ = shutdown_tx.send(());
        result
    };

    let (result, _) = tokio::join!(client_task, server);
    result
}

#[tokio::test]
async fn create_role_returns_created_when_authorized() {
    let organisation_id = Uuid::new_v4();
    let identity = make_identity(vec!["admin".to_string()]);

    let auth = auth_mock(identity);

    let mut repo = MockRoleRepository::new();
    repo.expect_list_by_names().times(1).returning(|_, _| {
        let role = Role {
            id: RoleId(Uuid::new_v4()),
            name: "admin".to_string(),
            permissions: Permissions::MANAGE_ROLES.bits(),
            organisation_id: None,
            color: None,
            created_at: Utc::now(),
        };
        Box::pin(async move { Ok(vec![role]) })
    });
    repo.expect_insert()
        .times(1)
        .returning(|_| Box::pin(async { Ok(()) }));

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), Some(Arc::new(repo)));
    let body = json!({
        "name": "admin",
        "permissions": 7,
        "color": "#ffffff"
    });

    let response = run_request(router, |client, base_url| async move {
        client
            .post(format!(
                "{}/organisations/{}/roles",
                base_url, organisation_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .json(&body)
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::CREATED);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["data"]["name"], "admin");
    assert_eq!(value["data"]["permissions"], 7);
}

#[tokio::test]
async fn create_role_returns_forbidden_when_not_allowed() {
    let organisation_id = Uuid::new_v4();
    let identity = make_identity(vec!["viewer".to_string()]);

    let auth = auth_mock(identity);

    let mut repo = MockRoleRepository::new();
    repo.expect_list_by_names().times(1).returning(|_, _| {
        let role = Role {
            id: RoleId(Uuid::new_v4()),
            name: "viewer".to_string(),
            permissions: Permissions::VIEW_ROLES.bits(),
            organisation_id: None,
            color: None,
            created_at: Utc::now(),
        };
        Box::pin(async move { Ok(vec![role]) })
    });
    repo.expect_insert().times(0);

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), Some(Arc::new(repo)));
    let body = json!({
        "name": "admin",
        "permissions": 7
    });

    let response = run_request(router, |client, base_url| async move {
        client
            .post(format!(
                "{}/organisations/{}/roles",
                base_url, organisation_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .json(&body)
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["code"], "E_FORBIDDEN");
}

#[tokio::test]
async fn create_role_returns_unauthorized_when_missing_auth_header() {
    let organisation_id = Uuid::new_v4();

    let state = build_state();
    let router = build_router(state, None, None);
    let body = json!({
        "name": "admin",
        "permissions": 7
    });
    let response = run_request(router, |client, base_url| async move {
        client
            .post(format!(
                "{}/organisations/{}/roles",
                base_url, organisation_id
            ))
            .json(&body)
            .send()
            .await
            .expect("response")
    })
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["code"], "E_MISSING_AUTH_HEADER");
}

#[tokio::test]
async fn create_role_returns_unprocessable_entity_for_bad_payload() {
    let organisation_id = Uuid::new_v4();
    let identity = make_identity(vec!["admin".to_string()]);

    let auth = auth_mock(identity);

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), None);

    let response = run_request(router, |client, base_url| async move {
        client
            .post(format!(
                "{}/organisations/{}/roles",
                base_url, organisation_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .header("content-type", "application/json")
            .body("not-json")
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn list_roles_returns_ok_when_authorized() {
    let organisation_id = Uuid::new_v4();
    let identity = make_identity(vec!["viewer".to_string()]);

    let auth = auth_mock(identity);

    let mut repo = MockRoleRepository::new();
    repo.expect_list_by_names().times(1).returning(|_, _| {
        let role = Role {
            id: RoleId(Uuid::new_v4()),
            name: "viewer".to_string(),
            permissions: Permissions::VIEW_ROLES.bits(),
            organisation_id: None,
            color: None,
            created_at: Utc::now(),
        };
        Box::pin(async move { Ok(vec![role]) })
    });
    repo.expect_list_by_organisation()
        .times(1)
        .returning(move |_| {
            let role = Role {
                id: RoleId(Uuid::new_v4()),
                name: "admin".to_string(),
                permissions: Permissions::MANAGE_ROLES.bits(),
                organisation_id: Some(OrganisationId(organisation_id)),
                color: None,
                created_at: Utc::now(),
            };
            Box::pin(async move { Ok(vec![role]) })
        });

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), Some(Arc::new(repo)));

    let response = run_request(router, |client, base_url| async move {
        client
            .get(format!(
                "{}/organisations/{}/roles",
                base_url, organisation_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["data"][0]["name"], "admin");
}

#[tokio::test]
async fn list_roles_returns_forbidden_when_not_allowed() {
    let organisation_id = Uuid::new_v4();
    let identity = make_identity(Vec::new());

    let auth = auth_mock(identity);

    let mut repo = MockRoleRepository::new();
    repo.expect_list_by_names().times(0);
    repo.expect_list_by_organisation().times(0);

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), Some(Arc::new(repo)));

    let response = run_request(router, |client, base_url| async move {
        client
            .get(format!(
                "{}/organisations/{}/roles",
                base_url, organisation_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["code"], "E_FORBIDDEN");
}

#[tokio::test]
async fn list_roles_returns_unauthorized_when_missing_auth_header() {
    let organisation_id = Uuid::new_v4();

    let state = build_state();
    let router = build_router(state, None, None);

    let response = run_request(router, |client, base_url| async move {
        client
            .get(format!(
                "{}/organisations/{}/roles",
                base_url, organisation_id
            ))
            .send()
            .await
            .expect("response")
    })
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["code"], "E_MISSING_AUTH_HEADER");
}

#[tokio::test]
async fn get_role_returns_ok_when_authorized() {
    let organisation_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let identity = make_identity(vec!["viewer".to_string()]);

    let auth = auth_mock(identity);

    let mut repo = MockRoleRepository::new();
    repo.expect_list_by_names().times(1).returning(|_, _| {
        let role = Role {
            id: RoleId(Uuid::new_v4()),
            name: "viewer".to_string(),
            permissions: Permissions::VIEW_ROLES.bits(),
            organisation_id: None,
            color: None,
            created_at: Utc::now(),
        };
        Box::pin(async move { Ok(vec![role]) })
    });
    repo.expect_get_by_id().times(1).returning(move |_| {
        let role = Role {
            id: RoleId(role_id),
            name: "admin".to_string(),
            permissions: Permissions::MANAGE_ROLES.bits(),
            organisation_id: Some(OrganisationId(organisation_id)),
            color: None,
            created_at: Utc::now(),
        };
        Box::pin(async move { Ok(Some(role)) })
    });

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), Some(Arc::new(repo)));

    let response = run_request(router, |client, base_url| async move {
        client
            .get(format!(
                "{}/organisations/{}/roles/{}",
                base_url, organisation_id, role_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["data"]["name"], "admin");
}

#[tokio::test]
async fn get_role_returns_forbidden_when_not_allowed() {
    let organisation_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let identity = make_identity(Vec::new());

    let auth = auth_mock(identity);

    let mut repo = MockRoleRepository::new();
    repo.expect_list_by_names().times(0);
    repo.expect_get_by_id().times(0);

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), Some(Arc::new(repo)));

    let response = run_request(router, |client, base_url| async move {
        client
            .get(format!(
                "{}/organisations/{}/roles/{}",
                base_url, organisation_id, role_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["code"], "E_FORBIDDEN");
}

#[tokio::test]
async fn get_role_returns_unauthorized_when_missing_auth_header() {
    let organisation_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();

    let state = build_state();
    let router = build_router(state, None, None);

    let response = run_request(router, |client, base_url| async move {
        client
            .get(format!(
                "{}/organisations/{}/roles/{}",
                base_url, organisation_id, role_id
            ))
            .send()
            .await
            .expect("response")
    })
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["code"], "E_MISSING_AUTH_HEADER");
}

#[tokio::test]
async fn update_role_returns_ok_when_authorized() {
    let organisation_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let identity = make_identity(vec!["admin".to_string()]);

    let auth = auth_mock(identity);

    let mut repo = MockRoleRepository::new();
    repo.expect_list_by_names().times(1).returning(|_, _| {
        let role = Role {
            id: RoleId(Uuid::new_v4()),
            name: "admin".to_string(),
            permissions: Permissions::MANAGE_ROLES.bits(),
            organisation_id: None,
            color: None,
            created_at: Utc::now(),
        };
        Box::pin(async move { Ok(vec![role]) })
    });
    repo.expect_get_by_id().times(1).returning(move |_| {
        let role = Role {
            id: RoleId(role_id),
            name: "old".to_string(),
            permissions: 1,
            organisation_id: Some(OrganisationId(organisation_id)),
            color: None,
            created_at: Utc::now(),
        };
        Box::pin(async move { Ok(Some(role)) })
    });
    repo.expect_update().times(1).returning(|role| {
        assert_eq!(role.name, "new");
        assert_eq!(role.permissions, 5);
        assert_eq!(role.color.as_deref(), Some("#00ff00"));
        Box::pin(async { Ok(()) })
    });

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), Some(Arc::new(repo)));
    let body = json!({
        "name": "new",
        "permissions": 5,
        "color": "#00ff00"
    });

    let response = run_request(router, |client, base_url| async move {
        client
            .patch(format!(
                "{}/organisations/{}/roles/{}",
                base_url, organisation_id, role_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .json(&body)
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["data"]["name"], "new");
    assert_eq!(value["data"]["permissions"], 5);
}

#[tokio::test]
async fn update_role_returns_forbidden_when_not_allowed() {
    let organisation_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let identity = make_identity(Vec::new());

    let auth = auth_mock(identity);

    let mut repo = MockRoleRepository::new();
    repo.expect_list_by_names().times(0);
    repo.expect_get_by_id().times(0);
    repo.expect_update().times(0);

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), Some(Arc::new(repo)));
    let body = json!({
        "name": "new"
    });

    let response = run_request(router, |client, base_url| async move {
        client
            .patch(format!(
                "{}/organisations/{}/roles/{}",
                base_url, organisation_id, role_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .json(&body)
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["code"], "E_FORBIDDEN");
}

#[tokio::test]
async fn update_role_returns_unauthorized_when_missing_auth_header() {
    let organisation_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();

    let state = build_state();
    let router = build_router(state, None, None);
    let body = json!({
        "name": "new"
    });
    let response = run_request(router, |client, base_url| async move {
        client
            .patch(format!(
                "{}/organisations/{}/roles/{}",
                base_url, organisation_id, role_id
            ))
            .json(&body)
            .send()
            .await
            .expect("response")
    })
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["code"], "E_MISSING_AUTH_HEADER");
}

#[tokio::test]
async fn update_role_returns_unprocessable_entity_for_bad_payload() {
    let organisation_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let identity = make_identity(vec!["admin".to_string()]);

    let auth = auth_mock(identity);

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), None);

    let response = run_request(router, |client, base_url| async move {
        client
            .patch(format!(
                "{}/organisations/{}/roles/{}",
                base_url, organisation_id, role_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .header("content-type", "application/json")
            .body("not-json")
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn delete_role_returns_ok_when_authorized() {
    let organisation_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let identity = make_identity(vec!["admin".to_string()]);

    let auth = auth_mock(identity);

    let mut repo = MockRoleRepository::new();
    repo.expect_list_by_names().times(1).returning(|_, _| {
        let role = Role {
            id: RoleId(Uuid::new_v4()),
            name: "admin".to_string(),
            permissions: Permissions::MANAGE_ROLES.bits(),
            organisation_id: None,
            color: None,
            created_at: Utc::now(),
        };
        Box::pin(async move { Ok(vec![role]) })
    });
    repo.expect_delete()
        .times(1)
        .returning(|_| Box::pin(async { Ok(()) }));

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), Some(Arc::new(repo)));

    let response = run_request(router, |client, base_url| async move {
        client
            .delete(format!(
                "{}/organisations/{}/roles/{}",
                base_url, organisation_id, role_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::OK);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["success"], true);
}

#[tokio::test]
async fn delete_role_returns_forbidden_when_not_allowed() {
    let organisation_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();
    let identity = make_identity(Vec::new());

    let auth = auth_mock(identity);

    let mut repo = MockRoleRepository::new();
    repo.expect_list_by_names().times(0);
    repo.expect_delete().times(0);

    let state = build_state();
    let router = build_router(state, Some(Arc::new(auth)), Some(Arc::new(repo)));

    let response = run_request(router, |client, base_url| async move {
        client
            .delete(format!(
                "{}/organisations/{}/roles/{}",
                base_url, organisation_id, role_id
            ))
            .header(AUTHORIZATION, "Bearer good-token")
            .send()
            .await
            .expect("response")
    })
    .await;

    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["code"], "E_FORBIDDEN");
}

#[tokio::test]
async fn delete_role_returns_unauthorized_when_missing_auth_header() {
    let organisation_id = Uuid::new_v4();
    let role_id = Uuid::new_v4();

    let state = build_state();
    let router = build_router(state, None, None);
    let response = run_request(router, |client, base_url| async move {
        client
            .delete(format!(
                "{}/organisations/{}/roles/{}",
                base_url, organisation_id, role_id
            ))
            .send()
            .await
            .expect("response")
    })
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let value: serde_json::Value = response.json().await.expect("json");
    assert_eq!(value["code"], "E_MISSING_AUTH_HEADER");
}
