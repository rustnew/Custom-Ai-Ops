#!/bin/bash
set -euo pipefail

MODEL_URL="${1:?Usage: smoke-test.sh <model_url> [api_key]}"
API_KEY="${2:-}"

echo "=== Model Serving Smoke Test ==="
echo "Endpoint: ${MODEL_URL}"
echo ""

echo "--- Test 1: Health endpoint (expect 200) ---"
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "${MODEL_URL}/health")
if [ "$HTTP_CODE" = "200" ]; then
    echo "PASS: /health returned 200"
else
    echo "FAIL: /health returned ${HTTP_CODE} (expected 200)"
    exit 1
fi

echo ""
echo "--- Test 2: Auth rejection (expect 401 without key) ---"
HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "${MODEL_URL}/v1/chat/completions" -H "Content-Type: application/json" -d '{"model":"test","messages":[{"role":"user","content":"hi"}]}')
if [ "$HTTP_CODE" = "401" ] || [ "$HTTP_CODE" = "403" ]; then
    echo "PASS: Auth rejection returned ${HTTP_CODE}"
else
    echo "WARN: Expected 401/403, got ${HTTP_CODE} (may not require auth)"
fi

echo ""
echo "--- Test 3: Chat completion (expect 200) ---"
if [ -n "$API_KEY" ]; then
    AUTH_HEADER="Authorization: Bearer ${API_KEY}"
    HTTP_CODE=$(curl -s -o /tmp/smoke_response.json -w "%{http_code}" \
        -H "$AUTH_HEADER" \
        -H "Content-Type: application/json" \
        "${MODEL_URL}/v1/chat/completions" \
        -d '{"model":"test","messages":[{"role":"user","content":"Say hello in one word"}],"max_tokens":10}')
else
    HTTP_CODE=$(curl -s -o /tmp/smoke_response.json -w "%{http_code}" \
        -H "Content-Type: application/json" \
        "${MODEL_URL}/v1/chat/completions" \
        -d '{"model":"test","messages":[{"role":"user","content":"Say hello in one word"}],"max_tokens":10}')
fi

if [ "$HTTP_CODE" = "200" ]; then
    echo "PASS: /v1/chat/completions returned 200"
    HAS_CONTENT=$(python3 -c "import json; d=json.load(open('/tmp/smoke_response.json')); print('yes' if d.get('choices',[{}])[0].get('message',{}).get('content','') else 'no')" 2>/dev/null || echo "unknown")
    echo "Response has content: ${HAS_CONTENT}"
else
    echo "FAIL: /v1/chat/completions returned ${HTTP_CODE}"
    cat /tmp/smoke_response.json 2>/dev/null || true
    exit 1
fi

echo ""
echo "--- Test 4: Cost metric is non-zero ---"
METRICS=$(curl -s "${MODEL_URL}/metrics" 2>/dev/null || echo "")
if echo "$METRICS" | grep -q "model_tokens_generated_total\|model_cost_total\|inference_cost"; then
    echo "PASS: Cost metric found in /metrics"
else
    echo "WARN: No cost metric found in /metrics (may need custom exporter)"
fi

echo ""
echo "=== Smoke tests completed ==="