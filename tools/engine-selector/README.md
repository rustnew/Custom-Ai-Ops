# Engine Selector CLI

A Rust CLI tool that implements the engine selection decision tree for ML model serving.

## Decision Tree Logic

```
Is the model in GGUF format?
├── Yes → llama.cpp
└── No
    └── Is the model in ONNX format?
        ├── Yes, simple use → ONNX Runtime GenAI
        └── Yes, multi-model use → Triton
    └── Is the model in safetensors/AWQ/GPTQ format?
        └── Yes → vLLM
    └── Does the model have TensorRT-LLM engine?
        └── Yes → Triton (TensorRT-LLM backend)
    └── Is the model in raw PyTorch?
        └── Yes → Ray Serve
```

## Usage

```bash
# Select engine for a model
./target/release/engine-selector --model-format GGUF
./target/release/engine-selector --model-format safetensors --quantization AWQ
./target/release/engine-selector --model-format ONNX --multi-model

# Generate Helm chart
./target/release/engine-selector --model-format GGUF --output-dir ./charts/model-serving-llamacpp/values.yaml
```

## Exit Codes

- 0: Success
- 1: Invalid model format
- 2: Error reading model metadata
- 3: Output directory error
