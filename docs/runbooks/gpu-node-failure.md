# Runbook: GPU Node Failure

## Symptoms
- Alert: `GPUEccErrors` or `GPUThermalThrottle`
- Pods on the node show OOM or CrashLoopBackOff
- DCGM metrics show high temperature or memory exhaustion

## Steps

1. **Identify the node**:
   ```bash
   kubectl get nodes -l nvidia.com/gpu.present=true
   kubectl describe node <node-name> | grep -A5 "Conditions"
   ```

2. **Cordon the node** (prevent new pods):
   ```bash
   kubectl cordon <node-name>
   ```

3. **Drain pods safely**:
   ```bash
   kubectl drain <node-name> --ignore-daemonsets --delete-emptydir-data --timeout=300s
   ```

4. **Verify failover**:
   - Check that Envoy Gateway routes traffic to healthy backends
   - `kubectl logs -n envoy-gateway-system deploy/envoy-gateway`

5. **Investigate root cause**:
   - Check ECC errors: `nvidia-smi -q -d ECC`
   - Check Xid errors: `dmesg | grep -i xid`
   - Check temperatures: `nvidia-smi -q -d TEMPERATURE`

6. **Recovery**:
   - If hardware: replace node, update Kubernetes node labels
   - If transient: `kubectl uncordon <node-name>` after cooling
   - Update `models/registry.yaml` if model moved to different pool

7. **Post-incident**:
   - Document in `docs/adr/` if new pattern discovered
   - Update runbook if steps were incomplete