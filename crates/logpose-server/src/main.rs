use axum::{
    extract::{State, Path},
    http::{StatusCode, Request},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use jsonwebtoken::{encode, decode, Header, Algorithm, Validation, EncodingKey, DecodingKey};
use logpose_core::{Identity, Role, Permission, Claims, RegistryStore, HealthStatus};
use logpose_db::DbRegistry;
use serde::{Deserialize, Serialize};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::{OpenApi, ToSchema};
use utoipa_swagger_ui::SwaggerUi;
use metrics_exporter_prometheus::PrometheusBuilder;

#[derive(Clone)]
struct AppState {
    registry: Arc<DbRegistry>,
    jwt_secret: String,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        get_token,
        list_services,
        register_service,
        discover_service,
        list_instances,
        update_health,
        health_check,
    ),
    components(
        schemas(
            AuthRequest, 
            AuthResponse, 
            RegisterServiceRequest, 
            HealthUpdate,
            logpose_core::auth::Role,
            logpose_core::instance::ServiceInstance,
            logpose_core::protocol::Protocol,
            logpose_core::runtime::Runtime,
            logpose_core::health::HealthStatus
        )
    ),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "api_jwt",
                utoipa::openapi::security::SecurityScheme::Http(
                    utoipa::openapi::security::HttpBuilder::new()
                        .scheme(utoipa::openapi::security::HttpAuthScheme::Bearer)
                        .bearer_format("JWT")
                        .build()
                ),
            )
        }
    }
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();
    
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Initialize metrics
    let recorder = PrometheusBuilder::new().build_recorder();
    let handle = recorder.handle();
    metrics::set_global_recorder(recorder).ok();

    let db_path = std::env::var("DATABASE_URL").unwrap_or_else(|_| "logpose.db".to_string());
    let registry = Arc::new(DbRegistry::new(&db_path).expect("Failed to open database"));
    
    let admin_cn = "admin.logpose.local";
    if registry.get_identity(admin_cn).is_err() {
        let admin = Identity {
            common_name: admin_cn.to_string(),
            organization: Some("LogPose".to_string()),
            roles: vec![Role::Admin],
        };
        registry.add_identity(&admin).expect("Failed to seed admin");
    }

    let state = AppState {
        registry: registry.clone(),
        jwt_secret: std::env::var("JWT_SECRET").unwrap_or_else(|_| "super-secret-key".to_string()),
    };

    // Spawn Health Worker
    let worker_registry = registry.clone();
    tokio::spawn(async move {
        tracing::info!("Health worker started");
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            interval.tick().await;
            if let Ok(instances) = worker_registry.get_all_instances() {
                for instance in instances {
                    let health = check_health(&instance.address).await;
                    let _ = worker_registry.update_instance_health(&instance.id, health);
                }
            }
        }
    });

    let app = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route("/health", get(health_check))
        .route("/metrics", get(move || {
            let rendered = handle.render();
            async move { rendered }
        }))
        .route("/api/auth/token", post(get_token))
        .route("/api/services", get(list_services))
        .route("/api/services", post(register_service))
        .route("/api/discover/:code", get(discover_service))
        .route("/api/services/:code/instances", get(list_instances))
        .route("/api/instances/:id/health", post(update_health))
        .layer(middleware::from_fn_with_state(state.clone(), auth_middleware))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], 3000));
    tracing::info!("listening on {}", addr);
    
    let server = axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal());

    if let Err(e) = server.await {
        tracing::error!("server error: {}", e);
    }
}

async fn check_health(addr: &SocketAddr) -> HealthStatus {
    match tokio::time::timeout(Duration::from_secs(2), tokio::net::TcpStream::connect(addr)).await {
        Ok(Ok(_)) => HealthStatus::Healthy,
        _ => HealthStatus::Unhealthy,
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("signal received, starting graceful shutdown");
}

#[derive(Serialize, Deserialize, ToSchema)]
struct AuthRequest {
    #[schema(example = "admin.logpose.local")]
    common_name: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
struct AuthResponse {
    token: String,
}

#[utoipa::path(
    post,
    path = "/api/auth/token",
    request_body = AuthRequest,
    responses(
        (status = 200, description = "Token generated successfully", body = AuthResponse),
        (status = 401, description = "Identity not found")
    )
)]
async fn get_token(
    State(state): State<AppState>,
    Json(payload): Json<AuthRequest>,
) -> impl IntoResponse {
    match state.registry.get_identity(&payload.common_name) {
        Ok(identity) => {
            let claims = Claims {
                sub: identity.common_name,
                roles: identity.roles,
                exp: 10000000000,
            };
            let token = encode(
                &Header::default(),
                &claims,
                &EncodingKey::from_secret(state.jwt_secret.as_ref()),
            ).unwrap();
            
            (StatusCode::OK, Json(AuthResponse { token })).into_response()
        }
        Err(_) => (StatusCode::UNAUTHORIZED, "Identity not found").into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/services",
    responses(
        (status = 200, description = "List of services retrieved successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions")
    ),
    security(("api_jwt" = []))
)]
async fn list_services(
    axum::extract::Extension(claims): axum::extract::Extension<Claims>,
) -> impl IntoResponse {
    let has_permission = claims.roles.iter().any(|role| {
        role.permissions().contains(&Permission::ServiceRead)
    });

    if !has_permission {
        return (StatusCode::FORBIDDEN, "Insufficient permissions").into_response();
    }

    (StatusCode::OK, "List of services").into_response()
}

