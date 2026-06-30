# VRAM Budget Calculation: llama-3-8b-instruct

## Inputs
- GPU: NVIDIA RTX A2000 (8 GB VRAM)
- Model: llama-3-8b-instruct (Q4_K_M)
- Quantisation: q4_km (~0.55 bytes/weight)
- Batch size: 1
- Context length: 4096
- Layers: 32
- Attention heads: 32

## Calculation

```
Usable VRAM = 8.0 * 0.90 = 7.20 GB
Model size  = ~4.70 GB
Fixed OH   = 1.00 GB
KV cache   = 2 * 1 * 4096 * 32 * 32 * 0.55 / (1024^3) = ~1.23 GB
Remaining  = 7.20 - 4.70 - 1.00 - 1.23 = 0.27 GB
```

## Result

**FITS**: 0.27 GB remaining after all allocations.

## Hardware Constraints
- FP8 rejected: RTX A2000 is Ampere architecture (no FP8 Tensor Cores)
- Minimum quantisation: Q4_K_M (lower quantisation causes OOM)

## Validated
```
$ vram-budget-calc -V 8 --model-size 4.7 --quant q4_km --gpu "RTX A2000" --batch 1 --context 4096 --layers 32 --heads 32 --json
```

Result: `"fits": true, "remaining_gb": 0.27`