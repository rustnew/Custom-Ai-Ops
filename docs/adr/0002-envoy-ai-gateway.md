# ADR-0002: Envoy AI Gateway for model federation

## Status: Accepted

## Context

Models may need to fail over to SaaS providers (OpenAI, Anthropic) during outages or capacity spikes. The failover must be transparent to clients.

## Decision

Use Envoy AI Gateway (Envoy Gateway extension) with priority routing:
- Priority 0: self-hosted model (primary)
- Priority 1: SaaS fallback (activated when latency > 2000ms or error rate > 5%)

Health checks use active HTTP probes with passive latency monitoring.

## Consequences

- Client code never changes during failover.
- Adding a new SaaS provider only requires a gateway values change.
- Latency threshold is configurable per-model in values.yaml.