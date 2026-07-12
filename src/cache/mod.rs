// Semantic LLM response cache
// - Cache key generation (SHA-256 of canonical JSON)
// - Cache lookup / store (interpret + respond routes)
// - Advisory lock stampede protection (pg_try_advisory_lock)
// - Lazy cleanup expired entries
// - Byte-compatible with Python cache_key hash
