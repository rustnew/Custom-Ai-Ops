use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// Engine Selector CLI - Selects the optimal serving engine based on model format
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Model format (GGUF, safetensors, ONNX, TensorRT, PyTorch)
    #[arg(short, long)]
    model_format: Option<String>,

    /// Quantization type (AWQ, GPTQ, INT4, INT8, FP8)
    #[arg(short, long)]
    quantization: Option<String>,

    /// Multi-model deployment flag
    #[arg(short, long)]
    multi_model: bool,

    /// Output directory for generated Helm values
    #[arg(short, long)]
    output_dir: Option<PathBuf>,

    /// Model metadata file path
    #[arg(short, long)]
    metadata_file: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelConfig {
    format: String,
    quantization: Option<String>,
    layers: Option<u32>,
    hidden_size: Option<u32>,
    vocab_size: Option<u32>,
    head_dim: Option<u32>,
    rope_scaling: Option<String>,
    attention_types: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct EngineConfig {
    name: String,
    chart_path: String,
    container_args: Vec<String>,
    sidecars: Vec<String>,
    probes: ProbeConfig,
    gpu_requirements: GpuRequirements,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProbeConfig {
    liveness: Probe,
    startup: Probe,
    readiness: Probe,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Probe {
    initial_delay_seconds: u64,
    period_seconds: u64,
    timeout_seconds: u64,
    failure_threshold: u64,
    success_threshold: u64,
    http_get_path: Option<String>,
    tcp_socket_port: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GpuRequirements {
    nvidia_compu: u32,
    memory_gb: f64,
    architecture: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HelmValues {
    engine_type: String,
    replicas: u32,
    resources: ResourceRequirements,
    image: ImageConfig,
    probes: ProbeConfig,
    auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResourceRequirements {
    requests: ResourceRequests,
    limits: ResourceLimits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResourceRequests {
    cpu: String,
    memory: String,
    nvidia_compu: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResourceLimits {
    cpu: String,
    memory: String,
    nvidia_compu: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ImageConfig {
    repository: String,
    tag: String,
    pull_policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AuthConfig {
    api_key_enabled: bool,
    api_key_file: Option<String>,
    sidecar_auth: bool,
}

fn detect_model_format(metadata: &Option<ModelConfig>) -> String {
    match metadata {
        Some(cfg) => cfg.format.clone(),
        None => "unknown".to_string(),
    }
}

fn calculate_vram_budget(
    total_vram_gb: f64,
    model_size_gb: f64,
    batch_size: u32,
    context_length: u32,
    layers: u32,
    hidden_size: u32,
    head_dim: u32,
    quantization_bits: Option<u8>,
) -> Result<f64> {
    let quantization_factor = quantization_bits.map_or(1.0, |b| {
        match b {
            4 => 0.25,
            8 => 0.5,
            16 => 1.0,
            _ => 1.0,
        }
    });

    let model_size_adjusted = model_size_gb * quantization_factor;
    let fixed_overhead = 1.0; // 1GB fixed overhead

    let batch_factor = batch_size as f64;
    let context_factor = context_length as f64;
    let layers_factor = layers as f64;
    let hidden_factor = hidden_size as f64;
    let head_factor = head_dim as f64;

    let kv_cache_overhead = 2.0 * batch_factor * context_factor * layers_factor * hidden_factor * head_factor * 0.001; // GB

    let usable_vram = total_vram_gb * 0.90 - model_size_adjusted - fixed_overhead - kv_cache_overhead;

    if usable_vram < 0.0 {
        anyhow::bail!(
            "VRAM budget negative: {} GB total - {} GB model - {} GB overhead - {} GB KV-cache = {} GB available",
            total_vram_gb,
            model_size_adjusted,
            fixed_overhead,
            kv_cache_overhead,
            usable_vram
        );
    }

    Ok(usable_vram)
}

fn select_engine(
    model_format: &str,
    quantization: &Option<String>,
    multi_model: bool,
) -> Result<EngineConfig> {
    let format_upper = model_format.to_uppercase();

    // GGUF -> llama.cpp
    if format_upper.contains("GGUF") {
        let args = vec![
            "--gguf".to_string(),
            format!("--context-length 8192".to_string()),
            format!("--batch-size 4".to_string()),
            format!("--n-gpu-layers -1".to_string()),
        ];

        let gpu_req = GpuRequirements {
            nvidia_compu: 1,
            memory_gb: 16.0,
            architecture: "any".to_string(),
        };

        Ok(EngineConfig {
            name: "llama.cpp".to_string(),
            chart_path: "charts/model-serving-llamacpp".to_string(),
            container_args: args,
            sidecars: vec![],
            probes: ProbeConfig {
                liveness: Probe {
                    initial_delay_seconds: 60,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 120,
                    success_threshold: 1,
                    http_get_path: None,
                    tcp_socket_port: Some(8080),
                },
                startup: Probe {
                    initial_delay_seconds: 120,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 120,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
                readiness: Probe {
                    initial_delay_seconds: 120,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 30,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
            },
            gpu_requirements: gpu_req,
        })
    }
    // ONNX simple -> ONNX Runtime GenAI
    else if format_upper.contains("ONNX") && !multi_model {
        let args = vec![
            format!("--model-path /models/onnx/model.onnx".to_string()),
            format!("--session-config 'optimal'".to_string()),
        ];

        let gpu_req = GpuRequirements {
            nvidia_compu: 1,
            memory_gb: 8.0,
            architecture: "Ampere".to_string(),
        };

        Ok(EngineConfig {
            name: "onnx-runtime-genai".to_string(),
            chart_path: "charts/model-serving-onnx-rust".to_string(),
            container_args: args,
            sidecars: vec![],
            probes: ProbeConfig {
                liveness: Probe {
                    initial_delay_seconds: 30,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 60,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
                startup: Probe {
                    initial_delay_seconds: 60,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 120,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
                readiness: Probe {
                    initial_delay_seconds: 60,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 30,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
            },
            gpu_requirements: gpu_req,
        })
    }
    // ONNX multi-model -> Triton
    else if format_upper.contains("ONNX") && multi_model {
        let args = vec![
            format!("--triton-server".to_string()),
            format!("--max-batch-size 16".to_string()),
        ];

        let gpu_req = GpuRequirements {
            nvidia_compu: 2,
            memory_gb: 32.0,
            architecture: "Ampere".to_string(),
        };

        Ok(EngineConfig {
            name: "triton-onnx".to_string(),
            chart_path: "charts/model-serving-triton".to_string(),
            container_args: args,
            sidecars: vec!["caddy-auth".to_string()],
            probes: ProbeConfig {
                liveness: Probe {
                    initial_delay_seconds: 60,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 120,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
                startup: Probe {
                    initial_delay_seconds: 120,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 120,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
                readiness: Probe {
                    initial_delay_seconds: 120,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 30,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
            },
            gpu_requirements: gpu_req,
        })
    }
    // Safetensors/AWQ/GPTQ -> vLLM
    else if quantization
        .as_ref()
        .map_or(false, |q| q.to_uppercase().contains("AWQ") || q.to_uppercase().contains("GPTQ"))
        || quantization.as_ref().map_or(false, |q| q.to_uppercase().contains("SAFETENSORS"))
    {
        let args = vec![
            format!("--model /models/safetensors/model.safetensors".to_string()),
            format!("--max-model-len 8192".to_string()),
            format!("--gpu-memory-utilization 0.90".to_string()),
            format!("--trust-remote-code false".to_string()),
        ];

        let gpu_req = GpuRequirements {
            nvidia_compu: 1,
            memory_gb: 24.0,
            architecture: "Ampere".to_string(),
        };

        Ok(EngineConfig {
            name: "vllm".to_string(),
            chart_path: "charts/model-serving-vllm".to_string(),
            container_args: args,
            sidecars: vec![],
            probes: ProbeConfig {
                liveness: Probe {
                    initial_delay_seconds: 60,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 120,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
                startup: Probe {
                    initial_delay_seconds: 180,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 180,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
                readiness: Probe {
                    initial_delay_seconds: 180,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 30,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
            },
            gpu_requirements: gpu_req,
        })
    }
    // TensorRT-LLM -> Triton
    else if format_upper.contains("TENSORRT") {
        let args = vec![
            format!("--triton-server".to_string()),
            format!("--tensorrt-llm".to_string()),
            format!("--max-batch-size 8".to_string()),
        ];

        let gpu_req = GpuRequirements {
            nvidia_compu: 1,
            memory_gb: 40.0,
            architecture: "Hopper".to_string(),
        };

        Ok(EngineConfig {
            name: "triton-tensorrt-llm".to_string(),
            chart_path: "charts/model-serving-triton".to_string(),
            container_args: args,
            sidecars: vec!["caddy-auth".to_string()],
            probes: ProbeConfig {
                liveness: Probe {
                    initial_delay_seconds: 60,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 120,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
                startup: Probe {
                    initial_delay_seconds: 120,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 120,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
                readiness: Probe {
                    initial_delay_seconds: 120,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 30,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
            },
            gpu_requirements: gpu_req,
        })
    }
    // Raw PyTorch -> Ray Serve
    else if format_upper.contains("PYTORCH") || format_upper.contains(".PT") {
        let args = vec![
            format!("--ray-serve".to_string()),
            format!("--max-concurrent-requests 10".to_string()),
        ];

        let gpu_req = GpuRequirements {
            nvidia_compu: 1,
            memory_gb: 16.0,
            architecture: "Ampere".to_string(),
        };

        Ok(EngineConfig {
            name: "ray-serve".to_string(),
            chart_path: "charts/model-serving-rayserve".to_string(),
            container_args: args,
            sidecars: vec!["caddy-auth".to_string()],
            probes: ProbeConfig {
                liveness: Probe {
                    initial_delay_seconds: 60,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 120,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
                startup: Probe {
                    initial_delay_seconds: 120,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 120,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
                readiness: Probe {
                    initial_delay_seconds: 120,
                    period_seconds: 10,
                    timeout_seconds: 5,
                    failure_threshold: 30,
                    success_threshold: 1,
                    http_get_path: Some("/health".to_string()),
                    tcp_socket_port: None,
                },
            },
            gpu_requirements: gpu_req,
        })
    }
    else {
        anyhow::bail!("Unsupported model format: {}", model_format);
    }
}

fn generate_helm_values(
    engine: &EngineConfig,
    model_name: &str,
    output_dir: &PathBuf,
) -> Result<()> {
    let values = HelmValues {
        engine_type: engine.name.clone(),
        replicas: 2,
        resources: ResourceRequirements {
            requests: ResourceRequests {
                cpu: "4000m".to_string(),
                memory: "16Gi".to_string(),
                nvidia_compu: "1".to_string(),
            },
            limits: ResourceLimits {
                cpu: "8000m".to_string(),
                memory: "32Gi".to_string(),
                nvidia_compu: "1".to_string(),
            },
        },
        image: ImageConfig {
            repository: format!("onnx-ai-helm/{}", engine.name),
            tag: "latest".to_string(),
            pull_policy: "IfNotPresent".to_string(),
        },
        probes: engine.probes.clone(),
        auth: AuthConfig {
            api_key_enabled: true,
            api_key_file: Some("/etc/caddy/api_key".to_string()),
            sidecar_auth: true,
        },
    };

    let values_yaml = serde_yaml::to_string_pretty(&values)?;

    let output_path = output_dir.join(format!("{}.yaml", model_name));
    fs::write(&output_path, values_yaml)?;

    println!("Generated Helm values: {}", output_path.display());
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let model_format = cli.model_format.ok_or_else(|| anyhow::anyhow!("--model-format is required"))?;
    let quantization = cli.quantization;
    let multi_model = cli.multi_model;
    let output_dir = cli.output_dir.unwrap_or_else(|| PathBuf::from("./charts"));

    // Load model metadata if provided
    let metadata = if let Some(metadata_file) = cli.metadata_file {
        if metadata_file.exists() {
            let content = fs::read_to_string(&metadata_file)?;
            serde_json::from_str(&content)
                .context("Failed to parse model metadata JSON")?
        } else {
            None
        }
    } else {
        None
    };

    let model_format = detect_model_format(&metadata);

    println!("=== Engine Selector ===");
    println!("Model format: {}", model_format);
    println!("Quantization: {:?}", quantization);
    println!("Multi-model: {}", multi_model);

    let engine = select_engine(&model_format, &quantization, multi_model)?;

    println!("\n=== Selected Engine ===");
    println!("Engine: {}", engine.name);
    println!("Chart: {}", engine.chart_path);
    println!("GPU Requirements: {} GPU, {} GB memory", engine.gpu_requirements.nvidia_compu, engine.gpu_requirements.memory_gb);

    if cli.output_dir.is_some() {
        generate_helm_values(&engine, "example-model", &output_dir)?;
    }

    Ok(())
}
