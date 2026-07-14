# Implementation Plan: Hybrid Rust+Laravel → Pure Rust Migration

> **Status tracking file.** Tick `- [x]` as work completes. Each phase is independently deployable.
> Companion to `MIGRATION_PROMPT.md`. All phases target a single Rust binary on Render.

## Guiding Principles

- Single Rust binary, single Render container, `/health` healthcheck.
- Inline LLM adapter → Rust calls OpenRouter `/chat/completions` directly (cache + governance + audit ledger in Rust).
- Python renderer HF Space remains external (HMAC-signed `/v1/generate`) — untouched.
- Blade admin panel removed; admin functions exposed via REST `/api/v1/admin/*` + activity logs.
- Background worker via Redis Streams (XADD/XREADGROUP/XACK); Flutter keeps REST polling.
- Auth uses `personal_access_tokens` table; opaque token `"{id}|{hash}"` stored as SHA-256.
- DB schema already complete (20 sqlx migrations incl. 5 governance tables) — no new migrations except as noted.
- Recommendation engine + taxonomy JSON ported in full.

---

## Phase 0 — Foundation & Build Fixes (1–2 days)

**Goal:** Repo compiles with real queries, middleware wired, Blueprint deployment.

- [x] **0.1 Fix `build.rs`**
  - [x] Replace `println!("Hello, world!");` with `tonic_build::configure().build_server(true).compile_protos(&["proto/klass/media/v1/media_generation.proto"], &["proto"])?`
  - [x] Verify `protoc` available in Dockerfile (already installed)
  - [x] Confirm generated `.rs` in `target/` after build
- [x] **0.2 Wire middleware stack in `src/main.rs`**
  - [x] `TraceLayer` (from `tower-http`)
  - [x] `RequestId` + `SetResponseRequestIdLayer`
  - [x] `CompressionLayer` (gzip)
  - [x] `CorsLayer` (origins from `CORS_ALLOWED_ORIGINS`)
  - [x] `TimeoutLayer` (default 120s, override 90s per media-gen routes)
  - [x] `tracing_subscriber` fmt JSON + `EnvFilter` (verify current setup)
- [x] **0.3 Expand `AppState` in `src/state.rs`**
  - [x] `pg_pool: PgPool` via `PgPoolOptions::max_connections(config.database_max_connections)` (currently unused)
  - [x] `redis_pool: Option<deadpool_redis::Pool>` (keep current)
  - [x] `s3_client: aws_sdk_s3::Client` (R2 endpoint via `aws-config::SdkConfig`)
  - [x] `http: reqwest::Client` (rustls, HTTP/2, connect_timeout 10s, timeout 90s, gzip)
  - [x] `taxonomy: Arc<TaxonomyCatalog>` (load `resources/json/kurikulum_merdeka_structure.json` at boot)
- [x] **0.4 Extend `src/config.rs`**
  - [x] Add per-service timeout/retry fields from `config/services.php` (interpreter/drafting/delivery/python)
  - [x] Fix env name mapping (flat `R2_ACCESS_KEY_ID`, not `R2__ACCESS_KEY_ID`)
  - [x] Add `LLM_ADAPTER_FALLBACK_URL` env (rollback safety)
  - [x] Add `recommendations` config struct (hardcoded defaults, no env needed)
- [x] **0.5 Initialize `.sqlx/` offline cache**
  - [x] Add first real `sqlx::query!` in `db` module
  - [x] Run `cargo sqlx prepare` against Neon dev DB
  - [x] Verify `SQLX_OFFLINE=true` build succeeds
  - [x] Commit `.sqlx/*.json` files
- [x] **0.6 Create `render.yaml` Blueprint**
  - [x] Service `klass-gateway` (web) — Docker, env vars, `/health` healthcheck, 512 MB
  - [x] Service `klass-gateway-worker` — same image, startCommand `-worker`, 512 MB
  - [x] Env group for shared secrets (DATABASE_URL, REDIS_URL, R2_*, OPENROUTER_*, HMAC_SECRET, MEDIA_GEN_*, MEDIA_GEN_HMAC_SECRET)
- [x] **0.7 Add Rust CI workflow**
  - [x] `.github/workflows/rust.yml`: `cargo fmt --check`
  - [x] `cargo clippy -- -D warnings`
  - [x] `cargo sqlx prepare --check`
  - [x] `cargo test`
  - [x] `cargo build --release`
- [x] **0.8 Add `[features]` to `Cargo.toml`**
  - [x] `default = ["rest"]`
  - [x] `grpc` (feature-gates tonic server)
  - [x] `worker` (feature-gates worker binary branch)
- [x] **0.9 Deploy Phase 0 to Render staging**
  - [x] `/health` returns 200
  - [x] CI Rust green on PR

---

## Phase 1 — Auth & User (3–4 days)

**Goal:** login/register/logout/me/refresh/avatar + security question + password reset. Protected endpoints testable.

- [x] **1.1 `src/auth/mod.rs` scaffolding**
  - [x] Declare submodules: `password`, `tokens`, `middleware`, `signing`
- [x] **1.2 `src/auth/password.rs`**
  - [x] `hash_password(plain) -> Result<String>` via `argon2::Argon2` default params
  - [x] `verify_password(plain, hash) -> bool` — handle both `argon2id` (`$argon2id$`) and bcrypt (`$2y$`/`$2b$`) prefixes for DB compatibility
  - [x] Unit tests: hash + verify roundtrip; bcrypt prefix detection
- [x] **1.3 `src/auth/tokens.rs`**
  - [x] `PersonalAccessToken` struct (`#[derive(FromRow)]`)
  - [x] `issue_token(user_id, name, abilities) -> String` returns `"{id}|{plain_random_64_hex}"`, stores `sha256(plain)` hex (64 chars)
  - [x] `verify_token(plain) -> Option<Token>`: split at `|`, parse `id`, hash remainder, SELECT
  - [x] `revoke_token(id)`, `revoke_all_for_user(user_id)`
