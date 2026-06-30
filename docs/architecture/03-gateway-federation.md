# Gateway Federation

## Architecture

All model-serving backends expose a uniform OpenAI-compatible API through Envoy AI Gateway. From the client perspective, self-hosted models and SaaS providers (OpenAI, Anthropic) are interchangeable.

## Priority Routing

- **Priority 0**: Self-hosted model (primary)
- **Priority 1**: SaaS fallback (activated when latency exceeds 2000ms or error rate > 5%)

## Health Checks

- **Active**: HTTP GET `/health` every 10s, timeout 2s, 3 failures → unhealthy
- **Passive**: Track response times; >2000ms triggers passive failover

## Configuration

All backends and models are defined declaratively in `charts/ai-gateway/values.yaml`, enabling zero-code model additions and failover configuration changes.