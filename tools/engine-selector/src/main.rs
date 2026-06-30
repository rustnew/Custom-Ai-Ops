use anyhow::{anyhow, Result};
use clap::Parser;
use serde::Serialize;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum ModelFormat {
    Gguf,
    Onnx,
    Safetensors,
    Awq,
    Gptq,
    Tensorrt,
    Pytorch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Engine {
    LlamaCpp,
    Vllm,
    OnnxRuntimeGenai,
    Triton,
    RayServe,
}

#[derive(Debug, Serialize)]
struct EngineSelection {
    format: String,
    engine: String,
    chart: String,
    confidence: f64,
    rationale: String,
}

#[derive(Parser)]
#[command(
    name = "engine-selector",
    version,
    about = "Selects the optimal serving engine for a given ML model format",
    long_about = None
)]
struct Cli {
    #[arg(
        short,
        long,
        help = "Path to the model file or directory",
        value_name = "PATH"
    )]
    model: String,

    #[arg(short, long, help = "Force a specific format", value_name = "FORMAT")]
    format: Option<String>,

    #[arg(
        long,
        help = "Output selection as JSON for pipeline integration",
        default_value_t = false
    )]
    json: bool,
}

fn detect_format(path: &str) -> Result<ModelFormat> {
    let p = Path::new(path);
    if !p.exists() {
        return Err(anyhow!("model path does not exist: {}", path));
    }

    let is_dir = p.is_dir();
    let filename = p
        .file_name()
        .ok_or_else(|| anyhow!("cannot determine filename from path: {}", path))?
        .to_string_lossy()
        .to_lowercase();
    let extension = p
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if extension == "gguf" || filename.contains(".gguf") {
        return Ok(ModelFormat::Gguf);
    }

    if is_dir {
        let has_onnx = walk_extensions(p, &["onnx", "onnx_data"]);
        let has_safetensors = walk_extensions(p, &["safetensors"]);
        let has_awq = walk_extensions(p, &["safetensors"]) && has_awq_config(p);
        let has_gptq = has_gptq_config(p);
        let has_trt = walk_extensions(p, &["plan", "engine"]);
        let has_pt = walk_extensions(p, &["pt", "bin"]);

        if has_trt {
            return Ok(ModelFormat::Tensorrt);
        }
        if has_awq {
            return Ok(ModelFormat::Awq);
        }
        if has_gptq {
            return Ok(ModelFormat::Gptq);
        }
        if has_onnx {
            return Ok(ModelFormat::Onnx);
        }
        if has_safetensors {
            return Ok(ModelFormat::Safetensors);
        }
        if has_pt {
            return Ok(ModelFormat::Pytorch);
        }
        return Err(anyhow!(
            "cannot detect model format in directory: {}",
            path
        ));
    }

    match extension.as_str() {
        "gguf" => Ok(ModelFormat::Gguf),
        "onnx" | "onnx_data" => Ok(ModelFormat::Onnx),
        "safetensors" => Ok(ModelFormat::Safetensors),
        "pt" | "pth" => Ok(ModelFormat::Pytorch),
        "plan" | "engine" => Ok(ModelFormat::Tensorrt),
        "bin" => {
            if filename.contains("awq") {
                Ok(ModelFormat::Awq)
            } else if filename.contains("gptq") {
                Ok(ModelFormat::Gptq)
            } else {
                Err(anyhow!(
                    "ambiguous .bin file — provide --format explicitly"
                ))
            }
        }
        _ => Err(anyhow!(
            "unsupported model extension '{}' — provide --format explicitly",
            extension
        )),
    }
}

fn walk_extensions(dir: &Path, exts: &[&str]) -> bool {
    walkdir_ext(dir, exts, 0, 3)
}

fn walkdir_ext(dir: &Path, exts: &[&str], depth: usize, max_depth: usize) -> bool {
    if depth > max_depth {
        return false;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if walkdir_ext(&path, exts, depth + 1, max_depth) {
                return true;
            }
        } else {
            let e = path
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            if exts.iter().any(|target| *target == e) {
                return true;
            }
        }
    }
    false
}

