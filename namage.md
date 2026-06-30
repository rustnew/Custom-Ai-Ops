# Deployment, Orchestration, and ML Model Production Management

## License

This project is licensed under the MIT License — see the [LICENSE](LICENSE) file for details.

## Document Objective

Master the complete lifecycle of a model in production: from packaging to serving millions of clients, passing through Kubernetes orchestration, fine-grained GPU management, autoscaling, and model-specific problems (LLM, vision, audio, multimodal, recommendation).

---

## 1. Fundamental Problems with Models in Production

### 1.1 Common Problems for All Models

- **Cold start**: loading weights (often several GB to hundreds of GB) takes time, delaying availability of freshly started pods.
- **Data drift**: input distribution in production gradually diverges from training distribution, degrading prediction quality without explicit error.
- **Concept drift**: the input-output relationship changes over time (user behavior, seasonality).
- **p99 vs p50 latency**: the average masks worst cases; a model can have excellent p50 and catastrophic p99 due to GC, batching, or GPU contention.
- **Non-determinism**: non-deterministic CUDA kernels, different floating-point reduction order by batch, making debugging difficult.
- **Memory leaks**: GPU memory fragmentation (especially with variable batch sizes), KV cache leaks for LLMs.
- **Inconsistent versioning**: divergence between trained model, converted model (ONNX, TensorRT), and actually served model.
- **Training-serving skew**: different feature preprocessing pipeline between offline and online.
- **Silent failures**: model responds but with degraded values (NaN, constant outputs) without crash, so without automatic alert.
- **Over-provisioning or under-sizing**: either GPU cost waste or starvation at traffic peak.

### 1.2 Model-Specific Problems

#### LLM (Generative, Transformer Decoder Type)

- **KV-cache management**: linear memory growth with sequence length and batch, #1 OOM GPU source.
- **Continuous batching** complex to implement correctly (vLLM, TGI, TensorRT-LLM): without this, throughput drops drastically.
- **Time-to-first-token (TTFT)** vs **time-per-output-token (TPOT)**: two different metrics to monitor separately.
- **Quantization** (INT8, INT4, FP8): quality/speed/memory tradeoff to validate by model family, not just architecture.
- **Variable context window**: requires efficient padding/masking, otherwise compute waste.
- **Token streaming**: connection management, timeouts, and error recovery for long connections.
- **Security**: prompt injection, data leakage via shared cache between requests.

#### Vision (CNN, ViT, Detection, Segmentation)

- **Variable image sizes**: requires consistent resize/padding in preprocessing, otherwise shape errors.
- **Pre/post-processing often more costly than inference itself** (NMS for object detection).
- **Batching difficult with varying resolutions** (video especially).
- **GPU underutilized if I/O pipeline (image/video decoding) is bottleneck** rather than compute.

#### Audio / Speech (ASR, TTS)

- **Real-time streaming processing** with strict latency constraint (< 300ms perceived as acceptable).
- **Variable audio sequence lengths** complicating batching.
- **Autoregressive TTS models slow token by token**, similar to LLM cache problems.

#### Recommendation / Ranking

- **Very high request throughput (QPS)** but often small models → bottleneck becomes network and feature store, not GPU.
- **Feature freshness critical** (recommendation based on events second-by-second).
- **Strong feature consistency required** between online and offline (shared feature store).

#### Multimodal Models

- **Heterogeneous pipelines** (image encoder + text encoder + decoder) with different GPU needs per component: difficult to place efficiently on single node type.
- **Synchronization between components** if deployed as separate microservices, adding internal network latency.

---

## 2. Kubernetes Orchestration Architecture

### 2.1 Basic Principles

The model must never be deployed as a simple standard `Deployment` without adaptation. Key elements:

- **Readiness probe** distinct from **liveness probe**: readiness must verify weights loaded and inference test succeeds, not just process running.
- **Precise resource requests/limits** on `nvidia.com/gpu`, CPU, and memory — never fractional GPU limit without MIG or time-slicing configured.
- **PodDisruptionBudget** to prevent rolling update or node drain from killing all model pods simultaneously.
- **Init containers** dedicated to downloading weights from object store (S3/GCS) to local fast volume (local NVMe rather than network), decoupling image pull from model loading.

### 2.2 Fine-Grained GPU Management in Kubernetes

- **NVIDIA device plugin**: exposes GPUs as schedulable resource (`nvidia.com/gpu: 1`), but without sharing by default.
- **MIG (Multi-Instance GPU)** on A100/H100 type GPUs: partitions physical GPU into multiple isolated instances (memory and compute), useful for small/medium models not requiring full GPU.
- **Time-slicing**: temporal GPU sharing between multiple pods without strict memory isolation — suitable for tolerance contention workloads (dev/test, light models).
- **NVIDIA GPU Operator**: automates driver, device plugin, DCGM exporter for metrics, and container toolkit deployment.
- **NUMA topology and GPU-CPU affinity**: for very large multi-GPU models, NUMA affinity and NVLink/PCIe bandwidth must be considered via kubelet `topologyManager`.
- **Dedicated node pools** by GPU type (A100, H100, L4, T4) with `nodeSelector`/`taints-tolerations`, routing each model family to cost/performance adapted hardware.
- **Bin packing vs spreading**: to maximize GPU utilization, prefer bin packing (group workloads on few nodes) rather than Kubernetes default spread, via custom schedulers (Volcano, Kueue) better suited for ML batch than default scheduler.

