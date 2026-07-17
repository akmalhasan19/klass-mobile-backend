#!/usr/bin/env bash
# ═══════════════════════════════════════════════════════════════════════════════
# E2E Happy Path Test — Sub-tasks 3.1.2 & 3.1.3
#
# Tests the full async media generation flow:
#   Mobile → Rust Gateway → Redis → Python Worker → S3 → Webhook → Rust → Mobile
#
# Prerequisites:
#   - All services running (bash e2e/start-all.sh --detach)
#   - curl, jq installed
#
# Usage:
#   bash e2e/run-e2e-test.sh
#   GATEWAY_URL=http://localhost:8080 bash e2e/run-e2e-test.sh
#
# Exit codes:
#   0 — all tests passed
#   1 — one or more tests failed
# ═══════════════════════════════════════════════════════════════════════════════

set -euo pipefail
set +H  # Disable history expansion — passwords may contain '!' characters

# ─── Configuration ───────────────────────────────────────────────────────────

GATEWAY_URL="${GATEWAY_URL:-http://localhost:8080}"
# NOTE: Rust Gateway routes are mounted at root (e.g. /auth/register, /media-generations).
# The utoipa OpenAPI annotations include /api/v1 prefix for docs, but actual
# axum routes in api_router() do NOT nest under /api/v1.
API_BASE="${API_BASE:-$GATEWAY_URL}"
PYTHON_API_URL="${PYTHON_API_URL:-http://localhost:7860}"
POLL_MAX_ATTEMPTS="${POLL_MAX_ATTEMPTS:-60}"
POLL_INTERVAL="${POLL_INTERVAL:-3}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
GATEWAY_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
COMPOSE_FILE="$GATEWAY_DIR/docker-compose.e2e.yml"
COMPOSE_PROJECT="${COMPOSE_PROJECT_NAME:-}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

PASS_COUNT=0
FAIL_COUNT=0

# ─── Helpers ─────────────────────────────────────────────────────────────────

log()    { echo -e "${CYAN}[test]${NC} $*"; }
ok()     { echo -e "${GREEN}[test] ✅ PASS:${NC} $*"; PASS_COUNT=$((PASS_COUNT + 1)); }
fail()   { echo -e "${RED}[test] ❌ FAIL:${NC} $*"; FAIL_COUNT=$((FAIL_COUNT + 1)); }
warn()   { echo -e "${YELLOW}[test] ⚠️  WARN:${NC} $*"; }
header() { echo -e "\n${BOLD}═══ $* ═══${NC}"; }

# HTTP helper: POST JSON with optional auth header
post_json() {
    local url="$1" token="$2" body="$3"
    if [[ -n "$token" ]]; then
        curl -sS -w "\n%{http_code}" \
            -X POST "$url" \
            -H "Authorization: Bearer $token" \
            -H "Content-Type: application/json" \
            -d "$body"
    else
        curl -sS -w "\n%{http_code}" \
            -X POST "$url" \
            -H "Content-Type: application/json" \
            -d "$body"
    fi
}

# HTTP helper: GET with auth header
get_json() {
    local url="$1" token="$2"
    curl -sS -w "\n%{http_code}" \
        -X GET "$url" \
        -H "Authorization: Bearer $token"
}

# Extract HTTP status from curl output (last line)
http_status() {
    echo "$1" | tail -n1
}

# Extract body from curl output (all lines except last)
http_body() {
    echo "$1" | sed '$d'
}

