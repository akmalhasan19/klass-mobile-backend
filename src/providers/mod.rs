// LLM Provider module
// - Provider trait (async complete method)
// - OpenRouterProviderClient → POST {base_url}/chat/completions (OpenAI-compatible)
//     Authorization: Bearer {openrouter_api_key}
//     Optional headers: HTTP-Referer, X-Title
// - ProviderRouter (primary + fallback logic)
// - Circuit breaker via tower::limit + tower::retry
// - HTTP/2 connection pooling via reqwest
