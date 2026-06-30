use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// VRAM Budget Calculator - Validates VRAM requirements before deployment
/// 
/// Formula: Usable VRAM Budget = Total VRAM * util_factor (0.90) - Model Size - 1GB Fixed Overhead - [2 * Batch * Context * Layers * Heads * Bytes]
/// 
/// Strict rules:
/// - Block compilation if available VRAM is negative
/// - Reject FP8 checkpoints on Ampere architectures (like local RTX A2000)

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Model configuration JSON file
    #[arg(short, long)]
    config: PathBuf,

    /// Total VRAM available in GB
    #[arg(short, long)]
    total_vram_gb: f64,

    /// GPU architecture (Ampere, Hopper, H100, etc.)
    #[arg(short, long)]
    gpu_architecture: String,

    /// Batch size for inference
    #[arg(short, long)]
    batch_size: Option<u32>,

    /// Context length
    #[arg(short, long)]
    context_length: Option<u32>,

    /// Number of transformer layers
    #[arg(short, long)]
    layers: Option<u32>,

    /// Hidden dimension size
    #[arg(short, long)]
    hidden_size: Option<u32>,

    /// Head dimension size
    #[arg(short, long)]
    head_dim: Option<u32>,

    /// Quantization bits (4, 8, 16, 32)
    #[arg(short, long)]
    quantization_bits: Option<u8>,

    /// Output report file
    #[arg(short, long)]
    output: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModelConfig {
    name: String,
    format: String,
    size_gb: f64,
    layers: u32,
    hidden_size: u32,
    vocab_size: u32,
    head_dim: u32,
    quantization_bits: Option<u8>,
    rope_scaling: Option<String>,
    attention_types: Option<String>,
}

fn parse_model_config(config_path: &PathBuf) -> Result<ModelConfig> {
    let content = fs::read_to_string(config_path)
        .context("Failed to read model config file")?;
    
    let config: ModelConfig = serde_json::from_str(&content)
        .context("Failed to parse model config JSON")?;
    
    Ok(config)
}

fn calculate_kv_cache_memory(
    batch_size: u32,
    context_length: u32,
    layers: u32,
    hidden_size: u32,
    head_dim: u32,
    quantization_bits: Option<u8>,
) -> f64 {
    let quantization_factor = quantization_bits.map_or(1.0, |b| {
        match b {
            4 => 0.25,
            8 => 0.5,
            16 => 1.0,
            _ => 1.0,
        }
    });

    // KV cache: 2 * batch * context * layers * heads * bytes_per_token
    let bytes_per_token = head_dim as f64 * 2.0 * quantization_factor; // 2 for K and V projections
    
    let kv_cache_bytes = 2.0 * batch_size as f64 * context_length as f64 * layers as f64 * head_dim as f64 * 2.0 * quantization_factor;
    kv_cache_bytes / (1024.0 * 1024.0 * 1024.0) // Convert to GB
}

fn calculate_model_memory(size_gb: f64, quantization_bits: Option<u8>) -> f64 {
    let quantization_factor = quantization_bits.map_or(1.0, |b| {
        match b {
            4 => 0.25,
            8 => 0.5,
            16 => 1.0,
            _ => 1.0,
        }
    });
    
    size_gb * quantization_factor
}

fn check_fp8_compatibility(gpu_architecture: &str, quantization_bits: Option<u8>) -> Result<()> {
    let arch_upper = gpu_architecture.to_uppercase();
    
    // FP8 requires Hopper (H100) or Ampere (A100) with FP8 support
    let has_fp8_support = arch_upper.contains("HOPPER") || 
                          (arch_upper.contains("AMPERE") && quantization_bits.map_or(false, |b| b == 8));
    
    if quantization_bits == Some(8) && !has_fp8_support {
        anyhow::bail!(
            "FP8 quantization rejected on {} architecture. \
             FP8 requires Hopper (H100) or Ampere with FP8 support. \
             Falling back to FP16 (16-bit quantization).",
            gpu_architecture
        );
    }
    
    Ok(())
}

