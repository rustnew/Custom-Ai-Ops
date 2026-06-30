# GitOps Deployment

## Sync Waves

| Wave | Content | Justification |
|------|---------|---------------|
| -3 | Bootstrap namespace, secrets | Nothing starts without these |
| -2 | Storage (PVC, Longhorn, versioned seed jobs) | Pods need volumes ready |
| -1 | Operators (NVIDIA GPU, Prometheus) | Must run before workloads to capture metrics |
| 0 | Workloads (StatefulSets) | Core model serving |
| 1 | Content (Grafana dashboards, gateway config) | Depends on workloads |
| 2+ | Post-sync (smoke tests, notifications) | Validation final |

## ArgoCD ApplicationSet

Production deployment uses `apps/argocd-appset-prod.yaml` with:
- Automated prune and self-heal
- Server-side apply
- Retry with exponential backoff

## Health Checks for ML CRDs

ArgoCD does not natively understand KServe/Triton CRDs. Custom Lua health checks are configured to avoid perpetual "Progressing" state.