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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Engine {
    LlamaCpp,
    OnnxRuntimeGenai,
    Vllm,
}

#[derive(Debug, Serialize)]
struct EngineSelection {
    format: String,
    engine: String,
    confidence: f64,
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

        if has_awq {
            return Ok(ModelFormat::Awq);
        }
        if has_onnx {
            return Ok(ModelFormat::Onnx);
        }
        if has_safetensors {
            return Ok(ModelFormat::Safetensors);
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
        "bin" => {
            if filename.contains("awq") {
                Ok(ModelFormat::Awq)
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

fn parse_format_override(s: &str) -> Result<ModelFormat> {
    match s.to_lowercase().as_str() {
        "gguf" => Ok(ModelFormat::Gguf),
        "onnx" => Ok(ModelFormat::Onnx),
        "safetensors" => Ok(ModelFormat::Safetensors),
        "awq" => Ok(ModelFormat::Awq),
        _ => Err(anyhow!("unknown format override: {}", s)),
    }
}

fn select_engine(fmt: ModelFormat) -> (Engine, f64) {
    match fmt {
        ModelFormat::Gguf => (Engine::LlamaCpp, 0.97),
        ModelFormat::Onnx => (Engine::OnnxRuntimeGenai, 0.95),
        ModelFormat::Safetensors => (Engine::Vllm, 0.96),
        ModelFormat::Awq => (Engine::Vllm, 0.94),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let fmt = match cli.format {
        Some(ref f) => parse_format_override(f)?,
        None => detect_format(&cli.model)?,
    };

    let (engine, confidence) = select_engine(fmt);

    let selection = EngineSelection {
        format: format!("{:?}", fmt).to_lowercase(),
        engine: match engine {
            Engine::LlamaCpp => "llama.cpp".to_string(),
            Engine::OnnxRuntimeGenai => "onnx-runtime-genai".to_string(),
            Engine::Vllm => "vllm".to_string(),
        },
        confidence,
    };

    if cli.json {
        println!("{}", serde_json::to_string_pretty(&selection)?);
    } else {
        println!(
            "Model format : {}",
            selection.format.to_uppercase()
        );
        println!(
            "Serving engine: {}",
            selection.engine
        );
        println!(
            "Confidence    : {:.0}%",
            selection.confidence * 100.0
        );
    }

    Ok(())
}