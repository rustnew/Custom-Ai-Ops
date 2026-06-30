# Model: llama-3-8b-instruct

## Metadata
- **Format**: GGUF (quantised Q4_K_M)
- **Engine**: llama.cpp
- **Status**: LIVE
- **GPU Pool**: gpu-edge-pool (NVIDIA RTX A2000)

## VRAM Budget

| Component              | Size     |
|------------------------|----------|
| Model weights (Q4_K_M) | 4.7 GB   |
| KV cache (bs=1, ctx=4096) | ~1.2 GB |
| Fixed overhead         | 1.0 GB   |
| Usable VRAM (90%)      | 7.2 GB   |
| **Remaining**          | **0.3 GB** |

Validated by `vram-budget-calc -V 8 --model-size 4.7 --quant q4_km --gpu "RTX A2000" --batch 1 --context 4096 --layers 32 --heads 32`

## Gateway Configuration
- Backend: `llama3-8b-local`
- Priority: 0 (primary)
- Fallback: `openai-gpt4o-mini` (priority 1)

## Deployment
- Chart: `model-serving-llamacpp`
- Environment: `environments/prod/`
- Sync wave: 0 (workload)

## History
- 2026-06-15: Initial deployment, Q4_K_M quantisation, LIVE on edge cluster