fn has_awq_config(dir: &Path) -> bool {
    let config_path = dir.join("config.json");
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return false;
    };
    content.contains("\"quant_method\"") && content.contains("\"awq\"")
}

fn has_gptq_config(dir: &Path) -> bool {
    let config_path = dir.join("config.json");
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return false;
    };
    content.contains("\"quant_method\"") && content.contains("\"gptq\"")
}

fn parse_format_override(s: &str) -> Result<ModelFormat> {
    match s.to_lowercase().as_str() {
        "gguf" => Ok(ModelFormat::Gguf),
        "onnx" => Ok(ModelFormat::Onnx),
        "safetensors" => Ok(ModelFormat::Safetensors),
        "awq" => Ok(ModelFormat::Awq),
        "gptq" => Ok(ModelFormat::Gptq),
        "tensorrt" | "trt" => Ok(ModelFormat::Tensorrt),
        "pytorch" | "pt" => Ok(ModelFormat::Pytorch),
        _ => Err(anyhow!("unknown format override: {}", s)),
    }
}

fn select_engine(fmt: ModelFormat) -> (Engine, f64, String, String) {
    match fmt {
        ModelFormat::Gguf => (
            Engine::LlamaCpp,
            0.97,
            "model-serving-llamacpp".to_string(),
            "llama.cpp is the most robust and lightweight engine for GGUF format, with no Python dependency".to_string(),
        ),
        ModelFormat::Onnx => (
            Engine::OnnxRuntimeGenai,
            0.95,
            "model-serving-onnx-rust".to_string(),
            "ONNX Runtime GenAI provides native ONNX execution with Rust FFI integration".to_string(),
        ),
        ModelFormat::Safetensors => (
            Engine::Vllm,
            0.96,
            "model-serving-vllm".to_string(),
            "vLLM offers PagedAttention and continuous batching for maximum throughput on safetensors".to_string(),
        ),
        ModelFormat::Awq => (
            Engine::Vllm,
            0.94,
            "model-serving-vllm".to_string(),
            "vLLM has native AWQ support, avoiding re-conversion from quantised format".to_string(),
        ),
        ModelFormat::Gptq => (
            Engine::Vllm,
            0.93,
            "model-serving-vllm".to_string(),
            "vLLM supports GPTQ natively without format conversion".to_string(),
        ),
        ModelFormat::Tensorrt => (
            Engine::Triton,
            0.98,
            "model-serving-triton".to_string(),
            "Triton Inference Server with TensorRT-LLM backend provides minimum latency on NVIDIA GPUs".to_string(),
        ),
        ModelFormat::Pytorch => (
            Engine::RayServe,
            0.70,
            "model-serving-rayserve".to_string(),
            "Ray Serve serves native PyTorch models transitively; convert to optimised format for production".to_string(),
        ),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let fmt = match cli.format {
        Some(ref f) => parse_format_override(f)?,
        None => detect_format(&cli.model)?,
    };

    let (engine, confidence, chart, rationale) = select_engine(fmt);

    let selection = EngineSelection {
        format: format!("{:?}", fmt).to_lowercase(),
        engine: match engine {
            Engine::LlamaCpp => "llama.cpp".to_string(),
            Engine::OnnxRuntimeGenai => "onnx-runtime-genai".to_string(),
            Engine::Vllm => "vllm".to_string(),
            Engine::Triton => "triton".to_string(),
            Engine::RayServe => "ray-serve".to_string(),
        },
        chart,
        confidence,
        rationale,
    };

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&selection)?);
    } else {
        println!("Model format   : {}", selection.format.to_uppercase());
        println!("Serving engine : {}", selection.engine);
        println!("Helm chart     : {}", selection.chart);
        println!("Confidence     : {:.0}%", selection.confidence * 100.0);
        println!("Rationale      : {}", selection.rationale);
    }

    Ok(())
}