# Performance Baseline — Monitoring & Measurement Guide

> **Created**: 2026-07-11
> **Purpose**: Document the complete approach for capturing performance baselines before the Rust gateway migration.

---

## 1. Tooling Summary

| Tool | Purpose | Location |
|------|---------|----------|
| **Sentry** | APM, distributed tracing, profiling | `config/sentry.php`, `composer.json` |
| **StructuredApiLogger** | Per-request timing (p50/p95/p99) | `app/Http/Middleware/StructuredApiLogger.php` |
| **TracksStepTiming** | Per-step HTTP call timing in pipeline | `app/Services/Concerns/TracksStepTiming.php` |
| **AuditTrailService** | Per-status duration tracking | `app/Services/MediaGenerationAuditTrailService.php` |
| **perf:baseline** | Artisan command for baseline dump | `app/Console/Commands/PerformanceBaselineDump.php` |
| **k6 baseline.js** | Load test: per-endpoint p50/p95/p99 | `tests/load/baseline.js` |
| **k6 media_generation_e2e.js** | E2E latency: submit → completed | `tests/load/media_generation_e2e.js` |
| **SQL: performance_baseline** | DB queries for audit trail metrics | `tests/load/sql/performance_baseline.sql` |
| **SQL: cache_hit_ratio** | LLM Adapter cache hit ratio | `tests/load/sql/cache_hit_ratio_baseline.sql` |

---

## 2. Sentry Setup

### 2.1 Install

```bash
cd backend
composer require sentry/sentry-laravel
php artisan sentry:publish --dsn=YOUR_DSN
```

### 2.2 Environment Variables

```env
SENTRY_LARAVEL_DSN=https://xxx@sentry.io/yyy
SENTRY_ENVIRONMENT=production
SENTRY_TRACES_SAMPLE_RATE=0.5
SENTRY_PROFILES_SAMPLE_RATE=0.2
SENTRY_TRACING_ENABLED=true
```

### 2.3 What Sentry Captures

- **Error tracking**: All unhandled exceptions with stack traces
- **Performance traces**: Per-request span with DB queries, HTTP calls
- **Profiling**: CPU/memory profiles for sampled requests (20%)
- **Breadcrumbs**: SQL queries, queue info, command info

---

## 3. Per-Endpoint Metrics (p50/p95/p99)

### 3.1 From StructuredApiLogger

The `StructuredApiLogger` middleware logs every API request with `duration_ms` to `storage/logs/api.log`. To extract percentiles:

```bash
# Parse api.log for duration_ms values (JSON log lines)
cat storage/logs/api.log | \
  jq -r 'select(.context.duration_ms != null) | "\(.context.path) \(.context.duration_ms)"' | \
  sort | \
  awk '{data[$1][NR]=$2; count[$1]++} END {for (p in data) {n=count[p]; asort(data[p]); print p, "p50=" data[p][int(n*0.5)], "p95=" data[p][int(n*0.95)], "p99=" data[p][int(n*0.99)]}}'
```

### 3.2 From k6 Load Test

```bash
# Run the baseline load test
k6 run --env API_BASE_URL=https://your-api.com \
       --env TEST_TEACHER_EMAIL=teacher@example.com \
       --env TEST_TEACHER_PASSWORD=password123 \
       tests/load/baseline.js

# Results: tests/load/results/summary.json
```

### 3.3 From Sentry

After 7 days of Sentry data:
1. Go to Sentry → Performance → Endpoints
2. Sort by p95 latency
3. Export per-endpoint: p50, p95, p99, throughput, error rate

---

## 4. Media Generation E2E Latency

### 4.1 From Audit Trail (SQL)

```sql
-- Run tests/load/sql/performance_baseline.sql query #2
-- Returns: e2e_p50_ms, e2e_p95_ms, e2e_p99_ms
```

### 4.2 From Artisan Command

```bash
php artisan perf:baseline --days=7 --output=table
```

### 4.3 From k6 E2E Test

```bash
k6 run --env API_BASE_URL=https://your-api.com \
       --env TEST_TEACHER_EMAIL=teacher@example.com \
       --env MAX_POLL_SECONDS=300 \
       tests/load/media_generation_e2e.js
```

### 4.4 Per-Step Duration Breakdown

The `TracksStepTiming` trait logs per-step timing to the `media_generation` log channel:

```json
{
  "message": "media_generation.step_timing",
  "context": {
    "media_generation_id": "uuid",
    "step": "ensure_classified",
    "duration_ms": 2345.67,
    "status": "success"
  }
}
```

Steps tracked:
- `ensure_classified` — interpretation + classification (LLM calls)
- `ensure_generated` — Python renderer call
- `ensure_published` — S3 upload + thumbnail generation
- `ensure_completed` — delivery response composition

