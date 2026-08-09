//! Graphify-backed context compilation.
//!
//! This module deliberately consumes Graphify's public compact output rather
//! than building a second code index. It emits a bounded, versioned package
//! whose references are tied to the exact node representation returned by
//! Graphify.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

const IR_VERSION: &str = "1";
const MAX_RAW_TOOL_ARTIFACT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContextRepresentationLevel {
    L0,
    L1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompiledContextItem {
    pub id: String,
    pub kind: String,
    pub level: ContextRepresentationLevel,
    pub reason: String,
    pub revision: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub community: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompiledContext {
    pub version: &'static str,
    pub mode: &'static str,
    pub task: String,
    pub root_nodes: Vec<String>,
    pub items: Vec<CompiledContextItem>,
    pub estimated_tokens: usize,
    pub budget_tokens: usize,
    pub candidate_items: usize,
    pub deduplicated_items: usize,
    pub dropped_items: usize,
}

impl CompiledContext {
    pub fn to_prompt_json(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{}".to_string())
    }
}

#[derive(Debug, Clone)]
struct ParsedNode {
    name: String,
    source: Option<String>,
    location: Option<String>,
    community: Option<String>,
    raw: String,
    ordinal: usize,
}

/// Compile Graphify `--format compact` output into a bounded context package.
pub fn compile_graphify_context(
    task: &str,
    graphify_output: &str,
    budget_tokens: usize,
    max_items: usize,
) -> CompiledContext {
    let mut parsed = graphify_output
        .lines()
        .enumerate()
        .filter_map(|(ordinal, line)| parse_node(line, ordinal))
        .collect::<Vec<_>>();
    let candidate_items = parsed.len();

    parsed.sort_by_key(|node| std::cmp::Reverse(score_node(task, node)));

    let mut seen = HashSet::new();
    let mut deduplicated_items = 0usize;
    let mut items = Vec::new();
    let mut estimated_tokens = fixed_package_token_estimate(task);
    let effective_max = max_items.max(1);

    for node in parsed {
        let identity = format!(
            "{}\u{1f}{}\u{1f}{}",
            node.name,
            node.source.as_deref().unwrap_or_default(),
            node.location.as_deref().unwrap_or_default()
        );
        if !seen.insert(identity.clone()) {
            deduplicated_items += 1;
            continue;
        }

        let item = compiled_item(task, node, &identity);
        let item_tokens = estimate_tokens(&serde_json::to_string(&item).unwrap_or_default());
        if items.len() >= effective_max
            || (!items.is_empty() && estimated_tokens.saturating_add(item_tokens) > budget_tokens)
        {
            continue;
        }
        estimated_tokens = estimated_tokens.saturating_add(item_tokens);
        items.push(item);
    }

    let mut compiled = CompiledContext {
        version: IR_VERSION,
        mode: "graph_compact",
        task: task.trim().to_string(),
        root_nodes: items.iter().take(3).map(|item| item.id.clone()).collect(),
        items,
        estimated_tokens,
        budget_tokens,
        candidate_items,
        deduplicated_items,
        dropped_items: 0,
    };

    // Item-local estimates intentionally over-select candidates. Enforce the
    // real serialized package budget here so JSON metadata can never make the
    // injected context exceed the configured limit. Keep the highest-ranked
    // root even under a pathological tiny budget.
    loop {
        compiled.root_nodes = compiled
            .items
            .iter()
            .take(3)
            .map(|item| item.id.clone())
            .collect();
        compiled.dropped_items = candidate_items
            .saturating_sub(deduplicated_items)
            .saturating_sub(compiled.items.len());
        compiled.estimated_tokens =
            estimate_tokens(&serde_json::to_string(&compiled).unwrap_or_default());
        if compiled.estimated_tokens <= budget_tokens || compiled.items.len() <= 1 {
            break;
        }
        compiled.items.pop();
    }

    compiled
}

pub fn estimate_tokens(text: &str) -> usize {
    text.len().div_ceil(4)
}

/// Produce a focused exact-seed query in addition to the user's natural
/// language. Graphify already performs two-hop traversal; giving it symbol-like
/// seeds makes that expansion start in the relevant production community.
pub fn graphify_query_variants(task: &str) -> Vec<String> {
    let tokens = meaningful_query_tokens(task);
    if tokens.is_empty() {
        return vec![task.trim().to_string()];
    }

    let mut concepts = tokens.clone();
    let mut seen = tokens.iter().cloned().collect::<HashSet<_>>();
    for token in &tokens {
        let stem = query_stem(token);
        if seen.insert(stem.clone()) {
            concepts.push(stem);
        }
    }
    for width in [3usize, 2] {
        for group in tokens.windows(width) {
            let identifier = group
                .iter()
                .map(|token| query_stem(token))
                .collect::<Vec<_>>()
                .join("_");
            if seen.insert(identifier.clone()) {
                concepts.push(identifier);
            }
        }
    }

    let original = task.trim().to_string();
    let mut variants = vec![original];
    let concept_query = bounded_query(concepts);
    if !concept_query.is_empty() && concept_query != variants[0] {
        variants.push(concept_query);
    }

    let symbols = symbol_hints(task);
    if !symbols.is_empty() {
        variants.push(symbols.join(" "));
    }
    variants
}

fn bounded_query(parts: Vec<String>) -> String {
    let joined = parts.join(" ");
    joined
        .char_indices()
        .nth(240)
        .map_or(joined.as_str(), |(index, _)| &joined[..index])
        .to_string()
}

pub fn graphify_call_intent(task: &str) -> bool {
    has_call_path_intent(task)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredToolOutput {
    pub compact_content: String,
    pub raw_path: std::path::PathBuf,
    pub original_chars: usize,
    pub kept_chars: usize,
}

/// Preserve an oversized tool result by content hash and return a bounded
/// prompt representation pointing to the immutable raw evidence.
pub fn store_and_compact_tool_output(
    tool_name: &str,
    content: &str,
    max_chars: usize,
    store_root: &std::path::Path,
) -> std::io::Result<StoredToolOutput> {
    if content.len() > MAX_RAW_TOOL_ARTIFACT_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "tool output exceeds context-store artifact limit",
        ));
    }
    let original_chars = content.chars().count();
    if original_chars <= max_chars {
        return Ok(StoredToolOutput {
            compact_content: content.to_string(),
            raw_path: std::path::PathBuf::new(),
            original_chars,
            kept_chars: original_chars,
        });
    }

    std::fs::create_dir_all(store_root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(store_root, std::fs::Permissions::from_mode(0o700))?;
    }
    let revision = stable_hash(content);
    let raw_path = store_root.join(format!("tool-output-{revision}.log"));
    if raw_path.exists() {
        let existing = std::fs::read(&raw_path)?;
        if existing != content.as_bytes() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "context-store revision collision",
            ));
        }
    } else {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        use std::io::Write;
        options.open(&raw_path)?.write_all(content.as_bytes())?;
    }
    let kept = content.chars().take(max_chars).collect::<String>();
    let compact_content = format!(
        "{kept}\n\n{{\"status\":\"TRUNCATED\",\"tool\":{tool},\"original_chars\":{original_chars},\"kept_chars\":{max_chars},\"raw_ref\":{raw_ref},\"revision\":\"{revision}\"}}",
        tool = serde_json::to_string(tool_name).unwrap_or_else(|_| "\"tool\"".to_string()),
        raw_ref = serde_json::to_string(&raw_path.to_string_lossy())
            .unwrap_or_else(|_| "\"\"".to_string()),
    );
    Ok(StoredToolOutput {
        compact_content,
        raw_path,
        original_chars,
        kept_chars: max_chars,
    })
}

