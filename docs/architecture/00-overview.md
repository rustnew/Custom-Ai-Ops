# Architecture Overview

This platform serves ML models of multiple formats (GGUF, Safetensors, ONNX, TensorRT, PyTorch) through a unified OpenAI-compatible API, using the optimal inference engine for each format.

## Three-Plane Architecture

1. **Model Plane** — Model weights + format (GGUF, Safetensors, ONNX, AWQ, GPTQ, TensorRT, PyTorch)
2. **Engine Plane** — Runtime that executes a given format (llama.cpp, vLLM, ONNX Runtime GenAI, Triton, Ray Serve)
3. **Exposure Plane** — OpenAI-compatible endpoint via Envoy AI Gateway (uniform API regardless of engine)

## Key Decisions

- See [ADR index](../adr/) for architectural decision records
- See [01-formats-and-engines.md](01-formats-and-engines.md) for format-to-engine mapping
- See [04-gitops-deployment.md](04-gitops-deployment.md) for deployment chain