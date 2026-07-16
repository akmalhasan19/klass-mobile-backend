# Klass API Gateway

The core orchestrator API gateway for the Klass educational platform, built in Rust using Axum and SQLx.
This service replaces the legacy PHP/Laravel backend.

## Architecture

- **Web Framework**: Axum
- **Database**: PostgreSQL (via Neon) + SQLx
- **Job Queue**: Redis (via Upstash)
- **Object Storage**: Cloudflare R2 / AWS S3
- **LLM Integration**: OpenRouter (primary) + Python Adapter (fallback)
- **Media Rendering**: Forwards to Python `media-generator-service` via HTTP/2 and HMAC-SHA256
- **Auth**: HMAC for inter-service communication

## Prerequisites

- Rust 1.97+
- PostgreSQL
- Redis

## Configuration

Copy `.env.example` to `.env` or `.env.local` and populate the values.
See `.env.example` for required variables.

## Running Locally

To run the REST server locally:

```bash
cargo run --features rest
```

To run the worker node:

```bash
cargo run --features worker -- --worker
```

## Testing

```bash
cargo test
```

## Deployment

This service is deployed using Render. See `render.yaml` for production deployment configuration and environment variable mappings.
