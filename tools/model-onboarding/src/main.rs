use anyhow::{anyhow, Result};
use clap::Parser;
use serde::Serialize;
use std::fs;

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
#[allow(dead_code)]
enum Engine {
    LlamaCpp,
    Vllm,
    OnnxRuntimeGenai,
    Triton,
    RayServe,
}

#[derive(Debug, Serialize)]
struct RegistryEntry {
    name: String,
    format: String,
    engine: String,
    chart: String,
    status: String,
    vram_budget_gb: f64,
    gpu_pool: String,
    context_length: u32,
    quantisation: String,
    gateway_backend: String,
    notes: String,
}

#[derive(Parser)]
#[command(
    name = "model-onboarding",
    version,
    about = "Scaffolds all files needed to onboard a new ML model into the platform",
    long_about = None
)]
struct Cli {
    #[arg(short, long, help = "Model name (kebab-case, e.g. llama-3-8b-instruct)")]
    name: String,

    #[arg(short, long, help = "Model format (gguf, onnx, safetensors, awq, gptq, tensorrt, pytorch)")]
    format: String,

    #[arg(short, long, help = "Total GPU VRAM in GB", value_name = "GB")]
    vram: f64,

    #[arg(short, long, help = "Model size in GB", value_name = "GB")]
    model_size: f64,

    #[arg(short, long, help = "GPU pool target (e.g. gpu-a100-pool)")]
    gpu_pool: String,

    #[arg(long, help = "Context length", default_value_t = 4096)]
    context_length: u32,

    #[arg(long, help = "Quantisation format")]
    quantisation: Option<String>,

    #[arg(short, long, help = "GPU name for VRAM validation (e.g. 'RTX A2000')")]
    gpu: Option<String>,

    #[arg(long, help = "Custom notes")]
    notes: Option<String>,

    #[arg(
        long,
        help = "Dry run: show what would be created without writing files",
        default_value_t = false
    )]
    dry_run: bool,
}

fn format_to_chart(fmt: ModelFormat) -> &'static str {
    match fmt {
        ModelFormat::Gguf => "model-serving-llamacpp",
        ModelFormat::Onnx => "model-serving-onnx-rust",
        ModelFormat::Safetensors | ModelFormat::Awq | ModelFormat::Gptq => "model-serving-vllm",
        ModelFormat::Tensorrt => "model-serving-triton",
        ModelFormat::Pytorch => "model-serving-rayserve",
    }
}

fn format_to_engine(fmt: ModelFormat) -> &'static str {
    match fmt {
        ModelFormat::Gguf => "llamacpp",
        ModelFormat::Onnx => "onnxGenai",
        ModelFormat::Safetensors | ModelFormat::Awq | ModelFormat::Gptq => "vllm",
        ModelFormat::Tensorrt => "triton",
        ModelFormat::Pytorch => "rayserve",
    }
}