# Download via docker network when URL hosts are internal (minio, etc.)
download_via_docker_network() {
    local url="$1" method="${2:-GET}"
    local network
    network=$(docker inspect klass-e2e-python-worker --format '{{range $k, $v := .NetworkSettings.Networks}}{{$k}}{{end}}' 2>/dev/null || true)
    if [[ -z "$network" ]]; then
        network=$(docker network ls --format '{{.Name}}' | grep -E 'gateway.*e2e|e2e' | head -n1 || true)
    fi
    if [[ -z "$network" ]]; then
        echo "000|0|"
        return 1
    fi
    if [[ "$method" == "HEAD" ]]; then
        docker run --rm --network "$network" curlimages/curl:8.5.0 -sS -o /dev/null \
            -w "%{http_code}|0|%{content_type}" -I "$url" 2>/dev/null || echo "000|0|"
    else
        docker run --rm --network "$network" curlimages/curl:8.5.0 -sS -o /dev/null \
            -w "%{http_code}|%{size_download}|%{content_type}" "$url" 2>/dev/null || echo "000|0|"
    fi
}

# ─── Pre-flight ──────────────────────────────────────────────────────────────

header "Pre-flight Checks"

for cmd in curl jq docker; do
    if command -v "$cmd" &>/dev/null; then
        log "$cmd is available"
    else
        fail "$cmd is not installed"
        exit 1
    fi
done

log "Gateway URL: $GATEWAY_URL"
log "API base:    $API_BASE"
log "Python API:  $PYTHON_API_URL"

# ─── Wait for services ──────────────────────────────────────────────────────

header "Waiting for Services"

log "Checking Gateway health..."
for i in $(seq 1 60); do
    STATUS=$(curl -sS -o /dev/null -w "%{http_code}" "$GATEWAY_URL/health" 2>/dev/null || echo "000")
    if [[ "$STATUS" == "200" ]]; then
        ok "Gateway is healthy"
        break
    fi
    if [[ $i -eq 60 ]]; then
        fail "Gateway health check failed after 60 attempts (last status: $STATUS)"
        exit 1
    fi
    sleep 2
done

log "Checking Python API health..."
for i in $(seq 1 30); do
    STATUS=$(curl -sS -o /dev/null -w "%{http_code}" "$PYTHON_API_URL/health" 2>/dev/null || echo "000")
    if [[ "$STATUS" == "200" ]]; then
        ok "Python API is healthy"
        break
    fi
    if [[ $i -eq 30 ]]; then
        fail "Python API health check failed after 30 attempts (last status: $STATUS)"
        exit 1
    fi
    sleep 2
done

# ─── Register a test user ───────────────────────────────────────────────────

header "Setup: Register Test User"

EMAIL="e2e-teacher-$(date +%s)@test.com"
REGISTER_RESP=$(post_json \
    "$API_BASE/auth/register" \
    "" \
    "{\"name\":\"E2E Test Teacher\",\"email\":\"$EMAIL\",\"password\":\"TestPass123!\"}" \
)

REGISTER_BODY=$(http_body "$REGISTER_RESP")
REGISTER_STATUS=$(http_status "$REGISTER_RESP")

if [[ "$REGISTER_STATUS" == "201" ]] || [[ "$REGISTER_STATUS" == "200" ]]; then
    TOKEN=$(echo "$REGISTER_BODY" | jq -r '.data.token // .token // empty')
    USER_ID=$(echo "$REGISTER_BODY" | jq -r '.data.user.id // .user.id // empty')

    if [[ -n "$TOKEN" ]] && [[ -n "$USER_ID" ]]; then
        ok "Test user registered (id=$USER_ID, token length=${#TOKEN})"
    else
        fail "Registration succeeded but token/user_id missing in response"
        echo "$REGISTER_BODY" | jq . 2>/dev/null || echo "$REGISTER_BODY"
        exit 1
    fi
else
    fail "Could not register test user (HTTP $REGISTER_STATUS)"
    echo "$REGISTER_BODY" | jq . 2>/dev/null || echo "$REGISTER_BODY"
    exit 1
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SUB-TASK 3.1.2: Happy Path — Mobile → Rust → Redis → Python Worker →
#                 S3 → Webhook → Rust → Mobile
# ═══════════════════════════════════════════════════════════════════════════════

header "3.1.2 — Happy Path: Create Media Generation"

log "Step 1: POST /api/v1/media-generations (submit generation request)"

