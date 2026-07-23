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
use klass_gateway::cache::LlmCacheRepo;
use klass_gateway::config::AppConfig;
use klass_gateway::governance::ledger::LedgerRepo;
use klass_gateway::governance::price_catalog::PriceCatalogRepo;
use klass_gateway::governance::rate_limit::{RateLimitPoliciesRepo, RateLimitBucketsRepo};
use klass_gateway::llm::draft::DraftService;
use klass_gateway::llm::interpret::InterpretService;
use klass_gateway::llm::respond::RespondService;
use klass_gateway::llm::step_adapters::{InterpretStepAdapter, DraftStepAdapter, ComposeStepAdapter};
use klass_gateway::media_gen::publication::MediaPublicationService;
use klass_gateway::media_gen::python_client::PythonMediaGeneratorClient;
use klass_gateway::orchestrator::workflow::{
    ComposeStep, DraftStep, InterpretStep,
};
use klass_gateway::providers::openrouter::{OpenRouterConfig, OpenRouterProviderClient};
use klass_gateway::providers::router::ProviderRouter;
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
        run_server_with_worker(config).await
    }
}

// ─── Server mode (with embedded worker) ───────────────────────────────────

/// Default mode: runs both the REST API server and the media generation
/// worker in a single process. The worker is spawned as a background task
/// so no separate Background Worker service is needed on Render (or similar
/// platforms).
///
/// Use `--worker` to run a standalone worker process (backward compat).
async fn run_server_with_worker(config: AppConfig) -> anyhow::Result<()> {
    info!(
        host = %config.host,
        port = %config.port,
        grpc_port = %config.grpc_port,
        "Starting Klass Gateway (server + embedded worker)"
    );

    let state = AppState::new(config.clone()).await?;

    // ── Spawn embedded worker in background ────────────────────────────────
    if state.redis_pool.is_some() {
        let worker_config = config.clone();
        let worker_state = state.clone();
        tokio::spawn(async move {
            loop {
                if let Err(e) = run_embedded_worker(worker_config.clone(), worker_state.clone()).await {
                    tracing::error!(error = %e, "Embedded worker exited with error, restarting in 5 seconds...");
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                } else {
                    tracing::info!("Embedded worker exited gracefully");
                    break;
                }
            }
        });
        info!("Embedded worker spawned as background task");
    } else {
        tracing::warn!("REDIS_URL not configured — embedded worker disabled");
    }

    // ── Run REST server (foreground) ────────────────────────────────────────
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
        .route("/api/v1/system-health", get(api::rest::system_health::system_health))
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

/// Run the embedded worker using an existing AppState (shared pool/redis).
///
/// This is a slimmer version of `run_worker` that reuses the AppState
/// created by the server, avoiding a second DB/Redis connection pool.
async fn run_embedded_worker(config: AppConfig, state: AppState) -> anyhow::Result<()> {
    info!("Embedded worker starting event loop");

    let redis_pool = state.redis_pool.ok_or_else(|| {
        anyhow::anyhow!("REDIS_URL is required for embedded worker")
    })?;

    let concurrency = config.media_generation.queue.concurrency as usize;
    let consumer_name = format!("{}-{}", CONSUMER_PREFIX, hostname());

    let worker = Worker::new(
        redis_pool.clone(),
        state.db_pool.clone(),
        consumer_name,
        concurrency,
    );

    // ── Build shared LLM infrastructure ──────────────────────────────────
    let pool = state.db_pool.clone();

    let or_config = OpenRouterConfig::from_app_config(&config);
    let openrouter_client = OpenRouterProviderClient::new(state.http.clone(), or_config.clone());
    let provider_router = Arc::new(ProviderRouter::new(Box::new(openrouter_client)));

    // ── Build LLM services ──────────────────────────────────────────────
    let interpret_service = InterpretService::new(
        LlmCacheRepo::new(pool.clone()),
        LedgerRepo::new(pool.clone()),
        PriceCatalogRepo::new(pool.clone()),
        RateLimitPoliciesRepo::new(pool.clone()),
        RateLimitBucketsRepo::new(pool.clone()),
        provider_router.clone(),
        state.taxonomy.clone(),
        or_config.model.clone(),
        klass_gateway::llm::interpret::DEFAULT_INTERPRET_INSTRUCTION.to_string(),
    );

    let draft_service = DraftService::new(
        LlmCacheRepo::new(pool.clone()),
        LedgerRepo::new(pool.clone()),
        PriceCatalogRepo::new(pool.clone()),
        RateLimitPoliciesRepo::new(pool.clone()),
        RateLimitBucketsRepo::new(pool.clone()),
        provider_router.clone(),
        or_config.model.clone(),
        klass_gateway::llm::draft::DEFAULT_DRAFT_INSTRUCTION.to_string(),
    );

    let respond_service = RespondService::new(
        LlmCacheRepo::new(pool.clone()),
        LedgerRepo::new(pool.clone()),
        PriceCatalogRepo::new(pool.clone()),
        RateLimitPoliciesRepo::new(pool.clone()),
        RateLimitBucketsRepo::new(pool.clone()),
        provider_router.clone(),
        or_config.model.clone(),
        klass_gateway::llm::respond::DEFAULT_RESPOND_INSTRUCTION.to_string(),
    );

    // ── Build step adapters ─────────────────────────────────────────────
    let interpret: Arc<dyn InterpretStep> = Arc::new(InterpretStepAdapter::new(pool.clone(), interpret_service));
    let draft: Arc<dyn DraftStep> = Arc::new(DraftStepAdapter::new(pool.clone(), draft_service));
    let compose: Arc<dyn ComposeStep> = Arc::new(ComposeStepAdapter::new(pool.clone(), respond_service));

    let generate = Arc::new(PythonMediaGeneratorClient::new(
        pool.clone(),
        state.http.clone(),
        &config,
    ));

    let publish = Arc::new(MediaPublicationService::new(
        pool.clone(),
        state.s3_client.clone(),
        state.http.clone(),
        config.r2_bucket_name.clone(),
        config.r2_transit_bucket_name.clone(),
        config.r2_public_url.clone(),
    ));

    let workflow = klass_gateway::orchestrator::workflow::WorkflowService::new(
        pool.clone(),
    );

    info!(
        concurrency = concurrency,
        model = %or_config.model,
        "Embedded worker starting event loop with LLM pipeline wired"
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
        .map_err(|e| anyhow::anyhow!("Embedded worker exited with error: {e}"))
}

// ─── Standalone Worker mode (--worker flag) ──────────────────────────────────

/// Standalone worker mode: runs only the media generation worker without
/// the REST server. Use this when deploying a separate Background Worker
/// service on Render (or similar platforms).
///
/// In most cases you should prefer the default `run_server_with_worker`
/// mode which embeds the worker as a background task.
async fn run_worker(config: AppConfig) -> anyhow::Result<()> {
    info!("Starting Klass Gateway (standalone worker mode)");

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

    // ── Build shared LLM infrastructure ──────────────────────────────────
    let pool = state.db_pool.clone();

    let or_config = OpenRouterConfig::from_app_config(&config);
    let openrouter_client = OpenRouterProviderClient::new(state.http.clone(), or_config.clone());
    let provider_router = Arc::new(ProviderRouter::new(Box::new(openrouter_client)));

    // ── Build LLM services ──────────────────────────────────────────────
    // Each service needs its own repo instances (repos are not Clone).
    let interpret_service = InterpretService::new(
        LlmCacheRepo::new(pool.clone()),
        LedgerRepo::new(pool.clone()),
        PriceCatalogRepo::new(pool.clone()),
        RateLimitPoliciesRepo::new(pool.clone()),
        RateLimitBucketsRepo::new(pool.clone()),
        provider_router.clone(),
        state.taxonomy.clone(),
        or_config.model.clone(),
        klass_gateway::llm::interpret::DEFAULT_INTERPRET_INSTRUCTION.to_string(),
    );

    let draft_service = DraftService::new(
        LlmCacheRepo::new(pool.clone()),
        LedgerRepo::new(pool.clone()),
        PriceCatalogRepo::new(pool.clone()),
        RateLimitPoliciesRepo::new(pool.clone()),
        RateLimitBucketsRepo::new(pool.clone()),
        provider_router.clone(),
        or_config.model.clone(),
        klass_gateway::llm::draft::DEFAULT_DRAFT_INSTRUCTION.to_string(),
    );

    let respond_service = RespondService::new(
        LlmCacheRepo::new(pool.clone()),
        LedgerRepo::new(pool.clone()),
        PriceCatalogRepo::new(pool.clone()),
        RateLimitPoliciesRepo::new(pool.clone()),
        RateLimitBucketsRepo::new(pool.clone()),
        provider_router.clone(),
        or_config.model.clone(),
        klass_gateway::llm::respond::DEFAULT_RESPOND_INSTRUCTION.to_string(),
    );

    // ── Build step adapters ─────────────────────────────────────────────
    let interpret: Arc<dyn InterpretStep> = Arc::new(InterpretStepAdapter::new(pool.clone(), interpret_service));
    let draft: Arc<dyn DraftStep> = Arc::new(DraftStepAdapter::new(pool.clone(), draft_service));
    let compose: Arc<dyn ComposeStep> = Arc::new(ComposeStepAdapter::new(pool.clone(), respond_service));

    let generate = Arc::new(PythonMediaGeneratorClient::new(
        pool.clone(),
        state.http.clone(),
        &config,
    ));

    let publish = Arc::new(MediaPublicationService::new(
        pool.clone(),
        state.s3_client.clone(),
        state.http.clone(),
        config.r2_bucket_name.clone(),
        config.r2_transit_bucket_name.clone(),
        config.r2_public_url.clone(),
    ));

    let workflow = klass_gateway::orchestrator::workflow::WorkflowService::new(
        pool.clone(),
    );

    info!(
        concurrency = concurrency,
        model = %or_config.model,
        "Worker starting event loop with LLM pipeline wired"
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