- [x] **1.4 `src/db/repositories/users.rs`**
  - [x] `UsersRepo` trait + `PgUsersRepo { pool }` impl
  - [x] `find_by_email`, `find_by_id`, `insert`, `update_avatar`, `update_password`, `set_role`, `set_security_qa`
  - [x] `User` struct `#[derive(FromRow)]` matching `users` table
- [x] **1.5 `src/db/repositories/personal_access_tokens.rs`**
  - [x] `find_by_id`, `find_by_token_hash`, `insert`, `delete_by_id`, `update_last_used_at`
- [x] **1.6 `src/auth/middleware.rs`**
  - [x] `AuthUser` extractor — reads `Authorization: Bearer`, populates `Extension<Principal>`
  - [x] `require_role(role)` tower middleware / handler predicate
  - [x] Rate limit via Redis (throttle 3,1 register; 5,1 login)
- [x] **1.7 `src/auth/signing.rs`**
  - [x] `InterServiceRequestSigner::build(secret, generation_id, payload) -> SignedRequest`
  - [x] HMAC-SHA256 over `timestamp.encoded_payload`
  - [x] Headers: `X-Request-Id`, `X-Klass-Generation-Id`, `X-Klass-Request-Timestamp`, `X-Klass-Signature-Algorithm`, `X-Klass-Signature`
  - [x] Unit tests: signature determinism, header shape
- [x] **1.8 `src/api/rest/auth.rs` — 7 handlers**
  - [x] `POST /auth/register` — 201, returns `UserResource` + token
  - [x] `POST /auth/login` — validate via `Auth::attempt`-equivalent, log `failed_login_attempt` on fail, return token + `UserResource`
  - [x] `POST /auth/logout` — revoke current token
  - [x] `GET /auth/me` — return `UserResource` of authenticated user
  - [x] `POST /auth/refresh` — revoke current, issue new
  - [x] `POST /auth/get-security-question` — return stored `security_question` for email
  - [x] `POST /auth/verify-and-reset-password` — verify `security_answer` hash, update password
- [x] **1.9 `src/api/rest/avatar.rs`**
  - [x] `POST /user/avatar` — any authenticated user, upload via R2, update `users.avatar_url`
  - [x] Depends on Phase 0 R2 client (can stub storage until Phase 3)
- [x] **1.10 `UserResource` serde struct**
  - [x] `id, name, email, avatar_url, role, primary_subject_id` (omit `password`, `security_answer`)
  - [x] Response envelope `{success, message, data:{user, token?}}`
- [x] **1.11 Tests `tests/api_auth.rs`**
  - [x] register → login → me → logout → refresh end-to-end
  - [x] Reset password flow with security question
  - [x] Throttle enforcement (use Redis test instance)
  - [x] Wrong password logs `failed_login_attempt` activity

---

## Phase 2 — Public Read API (4–5 days)

**Goal:** All GET endpoints powering Flutter home/learning feeds.

- [x] **2.1 Pagination helper**
  - [x] `Pagination { page, per_page, total }` struct
  - [x] Default `per_page=15`, max 50
  - [x] Meta envelope `{pagination:{total, per_page, current_page, last_page}}`
- [x] **2.2 `src/db/repositories/topics.rs`**
  - [x] `Topic` struct `#[derive(FromRow)]` (UUID id, FKs `sub_subject_id`, `owner_user_id`)
  - [x] `find_many(filters, pagination)`, `find_by_id(id)` (eager `contents.tasks`)
  - [x] Filters: `ilike` search on title, `subject_id`, `sub_subject_id`, `is_published`
- [x] **2.3 `src/db/repositories/contents.rs`**
  - [x] `Content` struct with `data: serde_json::Value` (`serde_json` preserve_order enabled ✓)
  - [x] `find_many(filters, pagination)`, `find_by_id(id)` (eager `topic`, `tasks`)
  - [x] Filters: `ilike` search, `topic_id`, `type`
- [x] **2.4 `src/db/repositories/marketplace_tasks.rs`**
  - [x] `MarketplaceTask` struct
  - [x] `find_many(filters, pagination)`, `find_by_id(id)`
  - [x] Filters: search via `content.title` join, `status`, `content_id`
- [x] **2.5 `src/db/repositories/student_progress.rs`**
  - [x] `StudentProgress` struct (`completion_date: chrono::DateTime<Utc>`)
  - [x] `find_many(filters, pagination)`, `find_by_id(id)`
  - [x] Filters: `ilike` on `student_name`, sorted by `completion_date DESC`
- [x] **2.6 `src/db/repositories/gallery.rs`**
  - [x] Reuse `contents` query — `find_many` where `media_url IS NOT NULL`
  - [x] Filters: `ilike` search, `type`, `topic_id`
- [x] **2.7 `src/db/repositories/homepage_sections.rs`**
  - [x] `HomepageSection` struct
  - [x] `find_enabled_ordered()` — WHERE `is_enabled=true` ORDER BY `position`
- [x] **2.8 `src/api/rest/topics.rs`**
  - [x] `GET /topics`, `GET /topics/{id}`
- [x] **2.9 `src/api/rest/contents.rs`**
  - [x] `GET /contents`, `GET /contents/{id}`
- [x] **2.10 `src/api/rest/marketplace_tasks.rs`**
  - [x] `GET /marketplace-tasks`, `GET /marketplace-tasks/{id}`
- [x] **2.11 `src/api/rest/student_progress.rs`**
  - [x] `GET /student-progress`, `GET /student-progress/{id}`