CREATE_RESP=$(post_json \
    "$API_BASE/media-generations" \
    "$TOKEN" \
    '{"raw_prompt":"Buatkan handout pecahan kelas 5 SD","preferred_output_type":"handout"}' \
)

CREATE_BODY=$(http_body "$CREATE_RESP")
CREATE_STATUS=$(http_status "$CREATE_RESP")

log "Response status: $CREATE_STATUS"
echo "$CREATE_BODY" | jq . 2>/dev/null || echo "$CREATE_BODY"

if [[ "$CREATE_STATUS" == "202" ]] || [[ "$CREATE_STATUS" == "201" ]]; then
    ok "Media generation accepted (HTTP $CREATE_STATUS)"

    GEN_ID=$(echo "$CREATE_BODY" | jq -r '.data.generation_id // .data.id // empty')
    JOB_ID=$(echo "$CREATE_BODY" | jq -r '.data.job_id // empty')
    JOB_STATUS=$(echo "$CREATE_BODY" | jq -r '.data.status // empty')
    POLL_URL=$(echo "$CREATE_BODY" | jq -r '.data.poll_url // empty')

    if [[ -n "$GEN_ID" ]]; then
        ok "generation_id = $GEN_ID"
    else
        fail "generation_id not found in response"
        exit 1
    fi

    if [[ -n "$JOB_ID" ]] && [[ "$JOB_ID" != "null" ]]; then
        ok "job_id = $JOB_ID"
    else
        fail "job_id not found in response (async tracking not working)"
        exit 1
    fi

    log "Initial status: $JOB_STATUS"
    log "Poll URL: $POLL_URL"
else
    fail "Media generation returned HTTP $CREATE_STATUS (expected 202)"
    exit 1
fi

# ─── Step 2: Poll job status ────────────────────────────────────────────────

header "3.1.2 — Poll Job Status (waiting for async processing)"

POLL_URL_FULL="$API_BASE/media-generations/$GEN_ID/job-status"
log "Polling: $POLL_URL_FULL"

COMPLETED=false
FINAL_STATUS=""
PRESIGNED_URL=""

for attempt in $(seq 1 "$POLL_MAX_ATTEMPTS"); do
    POLL_RESP=$(get_json "$POLL_URL_FULL" "$TOKEN")
    POLL_BODY=$(http_body "$POLL_RESP")
    POLL_STATUS=$(http_status "$POLL_RESP")

    if [[ "$POLL_STATUS" != "200" ]]; then
        fail "Poll returned HTTP $POLL_STATUS on attempt $attempt"
        echo "$POLL_BODY" | jq . 2>/dev/null || echo "$POLL_BODY"
        break
    fi

    CURRENT_STATUS=$(echo "$POLL_BODY" | jq -r '.data.status // .status // "unknown"')
    log "  Attempt $attempt/$POLL_MAX_ATTEMPTS: status=$CURRENT_STATUS"

    case "$CURRENT_STATUS" in
        completed)
            ok "Job COMPLETED after $attempt poll(s)!"
            COMPLETED=true
            FINAL_STATUS="completed"
            PRESIGNED_URL=$(echo "$POLL_BODY" | jq -r '.data.presigned_download_url // .presigned_download_url // empty')
            break
            ;;
        failed)
            ERROR_CODE=$(echo "$POLL_BODY" | jq -r '.data.error_code // .error_code // "unknown"')
            ERROR_MSG=$(echo "$POLL_BODY" | jq -r '.data.error_message // .error_message // "unknown"')
            fail "Job FAILED: code=$ERROR_CODE, message=$ERROR_MSG"
            FINAL_STATUS="failed"
            echo "$POLL_BODY" | jq . 2>/dev/null || echo "$POLL_BODY"
            break
            ;;
        pending|processing)
            sleep "$POLL_INTERVAL"
            ;;
        *)
            # Workflow lifecycle statuses (e.g. interpreting, generating) may appear
            # briefly if generation_status is not yet set — keep polling.
            warn "Non-async status '$CURRENT_STATUS' — continuing to poll"
            sleep "$POLL_INTERVAL"
            ;;
    esac
