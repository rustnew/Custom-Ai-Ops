# Formats and Engines

## Decision Tree

```
Is the model in GGUF?
├── Yes → llama.cpp
└── No
    ├── Is the model in ONNX?
    │   ├── Yes, single model → ONNX Runtime GenAI
    │   └── Yes, multi-model → Triton (ONNX backend)
    ├── Is the model in Safetensors/BF16/FP16?
    │   └── Yes → vLLM
    ├── Is the model in AWQ/GPTQ?
    │   └── Yes → vLLM (native support)
    ├── Does the model have a TensorRT engine?
    │   └── Yes → Triton (TensorRT-LLM backend)
    └── Is the model in raw PyTorch?
        └── Yes → Ray Serve (transitional, convert to optimised format)
```

## Format-Engine Mapping

| Format | Engine | Chart | Confidence |
|--------|--------|-------|------------|
| GGUF | llama.cpp | model-serving-llamacpp | 97% |
| Safetensors (BF16/FP16) | vLLM | model-serving-vllm | 96% |
| AWQ | vLLM | model-serving-vllm | 94% |
| GPTQ | vLLM | model-serving-vllm | 93% |
| ONNX | ONNX Runtime GenAI | model-serving-onnx-rust | 95% |
| TensorRT | Triton Inference Server | model-serving-triton | 98% |
| PyTorch (.pt/.bin) | Ray Serve | model-serving-rayserve | 70% (transitional) |

This decision tree is codified in `tools/engine-selector` to prevent knowledge drift.