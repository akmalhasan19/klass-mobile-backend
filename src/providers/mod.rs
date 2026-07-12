// LLM Provider module
// - Provider trait (async complete method)
// - GeminiProviderClient → POST generateContent API
// - OpenAIProviderClient → POST /v1/responses
// - ProviderRouter (primary + fallback logic)
// - Circuit breaker via tower::limit + tower::retry
// - HTTP/2 connection pooling via reqwest