done

if [[ "$COMPLETED" == "false" ]] && [[ "$FINAL_STATUS" != "failed" ]]; then
    fail "Job did not complete within $POLL_MAX_ATTEMPTS polls (${POLL_MAX_ATTEMPTS}x${POLL_INTERVAL}s = $((POLL_MAX_ATTEMPTS * POLL_INTERVAL))s)"
fi

# ─── Step 3: Verify webhook was received ────────────────────────────────────

header "3.1.2 — Verify Webhook Delivery (job-status / generation_status)"

if [[ "$FINAL_STATUS" == "completed" ]]; then
    # Primary source of truth for async completion is job-status (generation_status).
    # Workflow lifecycle `status` may still be "generating" until publish resumes.
    JOB_CHECK=$(get_json "$POLL_URL_FULL" "$TOKEN")
    JOB_CHECK_BODY=$(http_body "$JOB_CHECK")
    ASYNC_STATUS=$(echo "$JOB_CHECK_BODY" | jq -r '.data.status // empty')
    S3_HINT=$(echo "$JOB_CHECK_BODY" | jq -r '.data.presigned_download_url // empty')

    if [[ "$ASYNC_STATUS" == "completed" ]]; then
        ok "Webhook successfully updated generation_status to 'completed'"
    else
        fail "job-status is '$ASYNC_STATUS' (expected 'completed')"
    fi

    if [[ -n "$S3_HINT" ]] && [[ "$S3_HINT" != "null" ]]; then
        ok "job-status includes presigned_download_url (webhook stored S3 metadata)"
    else
        fail "job-status completed but presigned_download_url is missing"
    fi
else
    fail "Skipping webhook verification — job did not complete successfully"
fi

# ═══════════════════════════════════════════════════════════════════════════════
# SUB-TASK 3.1.3: Verify Presigned URL Download
# ═══════════════════════════════════════════════════════════════════════════════

header "3.1.3 — Presigned URL Download Verification"

if [[ -n "$PRESIGNED_URL" ]] && [[ "$PRESIGNED_URL" != "null" ]] && [[ "$PRESIGNED_URL" != "" ]]; then
    ok "Presigned URL received: ${PRESIGNED_URL:0:100}..."

    # Host machine cannot resolve docker-internal hostnames (minio). Download
    # from a container attached to the E2E network so SigV4 host stays valid.
    USE_DOCKER_DL=false
    if echo "$PRESIGNED_URL" | grep -qE '://minio[:/]|://klass-e2e-minio[:/]'; then
        USE_DOCKER_DL=true
        log "URL targets docker-internal host — verifying via compose network"
    fi

    log "Attempting HEAD request to verify URL is accessible..."
    if [[ "$USE_DOCKER_DL" == "true" ]]; then
        HEAD_RESP=$(download_via_docker_network "$PRESIGNED_URL" HEAD)
        HEAD_CODE=$(echo "$HEAD_RESP" | cut -d'|' -f1)
    else
        HEAD_CODE=$(curl -sS -o /dev/null -w "%{http_code}" -I "$PRESIGNED_URL" 2>/dev/null || echo "000")
    fi

    if [[ "$HEAD_CODE" == "200" ]]; then
        ok "Presigned URL is accessible (HEAD returned 200)"
    elif [[ "$HEAD_CODE" == "403" ]]; then
        warn "Presigned URL HEAD returned 403 (MinIO limitation — HEAD not supported for presigned URLs; GET succeeds)"
    elif [[ "$HEAD_CODE" == "404" ]]; then
        fail "Presigned URL returned 404 Not Found (artifact not uploaded to S3)"
    else
        warn "Presigned URL HEAD returned HTTP $HEAD_CODE — trying GET anyway"
    fi

    log "Attempting GET request to verify artifact is downloadable..."
    if [[ "$USE_DOCKER_DL" == "true" ]]; then
        DOWNLOAD_RESP=$(download_via_docker_network "$PRESIGNED_URL" GET)
    else
        DOWNLOAD_RESP=$(curl -sS -o /dev/null -w "%{http_code}|%{size_download}|%{content_type}" "$PRESIGNED_URL" 2>/dev/null || echo "000||")
    fi

    DL_STATUS=$(echo "$DOWNLOAD_RESP" | cut -d'|' -f1)
    DL_SIZE=$(echo "$DOWNLOAD_RESP" | cut -d'|' -f2)
    DL_CONTENT_TYPE=$(echo "$DOWNLOAD_RESP" | cut -d'|' -f3)

    log "  Status: $DL_STATUS | Size: $DL_SIZE bytes | Content-Type: $DL_CONTENT_TYPE"

    if [[ "$DL_STATUS" == "200" ]]; then
        if [[ -n "$DL_SIZE" ]] && [[ "$DL_SIZE" -gt 0 ]]; then
            ok "Artifact downloadable via presigned URL ($DL_SIZE bytes, $DL_CONTENT_TYPE)"
        else
            fail "Presigned URL returned 200 but body size is 0"
        fi
    elif [[ "$DL_STATUS" == "403" ]]; then
        fail "Artifact download returned 403 Forbidden"
    elif [[ "$DL_STATUS" == "404" ]]; then
        fail "Artifact download returned 404 — file not in S3"
    else
        fail "Artifact download returned HTTP $DL_STATUS"
    fi