fn parse_format(s: &str) -> Result<ModelFormat> {
    match s.to_lowercase().as_str() {
        "gguf" => Ok(ModelFormat::Gguf),
        "onnx" => Ok(ModelFormat::Onnx),
        "safetensors" => Ok(ModelFormat::Safetensors),
        "awq" => Ok(ModelFormat::Awq),
        "gptq" => Ok(ModelFormat::Gptq),
        "tensorrt" | "trt" => Ok(ModelFormat::Tensorrt),
        "pytorch" | "pt" => Ok(ModelFormat::Pytorch),
        _ => Err(anyhow!("unknown format: {}", s)),
    }
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let fmt = parse_format(&cli.format)?;
    let chart = format_to_chart(fmt);
    let engine = format_to_engine(fmt);
    let gateway_backend = format!("{}-local", cli.name);

    let usable_vram = cli.vram * 0.90;
    let fixed_overhead = 1.0;
    let remaining = usable_vram - cli.model_size - fixed_overhead;

    if remaining < 0.0 {
        eprintln!(
            "ERROR: Insufficient VRAM. Usable {:.2} GB - model {:.2} GB - overhead {:.2} GB = {:.2} GB",
            usable_vram, cli.model_size, fixed_overhead, remaining
        );
        std::process::exit(1);
    }

    let notes = cli.notes.unwrap_or_else(|| format!(
        "Model onboarded via model-onboarding tool, {} format on {} pool",
        cli.format, cli.gpu_pool
    ));

    let registry_entry = RegistryEntry {
        name: cli.name.clone(),
        format: cli.format.clone(),
        engine: engine.to_string(),
        chart: chart.to_string(),
        status: "STAGED".to_string(),
        vram_budget_gb: cli.vram,
        gpu_pool: cli.gpu_pool.clone(),
        context_length: cli.context_length,
        quantisation: cli.quantisation.clone().unwrap_or_else(|| "unknown".to_string()),
        gateway_backend: gateway_backend.clone(),
        notes,
    };

    let model_dir = format!("models/{}", cli.name);

    if cli.dry_run {
        println!("DRY RUN — would create:");
        println!("  Directory: {}/", model_dir);
        println!("  File: {}/model.md", model_dir);
        println!("  File: {}/budget.md", model_dir);
        println!("  File: {}/eval-report.md", model_dir);
        println!("  Append to: models/registry.yaml");
        println!();
        println!("Registry entry:");
        println!("{}", serde_json::to_string_pretty(&registry_entry)?);
        println!();
        println!("Helm chart to use: charts/{}/", chart);
        println!("Gateway backend: {}", gateway_backend);
        return Ok(());
    }

    fs::create_dir_all(&model_dir)?;

    let model_md = format!(
        "# Model: {name}\n\n\
         ## Metadata\n\
         - **Format**: {fmt}\n\
         - **Engine**: {engine}\n\
         - **Status**: STAGED\n\
         - **GPU Pool**: {gpu_pool}\n\
         - **Context Length**: {ctx}\n\
         - **Quantisation**: {quant}\n\n\
         ## VRAM Budget\n\n\
         | Component | Size |\n\
         |-----------|------|\n\
         | Model weights | {model_size:.2} GB |\n\
         | Fixed overhead | 1.00 GB |\n\
         | Usable VRAM (90%) | {usable:.2} GB |\n\
         | **Remaining** | **{remaining:.2} GB** |\n\n\
         ## Gateway Configuration\n\
         - Backend: `{backend}`\n\
         - Priority: 0 (primary)\n\n\
         ## Deployment\n\
         - Chart: `{chart}`\n\
         - Environment: `environments/prod/`\n\
         - Sync wave: 0 (workload)\n\n\
         ## History\n\
         - {date}: Initial onboarding via model-onboarding tool\n",
        name = cli.name,
        fmt = cli.format,
        engine = engine,
        gpu_pool = cli.gpu_pool,
        ctx = cli.context_length,
        quant = cli.quantisation.as_deref().unwrap_or("unknown"),
        model_size = cli.model_size,
        usable = usable_vram,
        remaining = remaining,
        backend = gateway_backend,
        chart = chart,
        date = chrono_like_date(),
    );
    fs::write(format!("{}/model.md", model_dir), &model_md)?;

    let budget_md = format!(
        "# VRAM Budget Calculation: {name}\n\n\
         ## Inputs\n\
         - GPU: {gpu}\n\
         - Total VRAM: {vram:.1} GB\n\
         - Model size: {model_size:.2} GB\n\
         - Quantisation: {quant}\n\
         - Context length: {ctx}\n\n\
         ## Calculation\n\n\
         ```\n\
         Usable VRAM = {vram:.1} * 0.90 = {usable:.2} GB\n\
         Model size  = {model_size:.2} GB\n\
         Fixed OH   = 1.00 GB\n\
         Remaining  = {usable:.2} - {model_size:.2} - 1.00 = {remaining:.2} GB\n\
         ```\n\n\
         ## Result\n\n\
         **{status}**: {remaining:.2} GB remaining after all allocations.\n",
        name = cli.name,
        gpu = cli.gpu.as_deref().unwrap_or("unspecified"),
        vram = cli.vram,
        model_size = cli.model_size,
        quant = cli.quantisation.as_deref().unwrap_or("unknown"),
        ctx = cli.context_length,
        usable = usable_vram,
        remaining = remaining,
        status = if remaining >= 0.0 { "FITS" } else { "OOM RISK" },
    );
    fs::write(format!("{}/budget.md", model_dir), &budget_md)?;

    let eval_md = format!(
        "# Evaluation Report: {name}\n\n\
         ## Status: PENDING\n\n\
         | Benchmark | Score | Baseline | Pass? |\n\
         |-----------|-------|----------|-------|\n\
         | MMLU (5-shot) | - | 0.650 | PENDING |\n\
         | HellaSwag | - | 0.750 | PENDING |\n\
         | ARC-Challenge | - | 0.580 | PENDING |\n\n\
         ## Latency (PENDING)\n\n\
         | Metric | Value | Threshold | Pass? |\n\
         |--------|-------|-----------|-------|\n\
         | TTFT (p50) | - | <500ms | PENDING |\n\
         | TTFT (p95) | - | <1000ms | PENDING |\n\
         | TPS | - | >20 | PENDING |\n\
         | E2E latency (p95) | - | <2000ms | PENDING |\n\n\
         ## Recommendation\n\
         PENDING — awaiting evaluation results.\n",
        name = cli.name,
    );
    fs::write(format!("{}/eval-report.md", model_dir), &eval_md)?;

    println!("Created model directory: {}/", model_dir);
    println!("  - {}/model.md", model_dir);
    println!("  - {}/budget.md", model_dir);
    println!("  - {}/eval-report.md", model_dir);
    println!();
    println!("To complete onboarding:");
    println!("  1. Run: vram-budget-calc -V {} --model-size {} --quant {} --gpu \"{}\"", cli.vram, cli.model_size, cli.quantisation.as_deref().unwrap_or("fp16"), cli.gpu.as_deref().unwrap_or("unspecified"));
    println!("  2. Add entry to models/registry.yaml:");
    println!("{}", serde_json::to_string_pretty(&registry_entry)?);
    println!("  3. Add gateway backend in charts/ai-gateway/values.yaml");
    println!("  4. Deploy with chart: charts/{}/", chart);
    println!("  5. Run smoke tests: tests/smoke/");
    println!("  6. Fill evaluation report: {}/eval-report.md", model_dir);
    println!("  7. Promote status from STAGED to LIVE in registry.yaml");

    Ok(())
}

fn chrono_like_date() -> String {
    let output = std::process::Command::new("date")
        .arg("+%Y-%m-%d")
        .output();
    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        Err(_) => "YYYY-MM-DD".to_string(),
    }
}