---

## 5. Memory/CPU Monitoring

### 5.1 Docker Stats (Hugging Face Spaces)

```bash
# Monitor running container
docker stats --format "table {{.Name}}\t{{.CPUPerc}}\t{{.MemUsage}}\t{{.NetIO}}"

# Continuous monitoring script
while true; do
  echo "$(date +%H:%M:%S) $(docker stats --no-stream --format '{{.CPUPerc}} {{.MemUsage}}' CONTAINER_NAME)"
  sleep 5
done
```

### 5.2 PHP-FPM Metrics

```bash
# Check PHP-FPM pool status
curl http://localhost:8000/fpm-status?json

# Key metrics:
# - active processes
# - idle processes
# - max children reached
# - slow requests
```

### 5.3 Sentry Profiling

Sentry profiles capture CPU/memory per request. After 7 days:
1. Go to Sentry → Profiling
2. Filter by endpoint
3. Check p95 memory usage and CPU time

---

## 6. Failure Mode Catalog

### 6.1 From Database

```sql
-- Run tests/load/sql/performance_baseline.sql query #7
-- Returns: error_code, error_class, message, retryable, occurrences
```

### 6.2 From Artisan Command

```bash
php artisan perf:baseline --days=30
```

The `Error Rates` section shows all error codes and their frequency.

### 6.3 From Logs

```bash
# Parse media_generation logs for failures
cat storage/logs/laravel-$(date +%Y-%m-%d).log | \
  jq 'select(.message == "media_generation.failed")' | \
  jq -s 'group_by(.context.error.error_code) | map({error: .[0].context.error.error_code, count: length})'
```

---

## 7. Cache Hit Ratio Baseline

### 7.1 From LLM Adapter Database

```sql
-- Run tests/load/sql/cache_hit_ratio_baseline.sql
-- Query #3 (from ledger) is most accurate
-- Query #4 shows daily trend
```

### 7.2 From LLM Adapter Ops Endpoint

```bash
# If LLM Adapter is running
curl https://klass-llm-adapter-prod.hf.space/v1/ops/summary?days=7

# Returns per-route metrics including cache_hit_ratio
```

### 7.3 Expected Baseline Values

| Metric | Interpret Route | Respond Route |
|--------|----------------|---------------|
| Cache hit ratio | Target: > 0.3 | Target: > 0.5 |
| Avg latency (cache hit) | < 50ms | < 50ms |
| Avg latency (cache miss) | < 5000ms | < 3000ms |
| Daily budget utilization | < 80% | < 80% |

---

## 8. Measurement Schedule

| Day | Action | Tool |
|-----|--------|------|
| Day 1 | Deploy Sentry, start k6 smoke test | Sentry + k6 |
| Day 2 | Run k6 load test (10→50→100 VUs) | k6 |
| Day 3 | Run k6 E2E test (5 generations) | k6 |
| Day 4 | Extract cache hit ratio from LLM Adapter | SQL + ops endpoint |
| Day 5 | Run `php artisan perf:baseline --days=7` | Artisan |
| Day 6 | Run k6 spike test (0→200 VUs) | k6 |
| Day 7 | Compile final baseline report | All tools |

---

## 9. Baseline Report Template

```markdown
## Performance Baseline Report (Date: YYYY-MM-DD)

### Per-Endpoint Latency
| Endpoint | p50 | p95 | p99 | Throughput (rps) |
|----------|-----|-----|-----|------------------|
| POST /auth/login | Xms | Xms | Xms | X |
| GET /topics | Xms | Xms | Xms | X |
| POST /media-generations | Xms | Xms | Xms | X |
| GET /media-generations/{id} | Xms | Xms | Xms | X |

### Media Generation E2E
| Metric | Value |
|--------|-------|
| p50 | Xs |
| p95 | Xs |
| p99 | Xs |
| Success rate | X% |

### Per-Step Duration
| Step | p50 | p95 | p99 |
|------|-----|-----|-----|
| interpretation | Xms | Xms | Xms |
| classification | Xms | Xms | Xms |
| generation | Xms | Xms | Xms |
| upload | Xms | Xms | Xms |
| publication | Xms | Xms | Xms |
| delivery | Xms | Xms | Xms |

### Cache Hit Ratio
| Route | Hit Ratio | Avg Latency (hit) | Avg Latency (miss) |
|-------|-----------|-------------------|---------------------|
| interpret | X% | Xms | Xms |
| respond | X% | Xms | Xms |

### Error Rates
| Error Code | Count | Percentage |
|------------|-------|------------|
| ... | ... | ...% |

### Resource Usage
| Metric | Value |
|--------|-------|
| Peak CPU | X% |
| Peak Memory | XMB |
| Max Active Connections | X |
```