else
    fail "No presigned URL available for download verification"

    if [[ -n "${GEN_ID:-}" ]]; then
        log "Fallback: re-fetching job status..."
        FALLBACK_RESP=$(get_json "$API_BASE/media-generations/$GEN_ID/job-status" "$TOKEN")
        FALLBACK_BODY=$(http_body "$FALLBACK_RESP")
        FALLBACK_URL=$(echo "$FALLBACK_BODY" | jq -r '.data.presigned_download_url // empty')
        echo "$FALLBACK_BODY" | jq . 2>/dev/null || echo "$FALLBACK_BODY"

        if [[ -n "$FALLBACK_URL" ]] && [[ "$FALLBACK_URL" != "null" ]]; then
            ok "Found presigned URL from fallback poll: ${FALLBACK_URL:0:80}..."
        else
            fail "No presigned URL found in fallback poll either"
        fi
    fi
fi

# ═══════════════════════════════════════════════════════════════════════════════
# Summary
# ═══════════════════════════════════════════════════════════════════════════════

header "E2E Test Results"

echo ""
echo -e "  ${GREEN}Passed: $PASS_COUNT${NC}"
echo -e "  ${RED}Failed: $FAIL_COUNT${NC}"
echo ""

if [[ $FAIL_COUNT -eq 0 ]]; then
    echo -e "${GREEN}${BOLD}═══════════════════════════════════════════════════${NC}"
    echo -e "${GREEN}${BOLD}  ALL E2E TESTS PASSED ✅${NC}"
    echo -e "${GREEN}${BOLD}  Happy path verified:${NC}"
    echo -e "${GREEN}${BOLD}    Mobile → Rust → Redis → Python Worker → S3 → Webhook → Rust → Mobile${NC}"
    echo -e "${GREEN}${BOLD}  Presigned URL download: VERIFIED${NC}"
    echo -e "${GREEN}${BOLD}═══════════════════════════════════════════════════${NC}"
    exit 0
else
    echo -e "${RED}${BOLD}═══════════════════════════════════════════════════${NC}"
    echo -e "${RED}${BOLD}  E2E TESTS FAILED ❌${NC}"
    echo -e "${RED}${BOLD}  $FAIL_COUNT test(s) did not pass${NC}"
    echo -e "${RED}${BOLD}═══════════════════════════════════════════════════${NC}"
    exit 1
fi
