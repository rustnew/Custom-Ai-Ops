# Capacity Forecasting

## Approach

- **Holt-Winters** seasonal models via Prometheus recording rules for predictable load patterns
- **KEDA** predictive scaling: cron-based pre-warming before known peaks + reactive scaling on real-time QPS
- **Periodic load tests** (`tests/load/`) run in CI to detect capacity drift before it becomes an incident

## Key Metrics for Forecasting

| Metric | Recording Rule | Retention |
|--------|---------------|-----------|
| Request rate (5m avg) | `model:serving:request_rate:5m` | 2 years (Mimir) |
| P95 latency (5m) | `model:serving:latency_p95:5m` | 2 years (Mimir) |
| GPU utilisation | `model:gpu:utilization:5m` | 2 years (Mimir) |
| Active models | `model:serving:active_models` | 2 years (Mimir) |

## Scaling Triggers

- CPU > 80% → HPA scale up
- GPU memory > 90% → alert + investigate model replacement
- P95 latency > 2000ms → gateway failover to SaaS
- Request queue depth > 50 → KEDA scale up