#[derive(Serialize, Deserialize, ToSchema)]
struct RegisterServiceRequest {
    #[schema(example = "Auth Service")]
    name: String,
    #[schema(example = "auth-svc")]
    code: String,
    #[schema(example = "Handles user authentication and authorization")]
    description: String,
}

#[utoipa::path(
    post,
    path = "/api/services",
    request_body = RegisterServiceRequest,
    responses(
        (status = 201, description = "Service registered successfully"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Insufficient permissions")
    ),
    security(("api_jwt" = []))
)]
async fn register_service(
    State(state): State<AppState>,
    axum::extract::Extension(claims): axum::extract::Extension<Claims>,
    Json(payload): Json<RegisterServiceRequest>,
) -> impl IntoResponse {
    let has_permission = claims.roles.iter().any(|role| {
        role.permissions().contains(&Permission::ServiceWrite)
    });

    if !has_permission {
        return (StatusCode::FORBIDDEN, "Insufficient permissions").into_response();
    }

    let service = logpose_core::Service::new(payload.name, payload.code, payload.description);
    match state.registry.add_service(&service) {
        Ok(_) => (StatusCode::CREATED, "Service registered").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed").into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/discover/{code}",
    responses((status = 200, description = "Discovery", body = Vec<ServiceInstance>)),
    params(("code" = String, Path, description = "Service code"))
)]
async fn discover_service(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> impl IntoResponse {
    match state.registry.get_instances(&code) {
        Ok(instances) => (StatusCode::OK, Json(instances)).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Service not found").into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/api/services/{code}/instances",
    responses((status = 200, description = "Instance list", body = Vec<ServiceInstance>)),
    params(("code" = String, Path, description = "Service code"))
)]
async fn list_instances(
    State(state): State<AppState>,
    Path(code): Path<String>,
) -> impl IntoResponse {
    match state.registry.get_instances(&code) {
        Ok(instances) => (StatusCode::OK, Json(instances)).into_response(),
        Err(_) => (StatusCode::NOT_FOUND, "Service not found").into_response(),
    }
}

#[derive(Serialize, Deserialize, ToSchema)]
struct HealthUpdate {
    status: HealthStatus,
}

#[utoipa::path(
    post,
    path = "/api/instances/{id}/health",
    request_body = HealthUpdate,
    responses((status = 200, description = "Updated")),
    params(("id" = String, Path, description = "Instance ID"))
)]
async fn update_health(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<HealthUpdate>,
) -> impl IntoResponse {
    let id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => return (StatusCode::BAD_REQUEST, "Invalid ID").into_response(),
    };
    match state.registry.update_instance_health(&id, payload.status) {
        Ok(_) => (StatusCode::OK, "Updated").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Failed").into_response(),
    }
}

#[utoipa::path(
    get,
    path = "/health",
    responses((status = 200, description = "OK"))
)]
async fn health_check() -> impl IntoResponse {
    (StatusCode::OK, "OK").into_response()
}

async fn auth_middleware<B>(
    State(state): State<AppState>,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, StatusCode> {
    let path = req.uri().path();
    if path == "/api/auth/token" || path == "/health" || path == "/metrics" || path.starts_with("/swagger-ui") || path.starts_with("/api-docs") {
        return Ok(next.run(req).await);
    }

    let auth_header = req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    match auth_header {
        Some(token) => {
            let token_data = decode::<Claims>(
                token,
                &DecodingKey::from_secret(state.jwt_secret.as_ref()),
                &Validation::new(Algorithm::HS256),
            ).map_err(|_| StatusCode::UNAUTHORIZED)?;

            req.extensions_mut().insert(token_data.claims);
            Ok(next.run(req).await)
        }
        None => Err(StatusCode::UNAUTHORIZED),
    }
}
