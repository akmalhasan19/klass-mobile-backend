#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# E2E Startup Script — Sub-task 3.1.1
#
# Starts all services required for the async media generation E2E test:
#   - PostgreSQL 17
#   - Redis 7
#   - MinIO (S3 mock)
#   - Rust Gateway (server + worker)
#   - Python API (FastAPI)
#   - Python Worker (Arq)
#
# Usage:
#   cd gateway/
#   bash e2e/start-all.sh           # start in foreground (Ctrl+C to stop)
#   bash e2e/start-all.sh --detach  # start in background (docker compose up -d)
# ═══════════════════════════════════════════════════════════════════════════════

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATEWAY_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="$GATEWAY_DIR/docker-compose.e2e.yml"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

log()   { echo -e "${CYAN}[e2e]${NC} $*"; }
ok()    { echo -e "${GREEN}[e2e] ✅${NC} $*"; }
warn()  { echo -e "${YELLOW}[e2e] ⚠️${NC} $*"; }
fail()  { echo -e "${RED}[e2e] ❌${NC} $*"; }

# ─── Pre-flight checks ──────────────────────────────────────────────────────

log "Pre-flight checks..."

if ! command -v docker &>/dev/null; then
    fail "docker is not installed or not in PATH"
    exit 1
fi

if ! docker info &>/dev/null; then
    fail "Docker daemon is not running"
    exit 1
fi

if ! command -v docker &>/dev/null || ! docker compose version &>/dev/null; then
    fail "docker compose (v2) is not available"
    exit 1
fi

ok "Docker is available"

# ─── Tear down any existing E2E stack ───────────────────────────────────────

log "Tearing down any existing E2E containers..."
docker compose -f "$COMPOSE_FILE" down -v --remove-orphans 2>/dev/null || true
ok "Clean slate ready"

# ─── Start services ──────────────────────────────────────────────────────────

DETAILED=""
if [[ "${1:-}" == "--detach" ]]; then
    DETAILED="-d"
fi

log "Building and starting all E2E services..."
cd "$GATEWAY_DIR"
docker compose -f "$COMPOSE_FILE" up --build $DETAILED

if [[ -n "$DETAILED" ]]; then
    log "Waiting for services to become healthy..."

    # Wait for all healthchecks to pass (max 180s)
    TIMEOUT=180
    ELAPSED=0
    while [[ $ELAPSED -lt $TIMEOUT ]]; do
        UNHEALTHY=$(docker compose -f "$COMPOSE_FILE" ps --format json 2>/dev/null | \
            grep -c '"healthy"' || echo "0")
        TOTAL=$(docker compose -f "$COMPOSE_FILE" ps --format json 2>/dev/null | \
            wc -l || echo "0")

        if [[ "$TOTAL" -gt 0 ]] && [[ "$UNHEALTHY" -eq "$TOTAL" ]]; then
            ok "All $TOTAL services are healthy!"
            break
        fi

        sleep 3
        ELAPSED=$((ELAPSED + 3))
        echo -ne "\r${CYAN}[e2e]${NC} Waiting... ${ELAPSED}s / ${TIMEOUT}s (${UNHEALTHY}/${TOTAL} healthy)"
    done

    if [[ $ELAPSED -ge $TIMEOUT ]]; then
        warn "Timeout waiting for health — some services may still be starting"
    fi

    echo ""
    log "Service URLs:"
    log "  Rust Gateway REST API : http://localhost:8080"
    log "  Python API            : http://localhost:7860"
    log "  MinIO Console         : http://localhost:9001"
    log "  PostgreSQL            : localhost:5433 (container 5432)"
    log "  Redis                 : localhost:6380 (container 6379)"
    echo ""
    log "Run the E2E test:  bash e2e/run-e2e-test.sh"
    log "Tear down:         docker compose -f docker-compose.e2e.yml down -v"
fi
