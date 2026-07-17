# E2E Integration Test Suite — Async Media Generation

End-to-end tests for the async media generation pipeline:
**Mobile → Rust Gateway → Redis → Python Worker → S3 → Webhook → Rust → Mobile**

## Architecture Under Test

```
┌──────────┐  POST /media-generations  ┌──────────────┐  XADD   ┌───────┐  POST /v1/jobs  ┌──────────────┐
│  curl    │ ──────────────────────────▶│ Rust Gateway │ ──────▶ │ Redis │ ──────────────▶│ Python API   │
│ (simulates│                           │ (REST server)│         │       │                │ (FastAPI)    │
│  mobile) │ ◀── GET /job-status ──────│              │◀─ POST ─│       │◀── enqueue ────│              │
└──────────┘                            │              │ webhook │       │                └──────┬───────┘
                                        │              │         │       │                       │
                                        └──────────────┘         └───────┘                       ▼
                                                                                         ┌──────────────┐
                                                                                         │ Python Worker│
                                                                                         │ (Arq)        │
                                                                                         │              │
                                                                                         │ generate →   │
                                                                                         │ upload S3 →  │
                                                                                         │ webhook      │
                                                                                         └──────┬───────┘
                                                                                                │
                                                                                                ▼
                                                                                        ┌──────────────┐
                                                                                        │ MinIO / S3   │
                                                                                        │ (artifact)   │
                                                                                        └──────────────┘
```

## Prerequisites

- Docker & Docker Compose v2
- `curl`, `jq` (for the test script)
- ~2 GB free disk (Docker images)

## Quick Start

```bash
# 1. Start all services (first run takes ~3-5 min to build)
cd gateway/
bash e2e/start-all.sh --detach

# 2. Run the E2E test
bash e2e/run-e2e-test.sh

# 3. Tear down when done
bash e2e/teardown.sh
```

## Services

| Service | Port (host) | Description |
|---------|-------------|-------------|
| Rust Gateway | `8080` | REST API server |
| Python API | `7860` | FastAPI media generator |
| PostgreSQL | `5433` | Application database (mapped from container 5432) |
| Redis | `6380` | Job queue (Arq) + Redis Streams (mapped from container 6379) |
| MinIO S3 | `9000` | S3/R2-compatible object storage |
| MinIO Console | `9001` | MinIO web UI |

## What the Test Verifies

### Sub-task 3.1.2 — Happy Path

1. **Register** a test teacher user via `POST /auth/register`
2. **Submit** a media generation via `POST /media-generations` → expects `202 Accepted`
3. **Verify** response contains `generation_id`, `job_id`, `status: "pending"`, and `poll_url`
4. **Poll** `GET /media-generations/{id}/job-status` until status becomes `completed` or `failed`
5. **Verify** the webhook was delivered: DB status updated to `completed`

### Sub-task 3.1.3 — Presigned URL Download

1. **Extract** `presigned_download_url` from the job status response
2. **HEAD** the presigned URL → expects `200 OK` (URL is accessible)
3. **GET** the presigned URL → expects `200 OK` with artifact bytes and content-type

## Configuration

All environment variables are set in `docker-compose.e2e.yml`. Key shared secrets:

| Variable | Value (E2E only) |
|----------|-------------------|
| `MEDIA_GEN_HMAC_SECRET` | `test-hmac-shared-secret-e2e` |
| `MEDIA_GEN_WEBHOOK_SECRET` | `test-webhook-shared-secret-e2e` |
| `MEDIA_GENERATION_PYTHON_SHARED_SECRET` | `test-hmac-shared-secret-e2e` |

> ⚠️ These are **test-only** secrets. Never use in production.

## Troubleshooting

### "Gateway health check failed"

The Rust Gateway compiles from source on first run (`cargo watch`). Allow 2-3 minutes.

### "Job did not complete within timeout"

Check Python Worker logs:
```bash
docker compose -f docker-compose.e2e.yml logs python-worker
```

### "Presigned URL returned 403"

MinIO may not have the bucket. Check init-minio:
```bash
docker compose -f docker-compose.e2e.yml logs init-minio
```

### Full log dump

```bash
docker compose -f docker-compose.e2e.yml logs --tail=100
```