### 2.3 Specialized Scheduling for AI

Default Kubernetes scheduler is not designed for ML batch and gang-scheduling. Solutions to consider:

- **Kueue**: ML job quotas and queue management, priority, and preemption.
- **Volcano**: gang scheduling (all distributed job pods start together or none), essential for distributed training and some synchronized multi-GPU inference.
- **Karpenter / Cluster Autoscaler** configured specifically to provision on-demand GPU nodes, with consolidation to reduce costs during off-hours.

### 2.4 Autoscaling

- **HPA (Horizontal Pod Autoscaler)** based on custom metrics (QPS, p99 latency, queue length) rather than CPU/memory alone — CPU is rarely the limiting factor for GPU inference.
- **KEDA**: event-driven scaling, useful to scale to zero an unused model and restart on demand (at cost of absorbing cold start).
- **VPA (Vertical Pod Autoscaler)**: less relevant for GPU (coarse granularity), more useful for adjusting CPU/memory of auxiliary containers (preprocessing, gateway).
- **Predictive scaling**: for known traffic patterns (office hours, regional peaks), proactive scaling based on history reduces cold start latency compared to purely reactive scaling.
- **Scale-to-zero**: relevant for low-traffic models, but incompatible with strict latency requirement without warm pool mechanism.

### 2.5 Service Mesh and Routing

- **Canary / Blue-Green deployment** for new model versions: route small percentage of traffic to new version and compare business metrics before full switch.
- **Shadow traffic (mirroring)**: duplicate real traffic to new version without impacting user response, validating under real conditions without risk.
- **ML-specific serving tools**: KServe, Seldon Core, or Ray Serve, adding on top of Kubernetes model versioning, automatic batching, native canary, and ML-adapted autoscaling — preferable to manual reimplementation.

---

## 3. Observability and Automatic Management in Production

### 3.1 Mandatory Metrics to Instrument

| Category | Key Metrics |
|---|---|
| Latency | p50, p90, p99, TTFT and TPOT for LLMs |
| Throughput | requests/s, tokens/s |
| Resources | GPU utilization (%), GPU used/total memory, temperature, power draw |
| Quality | error rate, NaN/degenerate output rate, mean confidence score |
| Business | conversion rate, user satisfaction, abandonment rate |
| Cost | cost per request, cost per token, cost per GPU-hour |

- **DCGM Exporter** (NVIDIA) coupled with Prometheus for low-level GPU metrics.
- **Distributed tracing** (OpenTelemetry) to track request through preprocessing → inference → postprocessing, essential for multimodal pipelines.
- **Dynamic threshold alerting** rather than static, adapting to traffic seasonality.

### 3.2 Automatic Drift and Degradation Detection

- **Continuous drift monitoring pipeline**, comparing production features/outputs distribution to reference window (statistical tests like KS-test, PSI).
- **Automatic retraining triggered** by drift threshold or metric degradation, with automatic validation before promotion.
- **Applicational circuit breaker**: if error rate or latency exceeds threshold, automatically failover to simpler/lighter fallback model rather than letting entire service fail.

### 3.3 Automatic Incident Management

- **Automatic rollback** triggered by post-deployment metrics (not just healthcheck failure), integrated into CI/CD pipeline.
- **Auto-healing**: automatic pod restart on repeated GPU OOM detection, with exponential backoff to avoid crash loops.
- **Targeted GPU chaos engineering** (simulating GPU node failure, degraded NVLink) to validate resilience before real incident.

---

## 4. Serving Millions of Clients: Large-Scale Considerations

- **Multi-region**: replicate models in multiple regions to reduce network latency and ensure continuity on regional failure, with strict version synchronization.
- **Response edge caching** for repeated requests (especially recommendation and certain LLM with frequent prompts) to reduce GPU load.
- **Strict ingestion/inference/response decoupling** via queue architecture (Kafka, NATS) to absorb peaks without losing requests.
- **Traffic prioritization (QoS)**: distinguish critical requests (guaranteed SLA) from best-effort, with separate capacity pools.
- **Capacity planning based on regular load tests** simulating predictable peak, not just average traffic.
- **Cost at scale**: at millions of requests, 10% optimization on cost per request (quantization, better batching) represents substantial savings; cost monitoring must be treated as production metric like latency.

---

## 5. Operational Synthesis

To master model production behavior end-to-end, treat three independent but coordinated layers:

1. **Model layer**: strict versioning, automated quality validation before promotion, continuous drift detection, graceful degradation fallback.
2. **Infrastructure layer (Kubernetes + GPU)**: AI-adapted scheduling (Kueue/Volcano), GPU partitioning (MIG/time-slicing), metric-based autoscaling, dedicated node pools by hardware family.
3. **Observability and automation layer**: end-to-end metrics (technical + business + cost), dynamic alerting, automatic rollback and auto-healing, regular load tests.

Best practice: never treat model deployment as classic application deployment: GPU memory constraints, latency variability by sequence length/image, and continuous quality validation need (beyond simple "service responds") impose specific ML serving tools and design (KServe, Ray Serve, vLLM/TGI for LLM) rather than ad hoc reimplementation on top of standard Kubernetes Deployment.
