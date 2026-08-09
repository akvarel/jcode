use jcode_base::context_compiler::{compile_graphify_context, estimate_tokens};
use serde::Serialize;
use std::process::Command;

#[derive(Serialize)]
struct EvaluationReport {
    task: String,
    baseline: Metrics,
    graph_compact: Metrics,
    input_compression_ratio: f64,
    root_nodes_preserved: usize,
}

#[derive(Serialize)]
struct Metrics {
    context_bytes: usize,
    context_tokens_estimated: usize,
    context_items: usize,
    compression_mode: &'static str,
    compression_version: &'static str,
}

fn main() {
    let task = std::env::args().skip(1).collect::<Vec<_>>().join(" ");
    let task = if task.trim().is_empty() {
        "messages_for_provider context compaction".to_string()
    } else {
        task
    };
    let budget = std::env::var("JCODE_CONTEXT_GRAPH_TOKEN_BUDGET")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(1_200);
    let max_items = std::env::var("JCODE_CONTEXT_MAX_GRAPH_ITEMS")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(24);

    let output = Command::new("graphify")
        .args(["query", &task, "--format", "compact"])
        .output()
        .expect("graphify must be installed and available on PATH");
    if !output.status.success() {
        eprintln!(
            "graphify query failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::process::exit(2);
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let candidate_items = raw.lines().filter(|line| line.starts_with("NODE ")).count();
    let baseline_tokens = estimate_tokens(&raw);
    let effective_budget = budget.min(baseline_tokens);
    let compiled = compile_graphify_context(&task, &raw, effective_budget, max_items);
    let compiled_json = compiled.to_prompt_json();
    let compressed_tokens = estimate_tokens(&compiled_json);
    let report = EvaluationReport {
        task,
        baseline: Metrics {
            context_bytes: raw.len(),
            context_tokens_estimated: baseline_tokens,
            context_items: candidate_items,
            compression_mode: "off",
            compression_version: "baseline",
        },
        graph_compact: Metrics {
            context_bytes: compiled_json.len(),
            context_tokens_estimated: compressed_tokens,
            context_items: compiled.items.len(),
            compression_mode: compiled.mode,
            compression_version: compiled.version,
        },
        input_compression_ratio: if compressed_tokens == 0 {
            0.0
        } else {
            baseline_tokens as f64 / compressed_tokens as f64
        },
        root_nodes_preserved: compiled.root_nodes.len(),
    };

    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}
