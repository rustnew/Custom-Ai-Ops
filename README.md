# Ultimate Reference Architecture — Cloud-Scale Multi-Format ML Model Serving Platform

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

## Objective

Define the most robust, modular, and durable project structure possible for deploying ML models (all formats) on the cloud, capable of serving millions of users, with self-healing, load forecasting, and long-term resilience (years, not months). This document synthesizes and extends the existing pattern (StatefulSet bjw-template + ArgoCD + Envoy AI Gateway + sync waves) into a generalized, multi-engine, multi-format system.

---

## 0. Guiding Principle

The system must **never rigidly couple model format to the serving engine**. The correct architecture strictly separates three layers:

1. **Model layer** (the weights + their format) — interchangeable.
2. **Engine layer** (the runtime that executes this format) — interchangeable depending on the format.
3. **Exposure layer** (the OpenAI-compatible API exposed to the gateway) — always identical, regardless of the engine underneath.

This decoupling makes the system modular: you can add a new model without touching the gateway, change an engine without touching the client.

---

## 1. Complete Map of Model Formats and Their Engines

Do not limit yourself to ONNX. Here is the complete mapping to anticipate in the architecture.

| Format | Typical Use Case | Recommended Open-Source Engine | Why This Engine |
|---|---|---|---|
| **GGUF** (quantized Q4/Q5/Q8) | Light LLMs, edge, modest CPU/GPU | **llama.cpp** | Most robust and lightweight for GGUF, no Python dependencies, fast startup, ideal for limited hardware (already LIVE in your pattern) |
| **Safetensors / BF16-FP16** | LLM full precision or semi-precision, large GPU datacenter | **vLLM** | PagedAttention, continuous batching, highest throughput on GPU server (A100/H100) |
| **ONNX (INT4 AWQ, INT8, FP16)** | Converted models, multi-platform portability, native Rust/C++ integration | **ONNX Runtime GenAI** or **Triton Inference Server** (ONNX backend) | ORT GenAI for a custom lightweight server (your Rust FFI pattern); Triton if you want unified multi-model/multi-framework |
| **TensorRT / TensorRT-LLM engines** | Minimal latency on NVIDIA GPU, large-scale production | **Triton Inference Server** (TensorRT-LLM backend) | Specific GPU compilation, kernel fusion, fastest in pure NVIDIA but least portable |
| **Native PyTorch (.pt/.bin)** | Custom models not yet converted, research → prod rapid iteration | **TorchServe** or **Ray Serve** | Quick bridge before conversion to an optimized format |
| **CoreML / TFLite** | Edge mobile, embedded inference outside central cloud | Out of scope for central server — mentioned for completeness | Not relevant for central server but to anticipate if roadmap includes edge device |
| **GGUF MoE / multi-file (sharded)** | Very large models like Mixtral | **llama.cpp** (native support) or **vLLM** (with tensor parallelism) | Depends on size — llama.cpp for single node, vLLM for multi-GPU |
| **AWQ/GPTQ safetensors** | Different quantization than GGUF, GPU server compatible | **vLLM** (native support) | Avoids re-conversion, vLLM reads these formats directly |

### Engine Selection Decision Tree

```
Is the model in GGUF format?
├── Yes → llama.cpp
└── No
    └── Is the model in ONNX format?
        ├── Yes, simple/unique use → ONNX Runtime GenAI (custom lightweight server)
        └── Yes, multi-model/multi-framework use → Triton (ONNX backend)
    └── Is the model in safetensors/BF16/AWQ/GPTQ format?
        └── Yes → vLLM
    └── Does the model have a compiled TensorRT-LLM engine?
        └── Yes → Triton (TensorRT-LLM backend)
    └── Is the model in raw PyTorch unconverted?
        └── Yes → Ray Serve (transient, waiting for conversion)
```

This rule must be **encoded in an internal tool** (see section 4.3) rather than left to ad hoc human decision at each new model.

---

## 2. Repository Structure (Monorepo GitOps, generalized from existing pattern)