- [x] **2.12 `src/api/rest/homepage_sections.rs`**
  - [x] `GET /homepage-sections`
- [x] **2.13 `src/api/rest/gallery.rs`**
  - [x] `GET /gallery`
- [x] **2.14 Stub `GET /homepage-recommendations`** (deferred to Phase 4)
  - [x] Return admin-curated `RecommendedProject` rows only (no personalization yet)
- [x] **2.15 `src/api/rest/mod.rs` router assembly**
  - [x] `Router::new().nest("/api/v1", ...)` with all handlers wired
- [x] **2.16 Tests `tests/api_public_read.rs`**
  - [x] Seed fixtures via `sqlx::query!` within transaction
  - [x] Assert JSON shape + pagination meta
  - [x] Filter combinations

---

## Phase 3 — Admin Write API (3–4 days)

**Goal:** Admin CRUD (replaces Blade), all mutations write `activity_logs`.

- [x] **3.1 `src/governance/activity_log.rs`**
  - [x] `record_activity(pool, actor_id, action, subject_type, subject_id, metadata) -> ActivityLog`
  - [x] Helper used by all admin write handlers
- [x] **3.2 `src/db/repositories/activity_logs.rs`**
  - [x] `find_many(filters, pagination)` — filter by `action`, `actor_id`, `subject_type`, `date_from`, `date_to`, `search` (actor name/email + subject_id + action)
- [x] **3.3 `src/db/repositories/media_files.rs`**
  - [x] `find_many(filters, pagination)`, `insert`, `delete_by_id`, `bulk_delete(ids)`
- [x] **3.4 `src/db/repositories/system_settings.rs`**
  - [x] `get(key, default)`, `set(key, value, type)`, `find_grouped()`, `bulk_update(map)`
  - [x] Boolean coercion for `type=boolean`
- [x] **3.5 `src/storage/r2.rs`** (deferred from Phase 0 stub)
  - [x] `upload(category: &str, bytes: Vec<u8>, content_type: &str) -> {path, public_url}`
  - [x] `delete(path: &str) -> bool`
  - [x] `exists(path: &str) -> bool`
  - [x] `generate_public_url(path)` via `R2_PUBLIC_URL` config
  - [x] Filename sanitize: slug + timestamp + 8-char random prefix
  - [x] Validate mime + max size per category (port `filesystems.upload_categories`)
- [x] **3.6 `src/api/rest/admin/topics.rs`**
  - [x] `PUT/PATCH /admin/topics/{id}`, `DELETE /admin/topics/{id}`
  - [x] `PATCH /admin/topics/{id}/reorder` (swap up/down via `BEGIN...SELECT FOR UPDATE...UPDATE "order"`)
  - [x] `PATCH /admin/topics/{id}/publish` (toggle `is_published`)
  - [x] Each mutation → `record_activity`
- [x] **3.7 `src/api/rest/admin/contents.rs`**
  - [x] `POST /admin/contents`, `PUT/PATCH /admin/contents/{id}`, `DELETE /admin/contents/{id}`
  - [x] Reorder + publish toggles
- [x] **3.8 `src/api/rest/admin/marketplace_tasks.rs`**
  - [x] `POST /admin/marketplace-tasks`, `PUT/PATCH /admin/marketplace-tasks/{id}`, `DELETE /admin/marketplace-tasks/{id}`
  - [x] `PATCH /admin/marketplace-tasks/{id}/status` (writes `update_task_status` activity)
- [x] **3.9 `src/api/rest/admin/student_progress.rs`**
  - [x] `POST /admin/student-progress`, `PUT/PATCH /admin/student-progress/{id}`, `DELETE /admin/student-progress/{id}`
- [x] **3.10 `src/api/rest/admin/uploads.rs`**
  - [x] `POST /admin/upload/{category}` (avatars|gallery|materials|attachments)
  - [x] `DELETE /admin/upload/{category}?path=...`
- [x] **3.11 `src/api/rest/admin/activity_logs.rs`**
  - [x] `GET /admin/activity-logs` (paginated, all filters)
- [x] **3.12 `src/api/rest/admin/homepage_sections.rs`**
  - [x] `PATCH /admin/homepage-sections` (bulk update position + is_enabled in DB transaction)
  - [x] Writes `update_homepage_sections` activity log
- [x] **3.13 `src/api/rest/admin/system_settings.rs`**
  - [x] `GET /admin/settings` (grouped by `group`)
  - [x] `PATCH /admin/settings` (settings map → type coercion per field)
- [x] **3.14 Router mounting with `require_role(admin)`**
  - [x] All `/api/v1/admin/*` routes guarded
- [x] **3.15 Tests `tests/api_admin_write.rs`**
  - [x] CRUD each entity
  - [x] Activity log recorded per mutation
  - [x] Upload via `mockito` mock S3 endpoint
  - [x] Reorder transaction safety

---

## Phase 4 — Recommendation Engine (4–5 days)

**Goal:** `GET /homepage-recommendations` fully featured (admin-curated + topic normalized + personalization + system assignments).

- [x] **4.1 Relocate `kurikulum_merdeka_structure.json`**
  - [x] Move `resources/json/kurikulum_merdeka_structure.json` → `src/recommendation/data/`
  - [x] Embed via `include_str!("data/kurikulum_merdeka_structure.json")`
  - [x] Keep `resources/` copy for now (Laravel still runs parallel)
- [x] **4.2 `src/recommendation/mod.rs`**
  - [x] Declare submodules: `taxonomy`, `aggregation`, `personalization`, `assignments`
