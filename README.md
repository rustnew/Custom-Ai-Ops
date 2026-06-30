# ML Model Serving Platform

A highly resilient, long-term, multi-format ML Model Serving Platform with triple-layer separation.

## Architecture

### Triple-Layer Separation

1. **Model Layer**: Interchangeable weights (GGUF, Safetensors, ONNX INT4 AWQ)
2. **Engine Layer**: Runtime containers (vLLM, llama.cpp, Custom Rust ONNX GenAI)
3. **Exposure Layer**: OpenAI-compatible endpoint (FQDN via Hetzner Envoy AI Gateway)

## Repository Structure

```
onnx-ai-helm/
├── tools/                    # Rust CLI tools
│   ├── engine-selector/      # Engine selection decision tree
│   └── vram-budget-calc/     # VRAM footprint calculator
├── charts/                   # Helm charts
│   ├── bjw-template/         # Common base template
│   ├── model-serving-llamacpp/
│   ├── model-serving-vllm/
│   ├── model-serving-onnx-rust/
│   ├── model-serving-triton/
│   └── model-serving-rayserve/
├── environments/             # Environment-specific configurations
│   ├── dev/
│   ├── staging/
│   └── prod/
├── apps/                     # GitOps pipeline configurations
│   ├── argocd-app-prod.yaml
│   ├── argocd-app-dev.yaml
│   ├── argocd-appset-prod.yaml
│   ├── argocd-appset-staging.yaml
│   └── argocd-appset-dev.yaml
├── observability/            # Monitoring and alerting
│   ├── envoy-gateway-config.yaml
│   ├── prometheus-anomaly-rules.yaml
│   └── grafana-dashboards.yaml
├── models/                   # Model registry and examples
│   ├── registry/
│   └── example-model/
├── .github/                  # GitHub Actions workflows
│   ├── workflows/
│   │   ├── ci-cd.yaml
│   │   ├── smoke-tests.yaml
│   │   ├── load-tests.yaml
│   │   └── chaos-tests.yaml
├── LICENSE
├── LICENSE.txt
└── README.md
```

## Quick Start

### 1. Build Rust CLI Tools

```bash
cd tools/engine-selector
cargo build --release

cd ../vram-budget-calc
cargo build --release
```

### 2. Validate Helm Charts

```bash
helm lint charts/bjw-template
helm lint charts/model-serving-llamacpp
helm lint charts/model-serving-vllm
```

### 3. Validate Kustomize Configs

```bash
kustomize build environments/dev
kustomize build environments/staging
kustomize build environments/prod
```

### 4. Deploy to Kubernetes

```bash
# Development
kubectl apply -k environments/dev

# Staging
kubectl apply -k environments/staging

# Production
kubectl apply -k environments/prod
```

## Engine Selection

Use the `engine-selector` tool to determine the optimal engine for your model:

```bash
# GGUF format -> llama.cpp
./target/release/engine-selector --model-format GGUF

# Safetensors/AWQ/GPTQ -> vLLM
./target/release/engine-selector --model-format safetensors --quantization AWQ

# ONNX format -> ONNX Runtime GenAI
./target/release/engine-selector --model-format ONNX --multi-model
```

## VRAM Budget Calculation

Use the `vram-budget-calc` tool to validate VRAM requirements:

```bash
# Calculate VRAM budget
./target/release/vram-budget-calc \
  --config model-config.json \
  --total-vram-gb 24 \
  --gpu-architecture Ampere \
  --batch-size 4

# Block deployment if VRAM budget is negative
```

## Sync Waves

The GitOps pipeline manages deployments in waves:

- **Wave -3**: Bootstrap & Secrets
- **Wave -2**: Storage (RWX PVC via Longhorn) and Versioned Seed Jobs
- **Wave -1**: NVIDIA GPU Operator & Prometheus Collectors
- **Wave 0**: Model Server StatefulSet workload

## Observability

### Health Checking

- Active health-checking at the Envoy Gateway level
- Immediate failover to commercial SaaS fallback if latency > 2000ms
- Priority routing 0 to 1

### Monitoring

- Prometheus anomaly detection rules
- DCGM exporter dashboards for GPU health tracking
- Grafana dashboards for performance visualization

## CI/CD Pipeline

### Automated Testing

- Engine selector testing
- VRAM budget calculator testing
- Helm chart validation
- Kustomize validation
- ArgoCD application validation
- Prometheus rules validation
- Grafana dashboard validation

### Load Testing

- Basic throughput tests
- Sustained load tests
- High latency requests
- Batch requests
- Error handling under load

### Chaos Engineering

- High latency simulation
- Network partition simulation
- Resource exhaustion simulation
- Failure recovery simulation

## License

MIT License
