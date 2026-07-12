# ═══════════════════════════════════════════════════════
# Stage 1: Builder
#   - rust:1.97 stable (plan specifies nightly for optimizations;
#     stable used here for production reliability — nightly
#     can be swapped via build arg if smaller binary is needed)
#   - Layer 1: system deps (slow-changing)
#   - Layer 2: cargo fetch (dependency cache)
#   - Layer 3: cargo build --release (source code)
# ═══════════════════════════════════════════════════════
FROM rust:1.97-slim-bookworm AS builder

WORKDIR /app

# ── System dependencies ──────────────────────
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    protobuf-compiler \
    && rm -rf /var/lib/apt/lists/*

# ── Dependency layer (cached unless Cargo.toml changes) ──
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs

# Explicit fetch for layer caching (plan: "cargo fetch")
RUN cargo fetch

# Dummy build to cache compiled dependencies
RUN cargo build --release
RUN rm -rf src

# ── Source code layer ────────────────────────
COPY src/ src/
COPY migrations/ migrations/
COPY proto/ proto/
COPY build.rs build.rs
COPY .sqlx/ .sqlx/

# Build with offline sqlx data (plan: SQLX_OFFLINE=true cargo build --release)
ENV SQLX_OFFLINE=true
RUN cargo build --release

# ═══════════════════════════════════════════════════════
# Stage 2: Runtime
#   - debian:bookworm-slim (plan spec)
#   - Copy binary only (no build tools)
#   - Target image: <30MB (plan spec)
# ═══════════════════════════════════════════════════════
FROM debian:bookworm-slim AS runtime

# Minimal runtime deps: TLS certs + health check curl
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Non-root user (security best practice)
RUN useradd --create-home --shell /bin/bash klass \
    && mkdir -p /opt/klass \
    && chown klass:klass /opt/klass

# Copy binary only (plan: "copy binary only")
COPY --from=builder /app/target/release/klass-gateway /usr/local/bin/klass-gateway

USER klass
WORKDIR /opt/klass

EXPOSE 8080 50051

# Health check (plan: GET /health → {"status":"ok"})
HEALTHCHECK --interval=30s --timeout=3s --retries=3 --start-period=10s \
    CMD curl -f http://localhost:8080/health || exit 1

ENTRYPOINT ["/usr/local/bin/klass-gateway"]
