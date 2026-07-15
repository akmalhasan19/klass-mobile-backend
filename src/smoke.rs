use std::time::Duration;

use crate::config::AppConfig;
use crate::providers::{
    ChatMessage, CompletionRequest, OpenRouterConfig, OpenRouterProviderClient, Provider,
};

pub async fn smoke_llm(config: &AppConfig) -> anyhow::Result<()> {
    if config.openrouter_api_key.is_empty() {
        anyhow::bail!("OPENROUTER_API_KEY is not set");
    }

    let http = reqwest::Client::builder()
        .use_rustls_tls()
        .http2_prior_knowledge()
        .gzip(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(90))
        .build()?;

    let or_config = OpenRouterConfig::from_app_config(config);
    let provider = OpenRouterProviderClient::new(http, or_config);

    let request = CompletionRequest::new(
        &config.openrouter_model,
        vec![
            ChatMessage {
                role: "system".to_string(),
                content: "You are a smoke-test helper. Respond with exactly 'OK' and nothing else."
                    .to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "Respond with OK.".to_string(),
            },
        ],
    );

    let response = provider.complete(request).await?;
    let content = response
        .first_choice_content()
        .ok_or_else(|| anyhow::anyhow!("OpenRouter returned no content in any choice"))?;

    tracing::info!(
        content = %content,
        model = ?response.model,
        finish_reason = ?response.first_finish_reason(),
        "OpenRouter LLM smoke test passed"
    );
    Ok(())
}

pub async fn smoke_python(config: &AppConfig) -> anyhow::Result<()> {
    if config.media_gen_url.is_empty() {
        anyhow::bail!("MEDIA_GEN_URL is not set");
    }

    let http = reqwest::Client::builder()
        .use_rustls_tls()
        .http2_prior_knowledge()
        .gzip(true)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .build()?;

    let base_url = config.media_gen_url.trim_end_matches('/');
    let health_url = format!("{}/v1/health", base_url);

    tracing::info!(url = %health_url, "Checking Python renderer health");

    let resp = http.get(&health_url).send().await.map_err(|e| {
        anyhow::anyhow!("Python renderer health check connection failed: {}", e)
    })?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Python renderer health check failed: HTTP {}",
            resp.status()
        );
    }

    let body: serde_json::Value = resp.json().await.map_err(|e| {
        anyhow::anyhow!("Failed to parse Python renderer health response: {}", e)
    })?;

    let formats = body
        .get("supported_formats")
        .and_then(|v| v.as_array())
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Missing or invalid 'supported_formats' in health response: {}",
                body
            )
        })?;

    for required in &["docx", "pdf", "pptx"] {
        if !formats.iter().any(|f| f.as_str() == Some(required)) {
            anyhow::bail!(
                "Missing required format '{}' in supported_formats ({:?})",
                required,
                formats
            );
        }
    }

    let _contracts = body.get("contract_versions").ok_or_else(|| {
        anyhow::anyhow!("Missing 'contract_versions' in Python renderer health response")
    })?;

    tracing::info!(
        supported_formats = ?formats,
        "Python renderer smoke test passed"
    );
    Ok(())
}