- [x] **4.3 `src/recommendation/taxonomy.rs`** (port `MediaPromptTaxonomyInferenceService`)
  - [x] `TaxonomyCatalog` struct loaded once at boot (`Arc<TaxonomyCatalog>` in `AppState`)
  - [x] `SubjectsJsonTaxonomyCatalog` parser
  - [x] `infer(prompt) -> TaxonomyInferenceResult`
  - [x] Scoring weights EXACTLY: subject phrase 7, sub_subject phrase 12, token overlaps 1.5/2.75/0.75/0.35
  - [x] Normalize /24, threshold 0.25 OR phrase match
  - [x] Detect jenjang (SD/SMP/SMA/SMK), class number (numeric + roman), semester, bab
  - [x] Output schema `media_prompt_taxonomy_inference.v1` with confidence label, best match (subject_id/sub_subject_id resolved from DB), candidate matches
- [x] **4.4 `src/recommendation/personization.rs`** (port `RecommendationPersonalizationService`)
  - [x] `resolve(user: Option<User>) -> PersonalizationContext`
  - [x] Guests → `global_feed` mode
  - [x] Authenticated → aggregate authored-topic activity by sub_subject
  - [x] Compute preferred (matching primary subject) + secondary sub_subject_ids
  - [x] `subject_anchor` from profile or fallback activity
- [x] **4.5 `src/recommendation/aggregation.rs`** (port `RecommendationAggregationService`)
  - [x] `build_feed_snapshot(at, ctx) -> Snapshot { items, source_status, personalization }`
  - [x] Combine admin-curated `RecommendedProject` (`visibleAt`) + normalized Topic items
  - [x] `selectSystemGeneratedCandidates` — apply personalization when signals available
  - [x] `build_system_distribution_summary(min_user_count)` — grouped by sub_subject, cap `maximum_items_per_sub_subject`
- [x] **4.6 `src/recommendation/assignments.rs`**
  - [x] `SystemRecommendationAssignmentsRepo` with upsert `(user_id, recommendation_key)` ON CONFLICT update `last_distributed_at`
- [x] **4.7 `src/db/repositories/recommended_projects.rs`**
  - [x] `find_visible_at(at)`, `find_by_id`, `create`, `update`, `delete`, `toggle_active`, `show_now`
  - [x] Filter by `source_type`, `status`
- [x] **4.8 `src/api/rest/homepage_recommendations.rs`**
  - [x] `GET /v1/homepage-recommendations?limit=`
  - [x] Load `HomepageSection` by key `personalized_project_recommendations.homepage.section_key` (`project_recommendations`)
  - [x] Resolve optional user (Bearer header)
  - [x] If section disabled → return empty collection + context meta
  - [x] Otherwise `build_feed_snapshot` + system assignments tracking
  - [x] Return `RecommendedProjectRecommendationCollection` with `{section, limit, personalization, source_status}` meta
- [x] **4.9 Subjects/sub_subjects seed verification**
  - [x] Check if Neon has data (run SELECT count FROM subjects/sub_subjects)
  - [x] If empty, create migration `000016_seed_subjects_sub_subjects_from_taxonomy.sql` (extract from `kurikulum_merdeka_structure.json`)
- [x] **4.10 Tests `tests/recommendation.rs`**
  - [x] Snapshot test: input "handout pecahan kelas 5 SD" → expected subject (math) + sub_subject match
  - [x] Confidence threshold boundary
  - [x] Guest vs authenticated personalization contexts
  - [x] Feed snapshot composition (admin + system items)

---

## Phase 5 — LLM Provider & Governance (5–6 days, densest)

**Goal:** Inline LLM adapter — Rust calls OpenRouter directly; cache, rate-limit, audit ledger operational.

- [ ] **5.1 `src/providers/mod.rs`**
  - [ ] Declare `Provider` trait with `async complete(req: CompletionRequest) -> Result<CompletionResponse>`
  - [ ] Declare `OpenRouterProviderClient`, `ProviderRouter`
- [ ] **5.2 `src/providers/openrouter.rs`**
  - [ ] POST `{openrouter_base_url}/chat/completions`
  - [ ] Headers: `Authorization: Bearer {openrouter_api_key}`, `HTTP-Referer: klass-mobile`, `X-Title: klass-gateway`
  - [ ] Body: `model={openrouter_model}`, `messages`, `response_format: {type: json_object}` for interpret/draft/respond
  - [ ] Parse `choices[0].message.content` (fallback: `output_text`, `choices[0].message.content`, `content` array)
  - [ ] Timeout from config, retry 2× backoff 500 ms
- [ ] **5.3 `src/providers/router.rs`**
  - [ ] Primary + fallback provider selection
  - [ ] Circuit breaker via `tower::limit` + `tower::retry`
  - [ ] HTTP/2 connection pooling via reqwest
- [ ] **5.4 `src/contracts/` scaffolding**
  - [ ] `mod.rs` declares all contract modules
  - [ ] `prompt_interpretation.rs` (port `MediaPromptInterpretationSchema`)
  - [ ] `content_draft.rs` (port `MediaContentDraftSchema`)
  - [ ] `delivery.rs` (port `MediaDeliveryResponseSchema`)
  - [ ] `artifact_metadata.rs` (port `MediaArtifactMetadataContract`)
  - [ ] `generation_spec.rs` (port `MediaGenerationSpecContract`)
  - [ ] `taxonomy_inference.rs` (port `MediaPromptTaxonomyInferenceService` output schema)
  - [ ] Each: `SchemaVersion`, `decode_and_validate`, `fallback` constructors
  - [ ] Use `serde` + `garde` for validation rules (EXACT field RNG from Laravel)
- [ ] **5.5 `src/cache/mod.rs`** (`LlmCacheRepo` over `llm_cache_entries`)
  - [ ] `cache_key = sha256(canonical_json(request_payload))` (byte-compatible with Python key)
  - [ ] `pg_try_advisory_lock(cache_key::bigint)` anti-stampede
  - [ ] `lookup(key, route) -> Option<response>`; on hit: `hit_count++`, `last_hit_at=now`
  - [ ] `store(key, route, request, response, ttl)`
  - [ ] TTL + route-aware (`interpret` / `respond`)
  - [ ] Lazy cleanup of expired entries (background or on lookup)
