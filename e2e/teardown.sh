#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# E2E Teardown Script
#
# Stops all E2E services and removes volumes.
#
# Usage:
#   bash e2e/teardown.sh
# ═══════════════════════════════════════════════════════════════════════════════

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATEWAY_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="$GATEWAY_DIR/docker-compose.e2e.yml"

RED='\033[0;31m'
GREEN='\033[0;32m'
CYAN='\033[0;36m'
NC='\033[0m'

echo -e "${CYAN}[e2e]${NC} Tearing down E2E stack..."

cd "$GATEWAY_DIR"
docker compose -f "$COMPOSE_FILE" down -v --remove-orphans

echo -e "${GREEN}[e2e] ✅${NC} All E2E containers and volumes removed"
