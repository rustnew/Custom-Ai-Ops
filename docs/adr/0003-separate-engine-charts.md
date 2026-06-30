# ADR-0003: Separate engine-specific Helm charts

## Status: Accepted

## Context

A single monolithic chart with conditionals for each engine becomes unmaintainable as the number of engines grows.

## Decision

Create separate Helm charts per engine type:
- `model-serving-llamacpp` — GGUF models
- `model-serving-vllm` — Safetensors/AWQ/GPTQ models
- `model-serving-onnx-rust` — ONNX models
- `model-serving-triton` — TensorRT/multi-framework models
- `model-serving-rayserve` — PyTorch transitional models

All charts depend on `bjw-template` library chart for common StatefulSet/PVC/probe patterns.

## Consequences

- Each chart focuses on a single engine's configuration.
- shared patterns are maintained once in `bjw-template`.
- The unified `model-serving-engine` chart remains for quick prototyping with `engine.type` switching.