- [ ] **5.6 `src/governance/rate_limit.rs`**
  - [ ] `RateLimitPoliciesRepo` over `llm_rate_limit_policies`
  - [ ] `RateLimitBucketsRepo` over `llm_rate_limit_buckets`
  - [ ] Fixed-window increment: `INSERT ... ON CONFLICT DO UPDATE SET request_count = ...`
  - [ ] Per-route budget check preflight
  - [ ] Deny/degrade exhaustion actions
- [ ] **5.7 `src/governance/ledger.rs`**
  - [ ] `LedgerRepo` over `llm_request_ledger`
  - [ ] Record each request: `request_id`, `generation_id`, `route`, provider, `latency_ms`, tokens, `cache_status`, `fallback_used`, `final_status`
- [ ] **5.8 `src/governance/price_catalog.rs`**
  - [ ] `PriceCatalogRepo` over `llm_price_catalog`
  - [ ] Migration `000105_seed_deepseek_price.sql` — hardcoded Deepseek V4 Flash pricing (input/output per 1M tokens)
  - [ ] Cost estimate helper
- [ ] **5.9 `src/llm/mod.rs`**
  - [ ] Declare `interpret`, `draft`, `respond` submodules
- [ ] **5.10 `src/llm/interpret.rs`** (port `MediaPromptInterpretationService`)
  - [ ] `InterpretService::interpret(generation)`
  - [ ] Build interpretation request payload (`MediaPromptInterpretationRequestContract::from_generation`)
  - [ ] Resolve taxonomy inference fallback when subject/SubSubject missing
  - [ ] Cache lookup; on miss call OpenRouter
  - [ ] Validate via `MediaPromptInterpretationSchema::decode_and_validate`
  - [ ] Enrich with subject/sub_subject context from taxonomy inference
  - [ ] Build `interpretation_audit_payload` (provider metadata, request payload, request meta, response, taxonomy inference, normalized payload, used_fallback, fallback_error)
  - [ ] Persist `llm_provider`, `llm_model`, `interpretation_payload`, `interpretation_audit_payload`
  - [ ] On contract failure → `MediaPromptInterpretationSchema::fallback(...)`
  - [ ] Write to ledger + rate-limit bucket
- [ ] **5.11 `src/llm/draft.rs`** (port `MediaContentDraftingService`)
  - [ ] `DraftService::draft(generation, decision)`
  - [ ] Validate interpretation payload
  - [ ] Cache lookup; call OpenRouter if miss (or deterministic fallback if adapter unconfigured)
  - [ ] POST `MediaContentDraftRequestContract` (deliverable includes `resolved_output_type`, `interpretation`, `taxonomy_hint`)
  - [ ] Validate via `MediaContentDraftSchema::decode_and_validate`
  - [ ] On failure → `MediaContentDraftSchema::fallback_from_interpretation` with `content_integrity` score 1.0
  - [ ] Return `{payload, source, adapter_metadata, fallback_error}`
- [ ] **5.12 `src/llm/respond.rs`** (port `MediaDeliveryResponseService`)
  - [ ] `RespondService::compose(generation)`
  - [ ] Build context: title, preview_summary, teacher_message, recommended_next_steps, classroom_tips, artifact metadata, publication entities
  - [ ] If file_url exists → POST `MediaDeliveryRequestContract` (HMAC-signed to LLM_ADAPTER_FALLBACK_URL if set, else OpenRouter)
  - [ ] Validate via `MediaDeliveryResponseSchema::validate`
  - [ ] On failure → `MediaDeliveryResponseSchema::fallback(...)`
  - [ ] Persist `delivery_payload` (incl. `response_meta` {provider, model} + `fallback` block)
- [ ] **5.13 `LLM_ADAPTER_FALLBACK_URL` rollback path**
  - [ ] If env set (Python adapter URL), route interpret/draft/respond through it via HMAC
  - [ ] If not set, use OpenRouter direct
  - [ ] Toggle via config without code changes
- [ ] **5.14 Tests**
  - [ ] `tests/provider_openrouter.rs` — mockito mock `/chat/completions`
  - [ ] `tests/contracts.rs` — snapshot tests decode+validate+fallback per schema (mirror Laravel `tests/Unit`)
  - [ ] `tests/cache.rs` — concurrency test 50 tasks, exactly 1 provider call
  - [ ] `tests/governance.rs` — fixed-window counter, deny action, ledger insertion, cost estimate
  - [ ] `tests/llm_smoke.rs` — end-to-end interpret→draft→respond with mocked OpenRouter

---

## Phase 6 — Media Generation Orchestration & Worker (6–7 days)

**Goal:** Async pipeline `queued→interpreting→classified→generating→uploading→publishing→completed`.

- [ ] **6.1 `src/orchestrator/lifecycle.rs`**
  - [ ] Enum `MediaGenerationLifecycle` (9 states matching `GenerationStatus` proto)
  - [ ] `can_transition(from, to)` matrix (EXACT from Laravel)
  - [ ] `terminal_states()`, `STATUS_ORDER` ordering, `StatusBefore` invariant (no regression)