```
ai-platform/
├── charts/
│   ├── model-serving-llamacpp/        # Generic GGUF template
│   ├── model-serving-vllm/            # Generic safetensors/AWQ/GPTQ template
│   ├── model-serving-onnx-rust/       # Generic ONNX (custom Rust server) template
│   ├── model-serving-triton/          # Generic Triton (ONNX/TensorRT-LLM/multi) template
│   ├── model-serving-rayserve/        # Transient raw PyTorch template
│   ├── bjw-template/                  # Common base StatefulSet/PVC/Ingress (Helm dependency)
│   ├── ai-gateway/                    # Envoy AI Gateway + backends + models + pricing
│   └── apps/                          # ArgoCD App-of-Apps (ApplicationSet per environment)
├── environments/
│   ├── dev/
│   │   └── values/<app>.yaml          # Per-app, per-env overrides
│   ├── staging/
│   └── prod/
├── models/
│   ├── registry.yaml                  # Declarative registry: name, format, engine, VRAM budget, status
│   └── <model-name>/
│       ├── model.md                   # Model card (individual paper, like docs/models/onnx.md)
│       ├── budget.md                  # Proven VRAM/CPU budget before deployment
│       └── eval-report.md             # Validation quality results before promotion
├── docs/
│   ├── architecture/
│   │   ├── 00-overview.md
│   │   ├── 01-formats-and-engines.md
│   │   ├── 02-gpu-scheduling.md
│   │   ├── 03-gateway-federation.md
│   │   ├── 04-gitops-deployment.md
│   │   ├── 05-observability.md
│   │   ├── 06-resilience-and-dr.md
│   │   └── 07-capacity-forecasting.md
│   ├── adr/                           # Architecture Decision Records (already in place in your pattern)
│   └── runbooks/                      # Step-by-step incident procedures
├── tools/
│   ├── engine-selector/               # CLI applying the decision tree (section 4.3)
│   ├── vram-budget-calc/              # Automatic VRAM budget calculator
│   └── model-onboarding/              # Automatic new model scaffold (generates charts + docs)
├── observability/
│   ├── grafana-dashboards/
│   ├── prometheus-rules/
│   └── alertmanager-routes/
└── tests/
    ├── smoke/                         # Automatic post-deployment tests per model
    ├── load/                          # k6/Locust load test scripts
    └── chaos/                         # GPU chaos engineering scenarios
```

**Why this structure is durable**: each format has its own reusable generic chart (not duplicated per model to infinity), each model has its declarative card in `models/`, and `tools/` codifies operational knowledge in code rather than tribal knowledge in one person's head.

---

## 3. Infrastructure Topology (generalizing the two-cluster pattern)

### 3.1 Control Plane / Worker Plane Separation

Reinforce and generalize the already validated principle:

- **Control cluster**: hosts only ArgoCD and CRDs `Application`/`ApplicationSet`. Never GPU workload here.
- **Worker cluster(s)**: one or more clusters dedicated to actual model execution, potentially split by region or cloud provider.

**Why this is essential at scale**: allows horizontal scaling of worker clusters (multi-cloud, multi-region) without ever touching the GitOps control logic, which remains unique and centralized.

### 3.2 Node Pools by Hardware Type

| Pool | Hardware | Usage |
|---|---|---|
| `gpu-h100-pool` | NVIDIA H100 | High-performance LLM, vLLM/Triton TensorRT-LLM |
| `gpu-a100-pool` | NVIDIA A100 | Standard LLM, vLLM |
| `gpu-l4-pool` | NVIDIA L4 | Light inference, ONNX/llama.cpp, cost-optimized |
| `gpu-edge-pool` | Modest GPU (e.g., A2000, like your home setup) | Small GGUF/ONNX models, PoC |
| `cpu-pool` | CPU only | Preprocessing, gateway, auxiliary services |

Each pool has its own taints/tolerations and `nodeSelector`, guaranteeing that Kueue/Volcano places each workload on the corresponding hardware for its cost/performance ratio.

### 3.3 Recommended GPU Orchestration Tools (open-source, ranked by production durability)

| Tool | Role | Maturity for Long-Term Production |
|---|---|---|
| **NVIDIA GPU Operator** | Driver, device plugin, DCGM exporter, toolkit | Industrial reference, actively maintained by NVIDIA |
| **Kueue** (sigs.k8s.io) | Quotas, queues, priority | Official Kubernetes SIG project, designed to last |
| **Volcano** (CNCF) | Gang scheduling | CNCF incubated, wide batch/ML adoption |
| **Karpenter** | On-demand GPU node provisioning | De facto AWS standard, portable via providers |
| **KEDA** (CNCF) | Event-driven autoscaling, scale-to-zero | CNCF graduated, very stable |

