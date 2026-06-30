# ADR-0001: Multi-format model serving architecture

## Status: Accepted

## Context

The platform must serve models in multiple formats (GGUF, Safetensors, ONNX, AWQ, GPTQ, TensorRT, PyTorch) without coupling format to engine. New models and engines should be addable without changing the exposure layer.

## Decision

Adopt a three-plane architecture:
1. **Model Plane** — interchangeable model weights and formats
2. **Engine Plane** — swappable runtime per format (llama.cpp, vLLM, ONNX Runtime GenAI, Triton, Ray Serve)
3. **Exposure Plane** — uniform OpenAI-compatible API via Envoy AI Gateway

Each format gets its own Helm chart derived from `bjw-template` library. The engine-selector CLI codifies the decision tree.

## Consequences

- Adding a new model requires only: `model-onboarding`, `engine-selector`, `vram-budget-calc`, then a PR.
- Changing engines for a format only requires changing the Decision Tree in `engine-selector`, not the gateway or client.
- SaaS failover is transparent to the client.