- [ ] **6.2 `src/orchestrator/audit_trail.rs`** (port `MediaGenerationAuditTrailService`)
  - [ ] `initialize(generation)` — base payload in transaction with `lockForUpdate`, idempotent
  - [ ] `transition(generation, to_status, context, attempt, job_context)` — validate via `can_transition`, compute timing, append to `status_history` (capped 50 events), update `status`/`error_code`/`error_message`
  - [ ] `record_attempt_failure(generation, throwable, context, attempt, job_context)` — append `attempt_failed`
  - [ ] `mark_failed(generation, throwable, ...)` — transition to `failed` if not completed/cancelled
  - [ ] Helpers: `lock_generation`, `base_payload`, `apply_runtime_metadata`, `apply_transition_timing`, `total_duration_ms`, `append_history`, `provider_trace`, `resolve_output_type`, `error_summary`, `safe_throwable_context`, `sanitize_message` (whitespace collapse + 240 char), `filter_context` (depth-limited scrubber)
  - [ ] Schema version `media_generation_orchestration_audit.v1`
- [ ] **6.3 `src/orchestrator/submission.rs`** (port `MediaGenerationSubmissionService`)
  - [ ] `create_or_reuse(teacher_id, raw_prompt, preferred_output_type, subject_id, sub_subject_id, provider_metadata)`
  - [ ] Compute `request_fingerprint` (sha256 over teacher/prompt/output/subject/sub_subject)
  - [ ] Compute `active_duplicate_key` when not terminal
  - [ ] DB transaction + `SELECT FOR UPDATE` on active duplicates
  - [ ] Reuse if found; else insert
  - [ ] Catch `QueryException` on unique-constraint race → retry lookup
  - [ ] `create_regeneration(parent, additional_prompt)` — combine prompts, `is_regeneration=true`, `generated_from_id`, `status=queued`
- [ ] **6.4 `src/orchestrator/decision.rs`** (port `MediaGenerationDecisionService`)
  - [ ] `resolve(generation)` — ensure interpretation exists, call `decide()`, call `draft()`, build `MediaGenerationSpecContract`, persist `resolved_output_type`, `decision_payload`, `generation_spec_payload`
  - [ ] `decide(interpretation, preferred_output_type)` — rank candidates, apply teacher override, interpretation constraints, keyword signals, tie-breaker `pdf→docx→pptx`
  - [ ] Returns: `schema_version=media_output_decision.v1`, `preferred_output_type`, `constraint_preferred_output_type`, `resolved_output_type`, `decision_source` (teacher_override|interpretation_constraint|candidate_ranking), `reason_code`, `reasoning`, `ranked_candidates`, `tie_breaker_applied`, `resolved_at`
- [ ] **6.5 `src/orchestrator/workflow.rs`** (port `MediaGenerationWorkflowService`)
  - [ ] `WorkflowService::process(generation_id, attempt, job_context)`
  - [ ] Sequential checkpointed steps wrapped by `timedStep` (via `tracing::span!`)
  - [ ] `ensureClassified`: `tokio::join!(interpret, draft)` parallel LLM calls
  - [ ] `ensureGenerated`: call Python renderer + transition to `uploading`
  - [ ] `ensurePublished`: publish entities
  - [ ] `ensureCompleted`: compose delivery payload + transition to `completed`
  - [ ] Status transitions via audit-trail service `transition` (validates lifecycle matrix)
- [ ] **6.6 `src/media_gen/python_client.rs`** (port `PythonMediaGeneratorClient`)
  - [ ] `PythonMediaGeneratorClient::generate(generation)`
  - [ ] Validate `MediaGenerationSpecContract` payload
  - [ ] Build request: `{generation_id, generation_spec, contracts:{generation_spec, artifact_metadata}}`
  - [ ] HMAC-sign via `InterServiceRequestSigner` (Phase 1.7)
  - [ ] Headers: `X-Klass-Generation-Id`, `X-Klass-Request-Timestamp`, `X-Klass-Signature-Algorithm: hmac-sha256`, `X-Klass-Signature`
  - [ ] POST `{media_gen_url}/v1/generate`
  - [ ] Decode artifact metadata via `MediaArtifactMetadataContract::validate`
  - [ ] Persist `resolved_output_type`, `generator_provider`, `generator_model`, `generator_service_response`, `mime_type`
  - [ ] Error code map: `error.laravel_error_code_hint` if present; 5xx/429 → `PYTHON_SERVICE_UNAVAILABLE`; else → `ARTIFACT_INVALID`
  - [ ] Timeout 60s, retry 2× backoff 500 ms
- [ ] **6.7 `src/media_gen/publication.rs`** (port `MediaPublicationService`)
  - [ ] `MediaPublicationService::publish(generation)`
  - [ ] DB transaction + lock generation
  - [ ] `prepareArtifactForPublication`: upload artifact to R2 (`storage_path`)
  - [ ] Validate artifact integrity: PDF header `%PDF`+EOF, OOXML zip entries, MIME type, size, SHA256 checksum
  - [ ] Thumbnail strategy: request `thumbnail_url` from Python renderer first; if absent, minimal Rust impl:
    - [ ] PDF → `pdfium-render` crate (render page 0 to 800×600 PNG)
    - [ ] PPTX/DOCX → `zip` crate extract `docProps/thumbnail.jpeg|png` or first `ppt/media/image*` / `word/media/image*`
    - [ ] Fallback SVG visual (1280×720, palette per format: PDF red, PPTX orange, default blue)
  - [ ] Resolve or create `Topic` (from interpretation taxonomy)
  - [ ] Resolve or create `Content` (type from `resolved_output_type`)
  - [ ] Resolve or create `RecommendedProject` (`source_type=ai_generated`)
  - [ ] Persist `delivery_payload` on generation
  - [ ] Compensation: `compensate_uploaded_files` (delete R2 uploads) on failure in catch/finally
  - [ ] Cleanup temp files in finally
- [ ] **6.8 `src/queue/mod.rs`**
  - [ ] Declare `redis_streams`, `worker`, `dead_letter`