All these tools are CNCF projects or maintained by hardware vendors themselves — this is the durability criterion (no risk of abandonment by a startup).

---

## 4. Multi-Engine Abstraction Layer (the core of modularity)

### 4.1 Unique Interface Contract

Regardless of the engine (llama.cpp, vLLM, ONNX Runtime GenAI, Triton, Ray Serve), each serving service MUST expose:

- `POST /v1/chat/completions` (OpenAI-compatible) — so the gateway never sees any difference
- `GET /health` (503 during loading, 200 when ready)
- Streaming SSE (`text/event-stream`)
- Native API key authentication (`--api-key-file` or equivalent) — avoids a Caddy sidecar when the engine supports it natively

This is exactly the principle already applied in your pattern (llama.cpp and ONNX Rust have native auth; vLLM requires a Caddy sidecar because it doesn't support it natively — to note as a technical debt to monitor if vLLM adds native support one day).

### 4.2 Gateway Federation

Each serving engine, once exposed via Ingress, is federated in the gateway exactly like an external SaaS backend:

```yaml
backends:
  <model>-local-01:
    schema: OpenAI
    prefix: /v1
    fqdn.hostname: <model>--poc.example.com
    securityType: APIKey
    tlsHostname: <model>--poc.example.com
models:
  <model>-local:
    info:
      displayName: "<Model> (self-hosted)"
    contextLength: <N>
    pricing: { strategy: weighted, ... }
    backends:
      - ref: <model>-local-01
        priority: 0
```

**Why this is the most important decision in the system**: from the end client's perspective, a self-hosted GGUF model, a vLLM safetensors model, and an external SaaS model (OpenAI, Anthropic) are strictly identical. This allows migrating a model from one engine to another, or switching to a SaaS provider in case of failure, without any client-side changes — this is the basis of long-term robustness.

### 4.3 Internal Tool `engine-selector`

A small tool (Rust CLI, consistent with your stack) that:

1. Reads the model format (extension, HuggingFace metadata, or explicit config).
2. Applies the decision tree from section 1.
3. Automatically generates the appropriate Helm chart from the corresponding generic template (`charts/model-serving-<engine>`).
4. Calculates and validates the VRAM budget before proposing deployment (see section 4.4).

**Why this tool is indispensable for durability**: eliminates tribal knowledge drift ("we know we need to use this engine for this format") by codifying it. An engineer in 3 years can onboard a model without knowing the decision history.

### 4.4 Systematic VRAM Budget Calculation (before any deployment)

Reinforce and generalize the already applied calculation:

```
Usable Budget = Total_VRAM × util_factor(0.85–0.90)
                − model_size(format, quantization)
                − fixed_overhead(~1 GB)
                = Available budget for KV-cache / activations
```

This calculation must be an automated test (`tools/vram-budget-calc`) executed in CI **before** the manifest is merged — reject deployment if the budget is negative. This avoids OOM in production, which is the most frequent and avoidable incident.

**Hard-coded hardware rule**: never deploy an FP8 checkpoint on a GPU architecture without native FP8 support (e.g., Ampere) — automatic verification to integrate into the tool.

---

## 5. Complete GitOps Pipeline (CI → CD → ArgoCD)

### 5.1 Continuous Delivery Flow

```
Merge to charts repo (main)
    → CI: lint + helm template (blank render) + values format test
    → Publish chart to OCI (chart registry, automatic semver)
    → argocd-image-updater detects new signed image (cosign)
    → Automatic commit of tag to values repo (separate, signed)
    → ArgoCD synchronizes (OCI chart source + separate values source)
```

**Why separate chart repo and values repo**: allows differentiated access control (who can change deployment structure vs who can change which version is in prod) and clearer audit — pattern already validated in your ADR-0055.

### 5.2 Generalized Sync Waves

| Wave | Content | Justification |
|---|---|---|
| -3 | Bootstrap namespace, base secrets | Nothing can start without this |
| -2 | Storage (PVC, databases) | Pods will need volumes ready |
| -1 | Operators and collectors (GPU Operator, Prometheus Operator, log collectors) | Must run before workloads to not miss any metrics at startup |
| 0 | Workloads (the model servers themselves) | The core of the system |
| 1 | Content (Grafana dashboards, gateway config) | Depends on workloads already in place |
| 2+ | Post-sync (automatic smoke tests, notifications) | Final validation |

### 5.3 Custom ArgoCD Health Checks for ML CRDs

Essential for KServe/Triton whose CRDs are not natively understood by ArgoCD:

```yaml
resource.customizations: |
  serving.kserve.io/InferenceService:
    health.lua: |
      hs = {}
      if obj.status and obj.status.conditions then
        for i, condition in ipairs(obj.status.conditions) do
          if condition.type == "Ready" and condition.status == "True" then
            hs.status = "Healthy"
            return hs
          end
        end
      end
      hs.status = "Progressing"
      return hs
```

Without this, ArgoCD will indefinitely show "Progressing" even when the model is actually ready — critical blind spot for visibility.

---

## 6. Observability and Forecasting (the system that "forecasts and repairs")

### 6.1 Observability Stack (open-source, chosen for durability)

| Layer | Tool | Why This Choice |
|---|---|---|
| Metrics | **Prometheus** + **Mimir** (long-term storage) | De facto CNCF standard, Mimir allows multi-year retention without exploding costs |
| Logs | **Loki** | Consistent with Grafana ecosystem (LGTM stack), low storage cost |
| Traces | **Tempo** + **OpenTelemetry** | Distributed tracing standard, essential for multi-modal pipelines |
| Visualization | **Grafana** | Unifies metrics/logs/traces in single dashboard |
| Low-level GPU metrics | **DCGM Exporter** (NVIDIA) | Only official exporter giving real SM/memory/temp per GPU |
| Collection | **Grafana Alloy** (successor to Grafana Agent) | Single agent for metrics/logs/traces, reduces operational complexity |

This is exactly the LGTM stack already present in your architecture (Mimir/Loki/Tempo/Grafana) — to generalize as mandatory base for any new worker cluster.

### 6.2 Load Forecasting (capacity forecasting)

- **Prometheus + simple time series models (Holt-Winters via `prometheus-anomaly-detector` or seasonal recording rules)** to anticipate recurring peaks (office hours, campaign launches).
- **KEDA with predictive scalers**: combine a cron-based scaler (pre-warming before known peak) with a reactive scaler (real QPS) to avoid cold start at critical moment.
- **Regular automated load tests** (k6 or Locust, in `tests/load/`) executed in CI periodically, not just before major deployment — to detect capacity drift before it becomes an incident.

### 6.3 Automatic Repair System (auto-healing in layers)

| Level | Mechanism | Tool |
|---|---|---|
| Pod | Restart on liveness probe failure | Kubernetes native |
| GPU node failure | Xid error detection + automatic cordon/drain | **NVIDIA GPU Operator** (integrated node health check) |
| Configuration drift | Automatic re-sync to Git state | **ArgoCD self-healing** (native) |
| Model quality degradation | Automatic failover to simpler fallback model | Circuit breaker at gateway level (Envoy) |
| Entire cluster failure | Traffic failover to another cluster/region | DNS-based failover or multi-backend gateway with priority |
| Data drift | Alert + trigger re-evaluation pipeline | **Evidently AI** (open-source, self-hosted, no SaaS dependency) |

**Key durability principle**: each repair mechanism must have a **trace in Git** of its action (even automatic), so in 2 years you can understand why a rollback occurred without archaeological log digging.

---

## 7. Multi-Year Robustness: What to Plan from Day One

### 7.1 Dependency Choices for Longevity

Systematically prefer:
- **CNCF graduated** projects (Kubernetes, Prometheus, Envoy, Helm, etc.) over recent ungoverned tools.
- Model formats with **established conversion ecosystems** (GGUF, ONNX, safetensors) over single-provider proprietary formats.
- Engines **actively maintained by multiple independent contributors** (llama.cpp, vLLM) over single-maintainer projects.

### 7.2 Living Documentation as Safeguard

Reinforce and systematize the already-in-place pattern:
- **ADR** (Architecture Decision Records) for each structural decision — why this engine was chosen for this format, why this two-cluster architecture.
- **Per-model card** (`models/<model>/model.md`) documenting proven VRAM budget, status (LIVE/STAGED/STANDBY), and migration history.
- **Incident runbooks** written BEFORE the incident, not after — a system that must last years will have team turnover, and knowledge must be in the repo, not in one person's head.

### 7.3 Structural Non-Regression Tests

- `helm lint --strict` + `helm template --dry-run` in CI on **all** charts at each commit, not just modified ones (detects shared Helm dependency regressions like `bjw-template`).
- Automatic registry coherence test (`models/registry.yaml`): every declared model must have a corresponding chart, a corresponding gateway entry, and a proven VRAM budget — otherwise CI fails.
- **Automated model onboarding checklist** by the `model-onboarding` tool (section 2), which scaffolds all necessary files and prevents forgetting a step (already present as manual in your document — to transform into executable tool).

### 7.4 Multi-Cloud/Anti-Lock-in Strategy

- Keep Kubernetes as the only orchestration dependency (no proprietary cloud services like AWS SageMaker endpoints).
- The OpenAI-compatible gateway pattern allows transparently switching between self-hosted and SaaS external in case of provider failure — already the basis of your architecture, to document explicitly as continuity strategy.
- Store model weights in an S3-compatible object store (MinIO self-hosted or S3/GCS/R2) rather than a non-portable proprietary service.

---

## 8. Complete Technology Stack Summary

| Layer | Tool Chosen | Alternative if Different Constraint |
|---|---|---|
| Orchestration | Kubernetes (Talos for nodes, or k3s for light clusters) | — |
| GitOps | ArgoCD | Flux (if different pull multi-tenant preference) |
| GGUF Engine | llama.cpp | — |
| safetensors/AWQ/GPTQ Engine | vLLM | TGI |
| Simple ONNX Engine | ONNX Runtime GenAI (custom Rust server) | — |
| Advanced Multi-Format Engine | Triton Inference Server | — |
| GPU Scheduling | Kueue + Volcano + NVIDIA GPU Operator | — |
| Autoscaling | KEDA + custom HPA metrics | — |
| Node Provisioning | Karpenter | Cluster Autoscaler |
| API Gateway | Envoy AI Gateway (OpenAI-compatible) | — |
| Observability | Prometheus/Mimir + Loki + Tempo + Grafana + DCGM | — |
| Drift/Quality | Evidently AI (self-hosted) | WhyLabs (if SaaS acceptable) |
| Secrets | External Secrets Operator + AWS Secrets Manager (or Vault) | — |
| Image Registry | Harbor (self-hosted, integrated CVE scan) | — |
| Model Registry | MLflow Model Registry (self-hosted) | — |
| Model Weights Object Store | MinIO (self-hosted, S3 compatible) | S3/GCS/R2 directly |
| Load Tests | k6 or Locust | — |

---

## 9. Final Model Onboarding Checklist (generalized, multi-format)

1. Identify the native model format (GGUF, safetensors, ONNX, TensorRT engine, raw PyTorch).
2. Run `engine-selector` → gets recommended engine and generated chart.
3. Run `vram-budget-calc` → validates VRAM budget is positive on targeted GPU pool; reject if negative or hardware incompatibility (e.g., FP8 on Ampere).
4. Fill model card (`models/<model>/model.md`) with budget, status, context.
5. Generate gateway entry (`backends` + `models` in `charts/ai-gateway/values.yaml`), with adapted pricing and timeout.
6. Open PR on values repo (not chart repo) — triggers standard GitOps flow.
7. Verify in CI: lint, template dry-run, registry coherence.
8. ArgoCD synchronizes according to defined sync waves.
9. Automatic smoke tests post-sync (`tests/smoke/`): auth 401/200, real completion, non-zero cost metric.
10. Progressive promotion: low `priority` in gateway first (canary), progressive load increase, then normal priority once validated on real traffic.
11. Add model to global Grafana dashboard and Prometheus alerting rules.
12. Document in ADR if this model introduces a new pattern (new format, new engine, new hardware constraint).

Once fully toolized (sections 2 and 4.3), this checklist transforms model addition from an artisanal operation into a reproducible, tested, and auditable operation — the necessary condition for a system that remains correct and understandable for years, with changing teams.
