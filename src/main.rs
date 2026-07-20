use std::sync::Arc;
use std::net::SocketAddr;
use std::time::Duration;

use axum::routing::{get, post};
use axum::Router;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;
use tower_http::compression::CompressionLayer;
use tower_http::cors::{AllowOrigin, Any, CorsLayer};
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

use klass_gateway::api;
use klass_gateway::config::AppConfig;
use klass_gateway::media_gen::publication::MediaPublicationService;
use klass_gateway::media_gen::python_client::PythonMediaGeneratorClient;
use klass_gateway::orchestrator::workflow::{
    ComposeStep, DraftStep, InterpretStep, WorkflowError,
};
use klass_gateway::queue::worker::{Worker, CONSUMER_PREFIX};
use klass_gateway::state::AppState;

// ─── Entrypoint ─────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,klass_gateway=debug".into()),
        )
        .json()
        .init();

    let args: Vec<String> = std::env::args().collect();
    let is_worker = args.iter().any(|a| a == "--worker");
    let smoke_llm = args.iter().any(|a| a == "--smoke-llm");
    let smoke_python = args.iter().any(|a| a == "--smoke-python");

    let config = AppConfig::from_env()?;

    if smoke_llm {
        return klass_gateway::smoke::smoke_llm(&config).await;
    }
    if smoke_python {
        return klass_gateway::smoke::smoke_python(&config).await;
    }

    if is_worker {
        run_worker(config).await
    } else {
        run_server(config).await
    }
}

// ─── Server mode ────────────────────────────────────────────────────────────

async fn run_server(config: AppConfig) -> anyhow::Result<()> {
    info!(
        host = %config.host,
        port = %config.port,
        grpc_port = %config.grpc_port,
        "Starting Klass Gateway (server mode)"
    );

    let state = AppState::new(config.clone()).await?;

    let cors = build_cors_layer(&config.cors_allowed_origins);

    let swagger: Router<AppState> = SwaggerUi::new("/api-docs/swagger-ui")
        .url("/api-docs/openapi.json", api::openapi::ApiDoc::openapi())
        .into();

    // Internal routes (not exposed in public API docs)
    let internal_routes = Router::new()
        .route(
            "/media-generations/webhook",
            post(api::rest::media_webhook::webhook_handler),
        );

    let app = Router::new()
        .nest("/api/v1", api::rest::api_router())
        .route("/health", get(health_check))
        .nest("/internal", internal_routes)
        .merge(swagger)
        .with_state(state)
        .layer(
            tower::ServiceBuilder::new()
                .layer(SetRequestIdLayer::new(
                    "x-request-id".parse().unwrap(),
                    MakeRequestUuid,
                ))
                .layer(TraceLayer::new_for_http())
                .layer(PropagateRequestIdLayer::new(
                    "x-request-id".parse().unwrap(),
                ))
                .layer(CompressionLayer::new())
                .layer(cors)
                .layer(tower_http::timeout::TimeoutLayer::with_status_code(
                    axum::http::StatusCode::REQUEST_TIMEOUT,
                    Duration::from_secs(120),
                ))
                .into_inner(),
        );

    let addr = SocketAddr::new(config.host.parse()?, config.port);
    info!("REST server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ─── Worker mode ────────────────────────────────────────────────────────────

async fn run_worker(config: AppConfig) -> anyhow::Result<()> {
    info!("Starting Klass Gateway (worker mode)");

    let state = AppState::new(config.clone()).await?;

    let redis_pool = state.redis_pool.ok_or_else(|| {
        anyhow::anyhow!("REDIS_URL is required in worker mode")
    })?;

    let concurrency = config.media_generation.queue.concurrency as usize;
    let consumer_name = format!("{}-{}", CONSUMER_PREFIX, hostname());

    let worker = Worker::new(
        redis_pool.clone(),
        state.db_pool.clone(),
        consumer_name,
        concurrency,
    );

    // Build step implementations
    let interpret: Arc<dyn InterpretStep> = Arc::new(UnimplementedStep::new("interpret"));
    let draft: Arc<dyn DraftStep> = Arc::new(UnimplementedStep::new("draft"));
    let compose: Arc<dyn ComposeStep> = Arc::new(UnimplementedStep::new("compose"));

    let generate = Arc::new(PythonMediaGeneratorClient::new(
        state.db_pool.clone(),
        state.http.clone(),
        &config,
    ));

    let publish = Arc::new(MediaPublicationService::new(
        state.db_pool.clone(),
        state.s3_client.clone(),
        state.http.clone(),
        config.r2_bucket_name.clone(),
        config.r2_transit_bucket_name.clone(),
        config.r2_public_url.clone(),
    ));

    let workflow = klass_gateway::orchestrator::workflow::WorkflowService::new(
        state.db_pool.clone(),
    );

    info!(
        concurrency = concurrency,
        "Worker starting event loop"
    );

    worker
        .run(
            &workflow,
            interpret,
            draft,
            generate,
            publish,
            compose,
        )
        .await
        .map_err(|e| anyhow::anyhow!("Worker exited with error: {e}"))
}

// ─── Placeholder step for services not yet implemented ─────────────────────

/// Returns an error indicating the step is not yet wired.
struct UnimplementedStep {
    _name: &'static str,
}

impl UnimplementedStep {
    const fn new(_name: &'static str) -> Self {
        Self { _name }
    }
}

#[async_trait::async_trait]
impl InterpretStep for UnimplementedStep {
    async fn interpret(&self, _generation_id: &str) -> Result<serde_json::Value, WorkflowError> {
        Err(WorkflowError::StepProvider(format!(
            "interpret step not yet wired: the InterpretService must be connected in Phase 5 completion"
        )))
    }
}

#[async_trait::async_trait]
impl DraftStep for UnimplementedStep {
    async fn draft(&self, _generation_id: &str) -> Result<serde_json::Value, WorkflowError> {
        Err(WorkflowError::StepProvider(format!(
            "draft step not yet wired: the DraftService must be connected in Phase 5 completion"
        )))
    }
}

#[async_trait::async_trait]
impl ComposeStep for UnimplementedStep {
    async fn compose(&self, _generation_id: &str) -> Result<serde_json::Value, WorkflowError> {
        Err(WorkflowError::StepProvider(format!(
            "compose step not yet wired: the RespondService must be connected in Phase 5 completion"
        )))
    }
}

// ─── CORS helper ───────────────────────────────────────────────────────────

fn build_cors_layer(origins: &str) -> CorsLayer {
    if origins.is_empty() || origins == "*" {
        return CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);
    }

    let allowed_origins: Vec<_> = origins
        .split(',')
        .filter_map(|o| {
            let o = o.trim();
            if o.is_empty() {
                None
            } else {
                o.parse().ok()
            }
        })
        .collect();

    if allowed_origins.is_empty() {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(allowed_origins))
            .allow_methods(Any)
            .allow_headers(Any)
    }
}

// ─── Health check ───────────────────────────────────────────────────────────

async fn health_check() -> axum::Json<serde_json::Value> {
    axum::Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION")
    }))
}

// ─── Helpers ───────────────────────────────────────────────────────────────

fn hostname() -> String {
    std::env::var("HOSTNAME")
        .or_else(|_| std::env::var("HOST"))
        .unwrap_or_else(|_| format!("worker-{}", std::process::id()))
}