- [ ] **6.9 `src/queue/redis_streams.rs`**
  - [ ] `enqueue(generation_id, attempt)` XADD `stream=klass:media-gen` with `{generation_id, attempt}`
  - [ ] Idempotent `XGROUP CREATE` on boot (`consumer_group=klass-workers`)
  - [ ] Honor `MEDIA_GENERATION_QUEUE_CONCURRENCY` env (default 1)
- [ ] **6.10 `src/queue/worker.rs`**
  - [ ] `Worker::run()` loop `XREADGROUP >` `count=10 block=5s`
  - [ ] Dispatch to `WorkflowService::process`
  - [ ] `XACK` on success
  - [ ] `XCLAIM` after idle-timeout 300s for recovery
  - [ ] Honor max tries 3 → else move to DLQ
- [ ] **6.11 `src/queue/dead_letter.rs`**
  - [ ] After N attempts → `XADD klass:media-gen-dlq` + `mark_failed`
  - [ ] Admin manual retry endpoint helper (used in Phase 7)
- [ ] **6.12 Worker entrypoint**
  - [ ] `src/main.rs` parses `--worker` arg
  - [ ] With `--worker`: run `Worker::run()` (no HTTP server)
  - [ ] Render service `klass-gateway-worker` uses same image, startCommand `-worker`
- [ ] **6.13 Tests**
  - [ ] `tests/orchestrator_lifecycle.rs` — transition matrix valid/invalid, StatusBefore invariant
  - [ ] `tests/workflow.rs` — end-to-end with mockito Python renderer + OpenRouter + R2 mock
  - [ ] Verify all state transitions + `orchestration_audit_payload` shape
  - [ ] `tests/queue.rs` — Redis test instance (testcontainers-rs or in-memory trait), enqueue→consume→ack
  - [ ] `tests/publication.rs` — artifact integrity validation (PDF/OOXML), compensation on failure

---

## Phase 7 — Media Generation REST + Freelancer (3–4 days)

**Goal:** Teacher + freelancer endpoints used by Flutter.

- [ ] **7.1 DB migration `000016_alter_marketplace_tasks_status_to_varchar.sql`**
  - [ ] ALTER `status` enum to VARCHAR(20) without CHECK constraint
  - [ ] Drop existing enum constraint
  - [ ] Backfill existing rows (no-op, keeps values)
- [ ] **7.2 `src/db/repositories/media_generations.rs`**
  - [ ] `MediaGeneration` struct `#[derive(FromRow)]` with all fields incl. JSONB columns as `serde_json::Value`
  - [ ] `find_recent_for_teacher(teacher_id, limit=20)` — eager `subject`, `sub_subject.subject`, `topic`, `content`, `recommended_project`
  - [ ] `find_by_id_for_teacher(id, teacher_id)`
  - [ ] `find_chain(parent_id)` — walk `get_original_generation` to root (depth 50) + direct children oldest-first
  - [ ] `insert`, `update_status`, `update_payloads`
- [ ] **7.3 `src/api/rest/media_generations.rs`** (4 teacher-only endpoints)
  - [ ] `require_teacher(request)` helper → 401 if not teacher
  - [ ] `GET /media-generations?parent_id=` — chain walk or 20 recent
  - [ ] `POST /media-generations` — `SubmissionService::create_or_reuse`; if was_created → `queue.enqueue`. Return 202 + `MediaGenerationResource`
  - [ ] `GET /media-generations/{id}` — scoped to teacher
  - [ ] `POST /media-generations/{id}/regenerate` — parent must be terminal, `create_regeneration`, enqueue
- [ ] **7.4 `MediaGenerationResource` serde struct**
  - [ ] All public fields + nested `subject`, `sub_subject`, `topic`, `content`, `recommended_project`
  - [ ] Match Laravel resource shape exactly for Flutter compatibility
- [ ] **7.5 `src/matching/mod.rs`** (port `FreelancerMatchingService`)
  - [ ] `find_best_matches(generation, limit=5)`
  - [ ] Fetch all `role=freelancer` users
  - [ ] Deterministic MD5-derived scores: portfolio 0.4-1.0, success 0.7-1.0, availability 0.5-1.0
  - [ ] Combine `0.5*p + 0.3*s + 0.2*a`
  - [ ] Return top N with `portfolio_relevance_score`, `success_rate`, `availability_score`, `match_score`
- [ ] **7.6 `src/db/repositories/freelancer_matches.rs`**
  - [ ] `upsert(media_generation_id, freelancer_id, scores)`, `find_for_generation(id)`
  - [ ] Unique constraint `(media_generation_id, freelancer_id)`
- [ ] **7.7 `src/api/rest/freelancer.rs`**
  - [ ] `POST /media-generations/{id}/suggest-freelancers` — `find_best_matches`, clamp `max_suggestions` 1..10, upsert `freelancer_matches`, return list
  - [ ] `POST /media-generations/{id}/hire-freelancer` — validate generation terminal + has `content_id`
  - [ ] `auto_suggest` mode → create `MarketplaceTask` `task_type=suggestion, status=assigned, suggested_freelancer_id`
  - [ ] `manual_task` mode → create `MarketplaceTask` `task_type=bid, status=open_for_bid`
- [ ] **7.8 Tests**
  - [ ] `tests/api_media_gen.rs` — store → 202, show, regenerate (terminal parent), index (parent_id chain)
  - [ ] `tests/api_freelancer.rs` — suggest deterministic, hire auto_suggest + manual_task
  - [ ] `tests/matching.rs` — score determinism for given user set

---

## Phase 8 — Admin Debug + OpenAPI + Deploy Cutover (2–3 days)

**Goal:** Laravel dependency fully eliminated; production on Rust only.