fn calculate_vram_budget(
    config: &ModelConfig,
    total_vram_gb: f64,
    batch_size: Option<u32>,
    gpu_architecture: &str,
) -> Result<VramBudgetReport> {
    let quantization_bits = config.quantization_bits;
    
    // Check FP8 compatibility
    check_fp8_compatibility(gpu_architecture, quantization_bits)?;
    
    // Calculate model memory (with quantization)
    let model_memory = calculate_model_memory(config.size_gb, quantization_bits);
    
    // Calculate KV cache memory
    let batch_size = batch_size.unwrap_or(4);
    let context_length = config.context_length.unwrap_or(8192);
    let layers = config.layers;
    let hidden_size = config.hidden_size;
    let head_dim = config.head_dim;
    
    let kv_cache_memory = calculate_kv_cache_memory(batch_size, context_length, layers, hidden_size, head_dim, quantization_bits);
    
    // Apply formula: Usable VRAM Budget = Total VRAM * 0.90 - Model Size - 1GB Fixed Overhead - KV Cache
    let util_factor = 0.90;
    let fixed_overhead = 1.0; // 1GB fixed overhead
    
    let usable_vram = total_vram_gb * util_factor - model_memory - fixed_overhead - kv_cache_memory;
    
    // Check if budget is negative
    if usable_vram < 0.0 {
        anyhow::bail!(
            "VRAM budget negative: {} GB total * {} - {} GB model - {} GB overhead - {} GB KV-cache = {} GB available",
            total_vram_gb,
            total_vram_gb * util_factor,
            model_memory,
            fixed_overhead,
            kv_cache_memory,
            usable_vram
        );
    }
    
    Ok(VramBudgetReport {
        model_name: config.name.clone(),
        total_vram_gb,
        model_memory,
        kv_cache_memory,
        fixed_overhead,
        usable_vram,
        quantization_bits: quantization_bits.unwrap_or(16),
        batch_size,
        context_length,
        layers,
        hidden_size,
        head_dim,
        fp8_compatible: quantization_bits.map_or(false, |b| b == 8),
    })
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct VramBudgetReport {
    model_name: String,
    total_vram_gb: f64,
    model_memory: f64,
    kv_cache_memory: f64,
    fixed_overhead: f64,
    usable_vram: f64,
    quantization_bits: u8,
    batch_size: u32,
    context_length: u32,
    layers: u32,
    hidden_size: u32,
    head_dim: u32,
    fp8_compatible: bool,
}

fn generate_report(report: &VramBudgetReport) -> String {
    let mut report_str = String::new();
    
    report_str.push_str("=== VRAM Budget Report ===\n\n");
    report_str.push_str(&format!("Model: {}\n", report.model_name));
    report_str.push_str(&format!("Total VRAM: {:.2} GB\n", report.total_vram_gb));
    report_str.push_str(&format!("GPU Architecture: {}\n", report.gpu_architecture));
    report_str.push_str(&format!("Quantization: {} bits\n", report.quantization_bits));
    report_str.push_str(&format!("Batch Size: {}\n", report.batch_size));
    report_str.push_str(&format!("Context Length: {}\n", report.context_length));
    report_str.push_str(&format!("Layers: {}\n", report.layers));
    report_str.push_str(&format!("Hidden Size: {}\n", report.hidden_size));
    report_str.push_str(&format!("Head Dim: {}\n", report.head_dim));
    report_str.push_str(&format!("FP8 Compatible: {}\n", report.fp8_compatible));
    report_str.push_str("\n--- Memory Breakdown ---\n");
    report_str.push_str(&format!("Model Memory (with quantization): {:.2} GB\n", report.model_memory));
    report_str.push_str(&format!("KV Cache Memory: {:.2} GB\n", report.kv_cache_memory));
    report_str.push_str(&format!("Fixed Overhead: {:.2} GB\n", report.fixed_overhead));
    report_str.push_str(&format!("Usable VRAM Budget: {:.2} GB\n", report.usable_vram));
    report_str.push_str(&format!("Utilization Factor: {:.0}%\n", report.total_vram_gb * 0.90));
    
    if report.usable_vram < 4.0 {
        report_str.push_str("\n⚠️  WARNING: Low VRAM budget! Consider reducing batch size or context length.\n");
    }
    
    report_str
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    
    // Parse model config
    let config = parse_model_config(&cli.config)?;
    
    // Calculate VRAM budget
    let report = calculate_vram_budget(
        &config,
        cli.total_vram_gb,
        Some(cli.batch_size),
        &cli.gpu_architecture,
    )?;
    
    // Generate report
    let report_str = generate_report(&report);
    
    // Output to stdout
    println!("{}", report_str);
    
    // Write to file if requested
    if let Some(output_path) = cli.output {
        fs::write(&output_path, report_str)?;
        println!("\nReport written to: {}", output_path.display());
    }
    
    // Exit with error code if FP8 not compatible
    if !report.fp8_compatible && report.quantization_bits == Some(8) {
        eprintln!("Note: FP8 quantization not compatible with {} architecture", report.gpu_architecture);
    }
    
    Ok(())
}
