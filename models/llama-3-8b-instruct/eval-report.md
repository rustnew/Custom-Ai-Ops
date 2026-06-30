# Evaluation Report: llama-3-8b-instruct

## Model Info
- **Name**: llama-3-8b-instruct
- **Format**: GGUF Q4_K_M
- **Engine**: llama.cpp

## Quality Benchmarks

| Benchmark           | Score  | Baseline | Pass? |
|---------------------|--------|----------|-------|
| MMLU (5-shot)       | 0.682  | 0.650    | YES   |
| HellaSwag           | 0.781  | 0.750    | YES   |
| ARC-Challenge       | 0.614  | 0.580    | YES   |
| TruthfulQA          | 0.523  | 0.500    | YES   |

## Latency Benchmarks (RTX A2000)

| Metric              | Value  | Threshold | Pass? |
|---------------------|--------|-----------|-------|
| TTFT (p50)          | 180ms  | <500ms    | YES   |
| TTFT (p95)          | 350ms  | <1000ms   | YES   |
| TPS (tokens/sec)    | 28     | >20       | YES   |
| E2E latency (p95)   | 1.8s   | <2000ms   | YES   |

## Safety
- Content filtering: Passed (standard refusal tests)
- Prompt injection: Passed (basic injection mitigated)

## Recommendation
**APPROVED for production promotion** - all benchmarks and latency thresholds met.

## Signed off
- Date: 2026-06-20
- Reviewer: automated-ci