- [ ] **8.1 `src/api/rest/admin/media_generations_debug.rs`**
  - [ ] `GET /v1/admin/media-generations/{id}/debug-taxonomy` → `MediaGenerationTaxonomyDebugResource`
  - [ ] Includes interpretation + taxonomy inference + decision metadata
- [ ] **8.2 `src/api/rest/admin/media_generations.rs`**
  - [ ] `GET /v1/admin/media-generations` paginated (15)
  - [ ] Filters: `status` (validated against `MediaGenerationLifecycle::all()`), `search` (id, raw_prompt, teacher name/email)
- [ ] **8.3 `src/api/rest/admin/recommended_projects.rs`**
  - [ ] `POST /admin/homepage-sections/recommended-projects` — upload thumbnail + project_file (PDF/PPT/DOC etc.), auto-generate thumbnail from project file when none supplied
  - [ ] `PUT /admin/homepage-sections/recommended-projects/{id}`
  - [ ] `DELETE /admin/homepage-sections/recommended-projects/{id}`
  - [ ] `PATCH .../toggle-active`, `PATCH .../show-now` (clear starts_at, activate)
  - [ ] Each mutation → `ActivityLog` (`create_recommended_project`, `update_recommended_project`, `delete_recommended_project`)
- [ ] **8.4 OpenAPI via `utoipa`**
  - [ ] `#[utoipa::path]` on every handler
  - [ ] `#[derive(ToSchema)]` on all resource structs
  - [ ] `GET /api-docs/openapi.json`
  - [ ] `GET /api-docs/swagger-ui` (or `/docs`)
- [ ] **8.5 Smoke test CLI args**
  - [ ] `--smoke-llm` (port `LlmAdapterSmokeTestService`) — exercise interpret + respond with synthetic signed payload, assert `fallback.triggered=false` + `llm_used=true`
  - [ ] `--smoke-python` (port `PythonMediaGeneratorHealthCheckService`) — assert `/v1/health` schema, supported_formats `[docx,pdf,pptx]`, contract versions
  - [ ] Render `postDeployCommand` runs both; fail = rollback
- [ ] **8.6 `render.yaml` final**
  - [ ] 2 services: `klass-gateway` (web) + `klass-gateway-worker` (worker)
  - [ ] Env group wired
  - [ ] Healthcheck `/health`
  - [ ] Resource sizing validated against smoke test
- [ ] **8.7 Staging cutover**
  - [ ] Deploy Rust image to staging Render
  - [ ] Run smoke tests (`--smoke-llm`, `--smoke-python`)
  - [ ] Point Flutter `BASE_URL` to staging Rust
  - [ ] Monitor 24h: API errors, latency, worker queue depth, LLM cost via `llm_request_ledger`
  - [ ] Verify R2 uploads + media-gen pipeline end-to-end
- [ ] **8.8 Production cutover**
  - [ ] Switch production Render service to Rust binary
  - [ ] Update Flutter `BASE_URL` (or DNS)
  - [ ] Set Laravel service to read-only mode (or pause)
  - [ ] Monitor 48h
- [ ] **8.9 Decommission Laravel**
  - [ ] Delete `app/`, `routes/`, `database/`, `config/`, `bootstrap/`, `resources/` (keep `resources/json/kurikulum_merdeka_structure.json` relocated in 4.1)
  - [ ] Delete `composer.json`, `composer.lock`, `phpunit.xml`, `phpunit.xml.dist`
  - [ ] Delete `.github/workflows/tests.yml` (PHP CI)
  - [ ] Delete `docker-compose.yml` Laravel-oriented entries (replace with Rust-only)
  - [ ] Remove `artisan`, `package.json`, `vite.config.js` if only Laravel-used
  - [ ] Final commit: repo is Pure Rust

---

## Rollback Strategy

- [ ] Document runbook: at any phase, re-point `BASE_URL` to Laravel service; same Neon DB works byte-compatible.
- [ ] Worker crashes → dead-letter queue → admin manual retry via `POST /v1/admin/media-generations/{id}/retry`.
- [ ] LLM OpenRouter degrades → set `LLM_ADAPTER_FALLBACK_URL` to Python adapter URL (HMAC) without code change. Disable after 1 week stable.
- [ ] Token scheme non-compatible → no users yet → risk nil (per user 2026-07-13).

---

## Dependency Order

```
Phase 0 ─┬─► Phase 1 ──┬─► Phase 2 ──┬─► Phase 3 ──┐
         │            │            │            │
         └─► Phase 5 ─┴──────────► Phase 6 ──► Phase 7 ──► Phase 8
                                   ▲
                         Phase 4 ──┘
```

## Total Estimates

- Phases 0–3: 9–12 days (auth + read + admin write) → MVP replacing ~70% of Flutter requests
- Phases 4–5: 9–11 days (recommendation + LLM inline) → AI core
- Phases 6–7: 9–11 days (orchestration worker + REST) → full media generation
- Phase 8: 2–3 days (deploy + decommission)
- **Total: ~29–37 working days** (~6–8 weeks solo, ~3–4 weeks with 2–3 parallel devs)

## High-Risk Items

1. **Thumbnail generation** (Phase 6) — delegate to Python renderer if possible; native Rust deps (pdfium/zip) heavy.
2. **`marketplace_tasks.status` enum mismatch** (Phase 7.1) — ALTER migration needed.
3. **`cargo sqlx prepare` early** (Phase 0.5) — Docker build breaks without it once first `query!` macro lands.
4. **OpenRouter JSON mode** (Phase 5) — Deepseek V4 Flash JSON is stable; schema fallback contracts retained.
5. **Subjects/sub_subjects bootstrap** (Phase 4.9) — verify Neon has data or seed from taxonomy JSON.
6. **Render 512 MB memory** — thumbnailing + R2 multipart upload RAM-heavy; keep worker concurrency=1 initially.