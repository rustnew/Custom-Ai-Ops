# Runbook: Model Serving Pod CrashLoop

## Symptoms
- Alert: `ModelServingPodCrashLooping`
- Pod restarts > 3 times in 1 hour
- `kubectl get pods` shows `CrashLoopBackOff`

## Steps

1. **Get pod logs**:
   ```bash
   kubectl logs <pod-name> -n model-serving-prod --previous
   kubectl logs <pod-name> -n model-serving-prod
   ```

2. **Check common causes**:
   - **OOM**: `kubectl describe pod <pod-name>` → look for `OOMKilled`
     - Fix: Increase memory limit or use quantised model
     - Validate: `vram-budget-calc -V <vram> --model-size <size> --quant <q>`
   - **Model not found**: Verify PVC is mounted and model file exists
     - `kubectl exec <pod-name> -- ls /models/`
   - **Probe failure**: Startup probe timed out
     - Increase `startupProbe.failureThreshold` or `initialDelaySeconds`

3. **Quick recovery**:
   ```bash
   kubectl rollout restart statefulset/<statefulset-name> -n model-serving-prod
   ```

4. **If persistent**:
   - Scale down: `kubectl scale statefulset/<name> --replicas=0`
   - Investigate model integrity
   - Consider switching to a different quantisation or engine

5. **Post-incident**:
   - Update VRAM budget in `models/<model>/budget.md`
   - If a new quantisation was needed, update `models/registry.yaml`