# GPU Scheduling

## Node Pools

| Pool | Hardware | Use Case |
|------|----------|----------|
| `gpu-h100-pool` | NVIDIA H100 | vLLM/Triton TensorRT-LLM high-performance |
| `gpu-a100-pool` | NVIDIA A100 | vLLM standard LLM |
| `gpu-l4-pool` | NVIDIA L4 | ONNX/llama.cpp lightweight inference |
| `gpu-edge-pool` | GPU modest (RTX A2000) | GGUF/ONNX small models, PoC |
| `cpu-pool` | CPU only | Preprocessing, gateway, auxiliary services |

## VRAM Budget Formula

```
Usable VRAM = Total VRAM × 0.90
Available   = Usable VRAM − Model Size − 1 GB Fixed Overhead − KV Cache
KV Cache    = 2 × Batch × Context × Layers × Heads × Bytes-per-weight / 1024³
```

If `Available < 0`, deployment is **blocked** by `vram-budget-calc`.

## Hardware Constraints

- **FP8 rejected on Ampere** (RTX A2000, A100 lack FP8 Tensor Cores)
- Minimum quantisation enforced per GPU pool

## Tooling

- `tools/vram-budget-calc` — CI gate, validates budget before merge
- DCGM Exporter — GPU metrics (utilisation, memory, temperature, ECC)
- Kueue — quota and queue management
- Karpenter — on-demand node provisioning