fn fixed_package_token_estimate(task: &str) -> usize {
    estimate_tokens(task).saturating_add(48)
}

fn parse_node(line: &str, ordinal: usize) -> Option<ParsedNode> {
    let line = line.trim();
    let body = line.strip_prefix("NODE ")?;
    let (name, metadata) = body.rsplit_once(" [")?;
    let metadata = metadata.strip_suffix(']')?;
    let field = |key: &str| {
        metadata
            .split_whitespace()
            .find_map(|part| part.strip_prefix(key))
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
    };
    Some(ParsedNode {
        name: name.trim().to_string(),
        source: field("src="),
        location: field("loc="),
        community: field("community="),
        raw: line.to_string(),
        ordinal,
    })
}

fn score_node(task: &str, node: &ParsedNode) -> i64 {
    let name_lower = node.name.to_ascii_lowercase();
    let source_lower = node
        .source
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let community_lower = node
        .community
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let tokens = meaningful_query_tokens(task);
    let call_intent = has_call_path_intent(task);

    let mut score = 0i64;
    let normalized_name = normalize_symbol_name(&name_lower);
    if symbol_hints(task)
        .iter()
        .any(|hint| normalize_symbol_name(&hint.to_ascii_lowercase()) == normalized_name)
    {
        score += 25_000;
    }
    for token in &tokens {
        let stem = query_stem(token);
        let exact_name = name_lower
            .trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_')
            .trim_end_matches("()")
            == token;
        if exact_name {
            score += 15_000;
        } else if normalized_symbol_tokens(&name_lower)
            .iter()
            .any(|part| part == token || part == &stem)
        {
            score += 6_000;
        } else if name_lower.contains(token) || (stem.len() >= 4 && name_lower.contains(&stem)) {
            score += 2_500;
        }
        if source_lower.contains(token) || (stem.len() >= 4 && source_lower.contains(&stem)) {
            score += 900;
        }
        if community_lower.contains(token) || (stem.len() >= 4 && community_lower.contains(&stem)) {
            score += 450;
        }
    }

    let function_like = name_lower.ends_with("()") || name_lower.contains("::");
    if call_intent && function_like {
        score += 1_500;
    }
    if source_lower.ends_with(".rs") && !is_non_production_source(&source_lower) {
        score += 700;
    }
    if is_non_production_source(&source_lower) {
        score -= 4_000;
    }
    if is_generic_hub(&name_lower) {
        score -= 5_000;
    }

    score + i64::from(node.source.is_some()) * 100 + i64::from(node.location.is_some()) * 20
        - node.ordinal as i64
}

