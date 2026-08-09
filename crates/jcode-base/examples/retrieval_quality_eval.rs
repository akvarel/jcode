use jcode_base::context_compiler::{
    compile_graphify_context, graphify_call_intent, graphify_query_variants,
};
use serde::Serialize;
use std::process::Command;

#[derive(Clone, Copy)]
struct GoldenCase {
    task: &'static str,
    expected: &'static [&'static str],
}

#[derive(Serialize)]
struct CaseReport {
    task: &'static str,
    expected: &'static [&'static str],
    top_five: Vec<String>,
    recall_at_five: f64,
    reciprocal_rank: f64,
    irrelevant_at_five: usize,
}

#[derive(Serialize)]
struct Report {
    cases: Vec<CaseReport>,
    mean_recall_at_five: f64,
    mean_reciprocal_rank: f64,
    mean_irrelevant_at_five: f64,
}

const CASES: &[GoldenCase] = &[
    GoldenCase {
        task: "Trace Graphify memory enrichment call path",
        expected: &[
            "query_graphify()",
            "enrich_context()",
            ".process_context()",
            "compile_graphify_context()",
        ],
    },
    GoldenCase {
        task: "Trace messages_for_provider context compaction call path",
        expected: &["messages_for_provider()", "CompactionManager"],
    },
    GoldenCase {
        task: "Trace tool output history compression call path",
        expected: &[
            "cap_tool_output_for_history()",
            "store_and_compact_tool_output()",
        ],
    },
    GoldenCase {
        task: "Find context compression environment override configuration",
        expected: &["ContextCompressionMode", "apply_env_overrides()"],
    },
];

fn graphify_output(task: &str) -> String {
    let mut merged = String::new();
    for query in graphify_query_variants(task) {
        let mut command = Command::new("graphify");
        command.args(["query", &query, "--format", "compact", "--budget", "2400"]);
        if graphify_call_intent(task) {
            command.args(["--context-filter", "call"]);
        }
        let output = command.output().expect("graphify must be installed");
        assert!(
            output.status.success(),
            "graphify query failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        merged.push_str(&String::from_utf8_lossy(&output.stdout));
        merged.push('\n');
    }
    merged
}

fn main() {
    let mut reports = Vec::new();
    for case in CASES {
        let raw = graphify_output(case.task);
        let compiled = compile_graphify_context(case.task, &raw, 1_200, 24);
        let top_five = compiled
            .items
            .iter()
            .take(5)
            .map(|item| item.name.clone())
            .collect::<Vec<_>>();
        let hits = case
            .expected
            .iter()
            .filter(|expected| {
                top_five
                    .iter()
                    .any(|name| normalize(name) == normalize(expected))
            })
            .count();
        let first_rank = top_five.iter().enumerate().find_map(|(index, name)| {
            case.expected
                .iter()
                .any(|expected| normalize(name) == normalize(expected))
                .then_some(index + 1)
        });
        reports.push(CaseReport {
            task: case.task,
            expected: case.expected,
            top_five,
            recall_at_five: hits as f64 / case.expected.len() as f64,
            reciprocal_rank: first_rank.map_or(0.0, |rank| 1.0 / rank as f64),
            irrelevant_at_five: 5usize.saturating_sub(hits),
        });
    }
    let count = reports.len() as f64;
    let report = Report {
        mean_recall_at_five: reports.iter().map(|case| case.recall_at_five).sum::<f64>() / count,
        mean_reciprocal_rank: reports.iter().map(|case| case.reciprocal_rank).sum::<f64>() / count,
        mean_irrelevant_at_five: reports
            .iter()
            .map(|case| case.irrelevant_at_five as f64)
            .sum::<f64>()
            / count,
        cases: reports,
    };
    println!("{}", serde_json::to_string_pretty(&report).unwrap());
}

fn normalize(name: &str) -> String {
    name.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .trim_end_matches("()")
        .to_ascii_lowercase()
}
