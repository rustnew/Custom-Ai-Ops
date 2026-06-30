# Observability

## Stack (LGTM)

| Layer | Tool | Purpose |
|-------|------|---------|
| Metrics | Prometheus + Mimir | Long-term metric storage |
| Logs | Loki | Log aggregation |
| Traces | Tempo + OpenTelemetry | Distributed tracing |
| Dashboards | Grafana | Unified visualisation |
| GPU Metrics | DCGM Exporter | GPU utilisation, memory, temperature, ECC |

## Dashboards

- `observability/grafana-dashboards/dcgm-dashboard.json` — GPU health
- `observability/grafana-dashboards/model-serving-dashboard.json` — Request latency, error rate, throughput

## Alerting Rules

- `observability/prometheus-anomaly-rules.yaml` — Latency, errors, GPU, pod, and anomaly rules
- `observability/alertmanager-routes/config.yaml` — Route critical → PagerDuty, warning → Slack

## Anomaly Detection

- Derivative-based latency anomaly (p95 increasing >0.1 s/s over 30m)
- Throughput drop anomaly (deriv < -0.5 req/s/s over 30m)