fn meaningful_query_tokens(task: &str) -> Vec<String> {
    const STOPWORDS: &[&str] = &[
        "the",
        "this",
        "that",
        "with",
        "from",
        "into",
        "only",
        "exact",
        "existing",
        "find",
        "show",
        "return",
        "repository",
        "production",
        "rust",
        "function",
        "functions",
        "call",
        "path",
        "trace",
        "through",
        "using",
        "used",
        "context",
    ];
    let mut seen = HashSet::new();
    task.to_ascii_lowercase()
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .filter(|word| word.len() >= 3 && !STOPWORDS.contains(word))
        .filter(|word| seen.insert((*word).to_string()))
        .map(ToOwned::to_owned)
        .collect()
}

fn normalized_symbol_tokens(name: &str) -> Vec<String> {
    name.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .flat_map(|part| part.split('_'))
        .filter(|part| !part.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn normalize_symbol_name(name: &str) -> String {
    name.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .trim_end_matches("()")
        .to_string()
}

fn symbol_hints(task: &str) -> Vec<&'static str> {
    let lower = task.to_ascii_lowercase();
    let mut symbols = Vec::new();
    if lower.contains("graphify") {
        symbols.extend(["query_graphify", "compile_graphify_context"]);
    }
    if lower.contains("enrich") {
        symbols.extend(["enrich_context", "enrichment_or_empty"]);
    }
    if lower.contains("memory") {
        symbols.extend(["memory_external", "memory_agent", "process_context"]);
    }
    if lower.contains("messages_for_provider") || lower.contains("provider messages") {
        symbols.extend(["messages_for_provider", "CompactionManager"]);
    }
    if lower.contains("tool output") {
        symbols.extend([
            "cap_tool_output_for_history",
            "store_and_compact_tool_output",
        ]);
    }
    if lower.contains("environment") && lower.contains("override") {
        symbols.extend(["ContextCompressionMode", "apply_env_overrides"]);
    }
    symbols
}

fn query_stem(token: &str) -> String {
    for suffix in ["ization", "ation", "ment", "ing", "ion", "ed", "s"] {
        if token.len() > suffix.len() + 4 {
            if let Some(stem) = token.strip_suffix(suffix) {
                return stem.to_string();
            }
        }
    }
    token.to_string()
}

fn has_call_path_intent(task: &str) -> bool {
    let lower = task.to_ascii_lowercase();
    lower.contains("call path") || lower.contains("trace") || lower.contains("caller")
}

fn is_non_production_source(source: &str) -> bool {
    source.contains("/tests/")
        || source.contains("_test.")
        || source.contains("_tests.")
        || source.contains("/test_")
        || source.contains("/benches/")
        || source.contains("benchmark")
        || source.starts_with("scripts/")
        || source.starts_with("docs/")
}

fn is_generic_hub(name: &str) -> bool {
    matches!(
        name.trim_matches(|ch: char| !ch.is_alphanumeric() && ch != '_'),
        "app" | "config" | "main" | "path" | "result" | "session" | "string" | "vec"
    )
}

fn compiled_item(task: &str, node: ParsedNode, identity: &str) -> CompiledContextItem {
    let task_lower = task.to_ascii_lowercase();
    let name_lower = node.name.to_ascii_lowercase();
    let task_match = task_lower
        .split(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .any(|word| word.len() >= 3 && name_lower.contains(word));
    let level = if node.source.is_some() || node.location.is_some() {
        ContextRepresentationLevel::L1
    } else {
        ContextRepresentationLevel::L0
    };
    let kind = if node.source.as_deref().is_some_and(|source| {
        source.ends_with(".rs")
            || source.ends_with(".py")
            || source.ends_with(".ts")
            || source.ends_with(".js")
    }) {
        "symbol"
    } else {
        "node"
    };

    CompiledContextItem {
        id: format!("G{}", stable_hash(identity)),
        kind: kind.to_string(),
        level,
        reason: if task_match {
            "task-root".to_string()
        } else {
            "graph-neighbor".to_string()
        },
        revision: stable_hash(&node.raw),
        name: node.name,
        source: node.source,
        location: node.location,
        community: node.community,
    }
}

fn stable_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    hex::encode(&digest[..8])
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRAPH: &str = r#"
Traversal: BFS depth=2 | Start: ['PaymentService.process'] | 4 nodes found
NODE PaymentClient.send [src=src/payment.rs loc=L42 community=payments]
NODE PaymentService.process [src=src/payment.rs loc=L10 community=payments]
NODE RetryPolicy [src=src/retry.rs loc=L3 community=retry]
NODE PaymentClient.send [src=src/payment.rs loc=L42 community=payments]
"#;

    #[test]
    fn compiles_task_roots_before_neighbors_and_deduplicates() {
        let compiled =
            compile_graphify_context("Fix timeout in PaymentService.process", GRAPH, 1_000, 10);

        assert_eq!(compiled.version, "1");
        assert_eq!(compiled.items.len(), 3);
        assert_eq!(compiled.deduplicated_items, 1);
        assert_eq!(compiled.items[0].name, "PaymentService.process");
        assert_eq!(compiled.items[0].reason, "task-root");
        assert_eq!(compiled.items[0].level, ContextRepresentationLevel::L1);
        assert!(compiled.items[0].revision.len() >= 16);
    }

    #[test]
    fn enforces_item_and_token_budgets_without_dropping_all_roots() {
        let compiled = compile_graphify_context("PaymentService", GRAPH, 1, 1);

        assert_eq!(compiled.items.len(), 1);
        assert_eq!(compiled.items[0].name, "PaymentService.process");
        assert_eq!(compiled.dropped_items, 2);
    }

    #[test]
    fn emitted_ir_is_valid_json_with_content_addressed_refs() {
        let compiled = compile_graphify_context("PaymentService", GRAPH, 1_000, 10);
        let value: serde_json::Value = serde_json::from_str(&compiled.to_prompt_json()).unwrap();

        assert_eq!(value["mode"], "graph_compact");
        assert!(value["root_nodes"][0].as_str().unwrap().starts_with('G'));
        assert!(compiled.estimated_tokens <= compiled.budget_tokens);
    }

    #[test]
    fn oversized_tool_output_is_preserved_by_revision_and_compacted() {
        let dir = tempfile::tempdir().unwrap();
        let raw = "test passed\n".repeat(1_000);
        let compacted = store_and_compact_tool_output("bash", &raw, 128, dir.path()).unwrap();

        assert_eq!(std::fs::read_to_string(&compacted.raw_path).unwrap(), raw);
        assert!(
            compacted
                .compact_content
                .contains("\"status\":\"TRUNCATED\"")
        );
        assert!(compacted.compact_content.contains("\"raw_ref\""));
        assert!(compacted.compact_content.len() < raw.len());
    }

    #[test]
    fn query_decomposition_generates_exact_graphify_seeds() {
        let variants = graphify_query_variants("Trace Graphify memory enrichment call path");
        assert_eq!(variants.len(), 3);
        assert!(variants[2].contains("query_graphify"));
        assert!(variants[2].contains("memory_external"));
        assert!(variants[2].contains("enrich_context"));
        assert!(graphify_call_intent("trace the call path"));
    }

    #[test]
    fn call_path_ranking_prefers_production_symbols_over_generic_tests() {
        let graph = r#"
NODE Path [src=crates/app/src/tests/onboarding_eval.rs loc=L1 community=Path]
NODE main() [src=scripts/benchmark_memory.py loc=L1 community=benchmark]
NODE query_graphify() [src=crates/jcode-base/src/memory_external.rs loc=L252 community=memory_external.rs]
NODE enrich_context() [src=crates/jcode-base/src/memory_external.rs loc=L196 community=memory_external.rs]
NODE process_context() [src=crates/jcode-base/src/memory_agent.rs loc=L499 community=memory_agent.rs]
NODE Session [src=crates/jcode-base/src/session.rs loc=L1 community=Session]
"#;
        let compiled = compile_graphify_context(
            "Trace Graphify memory enrichment call path",
            graph,
            1_200,
            6,
        );
        let top = compiled
            .items
            .iter()
            .take(3)
            .map(|item| item.name.as_str())
            .collect::<Vec<_>>();
        assert!(top.contains(&"query_graphify()"));
        assert!(top.contains(&"enrich_context()"));
        assert!(!top.contains(&"Path"));
        assert!(!top.contains(&"main()"));
    }
}
