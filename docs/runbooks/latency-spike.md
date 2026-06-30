# Runbook: Latency Spike / Failover Triggered

## Symptoms
- Alert: `ModelServingHighLatency` (p95 > 2s)
- Alert: `ModelServingCriticalLatency` (p99 > 5s)
- Envoy Gateway may have activated SaaS fallback

## Steps

1. **Check current latency**:
   ```bash
   kubectl logs -n envoy-gateway-system deploy/envoy-gateway | grep "failover"
   ```

2. **Identify bottleneck**:
   - GPU utilisation: Grafana DCGM dashboard
   - Request queue depth: Check model-serving dashboard
   - Network: Check for pod network latency in chaos test results

3. **Check if GPU is throttled**:
   ```bash
   nvidia-smi -q -d PERFORMANCE
   kubectl get events -n model-serving-prod --sort-by='.lastTimestamp'
   ```

4. **Immediate actions**:
   - If GPU overloaded: scale up replicas
     ```bash
     kubectl scale statefulset/<name> --replicas=<n+1> -n model-serving-prod
     ```
   - If model is too large: consider lower quantisation
   - If traffic spike is predictable: pre-warm with KEDA cron scaler

5. **Verify recovery**:
   - P95 latency < 2s on Grafana dashboard
   - Fallback route deactivated (priority 0 receiving 100% traffic)

6. **Post-incident**:
   - Add capacity forecast recording rule if traffic pattern is new
   - Update model serving runbook with the specific cause