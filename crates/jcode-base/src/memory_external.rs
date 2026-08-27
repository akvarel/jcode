//! External memory enrichment sources: graphify, Obsidian vault, pgvector RAG.
//!
//! The AGENTS.md rules instruct the agent to query these sources before answering
//! codebase questions. This module implements automatic enrichment so the memory
//! pipeline injects relevant external context without relying on the agent to read
//! those instructions — it calls:
//!
//! - **Graphify** — AST-based codebase knowledge graph (`graphify query`)
//! - **Obsidian vault** — Zettelkasten notes under `~/vault/` (configurable)
//! - **Pgvector RAG** — vector DB-backed vault search (OrangeHat infra)
//!
//! Each source is controlled by a config flag (opt-in, default off) and runs with
//! a configurable timeout.
//!
//! **Smart trigger**: enrichment only fires when the context contains codebase
//! signals (file paths, function names, architecture terms), to avoid injecting
//! noise on everyday chat turns.

use anyhow::Result;
use std::time::Duration;

use crate::config::config;
use crate::memory::{MemoryCategory, MemoryEntry};

/// Default vault root when `memory_vault_root` is not configured — user's home.
fn default_vault_root() -> String {
    dirs::home_dir()
        .map(|h| h.join("vault").to_string_lossy().to_string())
        .unwrap_or_else(|| "/sharedssd/vault".to_string())
}

// ---------------------------------------------------------------------------
// Smart trigger — enrichment only on codebase-relevant context
// ---------------------------------------------------------------------------

/// Signal terms that suggest a codebase or architecture question.
const CODEBASE_KEYWORDS: &[&str] = &[
    // File system signals
    "src/",
    "lib/",
    "crates/",
    "Cargo.toml",
    "Cargo.lock",
    ".rs",
    ".py",
    ".js",
    ".ts",
    ".go",
    ".rs ",
    ".py ",
    ".ts ",
    // Architecture / design
    "architecture",
    "design",
    "pattern",
    "struct",
    "trait",
    "impl",
    "module",
    "function",
    "method",
    "class",
    "interface",
    "type",
    "pipeline",
    "flow",
    "diagram",
    "graph",
    "memory",
    "hook",
    // Codebase verbs
    "how does",
    "where is",
    "what is",
    "how is",
    "show me",
    "find",
    "search",
    "locate",
    "explain",
    "implement",
    // OrangeHat / system paths
    "/sharedssd/",
    "~/.jcode",
    "config.toml",
    "AGENTS.md",
    ".jcode",
    "$HOME",
    "$PATH",
    // English technical
    "codebase",
    "code base",
    "repository",
    "repo",
    "api",
    "endpoint",
    "route",
    "middleware",
    "service",
    "handler",
    // Russian technical (code/architecture questions)
    "код",
    "архитектура",
    "память",
    "структура",
    "функция",
    "класс",
    "метод",
    "модуль",
    "схема",
    "поток",
];

/// Return `true` when the context contains enough signal to warrant enrichment.
///
/// Checks for codebase-related terms, file paths, identifiers with underscores,
/// and known system paths. Short or query-free contexts are skipped to avoid
/// triggering on simple greetings or small-talk.
pub fn should_enrich_context(context: &str) -> bool {
    let ctx = context.trim();
    if ctx.len() < 20 {
        // Too short to be a meaningful codebase question
        return false;
    }

    let ctx_lower = ctx.to_ascii_lowercase();

    // Check for explicit codebase keywords
    for kw in CODEBASE_KEYWORDS {
        if ctx_lower.contains(kw) {
            return true;
        }
    }

    // Check for path-like patterns
    if ctx_lower.contains('/') || ctx_lower.contains("\\") {
        return true;
    }

    // Check for identifier patterns (snake_case or CamelCase)
    let words: Vec<&str> = ctx_lower.split_whitespace().collect();
    let word_count = words.len();
    let mut identifier_count = 0;
    for w in &words {
        // snake_case signal
        if w.contains('_') && w.chars().any(|c| c.is_ascii_lowercase()) && !w.contains("__") {
            identifier_count += 1;
        }
        // CamelCase signal
        let upper_count = w.chars().filter(|c| c.is_ascii_uppercase()).count();
        if upper_count >= 2 && w.len() >= 4 {
            identifier_count += 1;
        }
    }

    if identifier_count > 0 && identifier_count as f64 / word_count as f64 > 0.15 {
        return true;
    }

    // Check for file extensions
    if ctx_lower.contains(".rs")
        || ctx_lower.contains(".py")
        || ctx_lower.contains(".js")
        || ctx_lower.contains(".ts")
        || ctx_lower.contains(".toml")
        || ctx_lower.contains(".json")
        || ctx_lower.contains(".yaml")
        || ctx_lower.contains(".yml")
        || ctx_lower.contains(".md")
        || ctx_lower.contains(".go")
    {
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Enrich a search context by querying enabled external sources.
///
/// Returns a list of [`MemoryEntry`] values suitable for injection alongside
/// regular memory search results. Each entry is tagged with a synthetic source
/// so downstream code (and the sidecar) can distinguish jcode memories from
/// external enrichments.
///
/// **Smart trigger**: enrichment is skipped when `should_enrich_context()`
/// returns false, avoiding noise on non-codebase turns.
///
/// Sources are only queried when their corresponding config flag is `true`:
/// - `agents.memory_graphify_enabled`
/// - `agents.memory_vault_enabled`
/// - `agents.memory_pgvector_enabled`
pub async fn enrich_context(context: &str) -> Vec<MemoryEntry> {
    if !should_enrich_context(context) {
        return Vec::new();
    }

    let cfg = config();
    let mut all: Vec<MemoryEntry> = Vec::new();

    if cfg.agents.memory_graphify_enabled {
        let mut res = enrichment_or_empty("graphify", query_graphify(context).await);
        all.append(&mut entries_from_enrichments(
            &mut res,
            "graphify",
            "graphify-codebase",
        ));
    }

    if cfg.agents.memory_vault_enabled {
        let vault_root = cfg
            .agents
            .memory_vault_root
            .clone()
            .unwrap_or_else(default_vault_root);
        let mut res = enrichment_or_empty("vault", search_vault(context, &vault_root).await);
        all.append(&mut entries_from_enrichments(
            &mut res,
            "vault",
            "vault-obsidian",
        ));
    }

    if cfg.agents.memory_pgvector_enabled {
        let mut res = enrichment_or_empty("pgvector", search_pgvector(context).await);
        all.append(&mut entries_from_enrichments(
            &mut res,
            "pgvector",
            "pgvector-rag",
        ));
    }

    all
}

// ---------------------------------------------------------------------------
// Graphify
// ---------------------------------------------------------------------------

/// Limits for graphify query execution.
const GRAPHIFY_LIMITS: EnrichmentLimits = EnrichmentLimits {
    max_results: 10,
    timeout: Duration::from_secs(15),
};

/// Query the graphify codebase knowledge graph via `graphify query`.
async fn query_graphify(query_text: &str) -> Result<Vec<ExternalEnrichment>> {
    let truncated = truncate_graphify_query(query_text, 240);

    let output = tokio::time::timeout(
        GRAPHIFY_LIMITS.timeout,
        tokio::process::Command::new("graphify")
            .arg("query")
            .arg(truncated)
            .arg("--budget")
            .arg("1200")
            .output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("graphify query timed out after 15s"))?
    .map_err(|e| anyhow::anyhow!("failed to run graphify: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        crate::logging::warn(&format!("graphify query failed: {stderr}"));
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(parse_graphify_output(&stdout, GRAPHIFY_LIMITS.max_results))
}

fn truncate_graphify_query(query: &str, max_bytes: usize) -> &str {
    if query.len() <= max_bytes {
        return query;
    }
    let mut boundary = max_bytes;
    while boundary > 0 && !query.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &query[..boundary]
}

fn parse_graphify_output(output: &str, max_results: usize) -> Vec<ExternalEnrichment> {
    output
        .lines()
        .filter_map(parse_graphify_line)
        .take(max_results)
        .collect()
}

fn parse_graphify_line(line: &str) -> Option<ExternalEnrichment> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("NODE ") {
        let metadata_start = rest.find(" [src=")?;
        let label = rest[..metadata_start].trim();
        let metadata_and_summary = &rest[metadata_start + 2..];
        let metadata_end = metadata_and_summary.find(']')?;
        let metadata = &metadata_and_summary[..metadata_end];
        let summary = metadata_and_summary[metadata_end + 1..]
            .trim()
            .strip_prefix("summary=")
            .unwrap_or("")
            .trim();
        let source_file = metadata
            .split_whitespace()
            .find_map(|part| part.strip_prefix("src="))
            .unwrap_or("");
        let source_location = metadata
            .split_whitespace()
            .find_map(|part| part.strip_prefix("loc="))
            .unwrap_or("");
        let is_knowledge = source_file == "lat.md" || source_file.starts_with("lat.md/");
        let kind = if is_knowledge {
            "knowledge_section"
        } else {
            "code_node"
        };
        let mut content = format!("kind: {kind}\nlabel: {label}");
        if !summary.is_empty() {
            content.push_str("\nsummary: ");
            content.push_str(summary);
        }
        if !source_file.is_empty() {
            content.push_str("\nsource: ");
            content.push_str(source_file);
            if !source_location.is_empty() {
                content.push(':');
                content.push_str(source_location);
            }
        }
        let source_id = (!source_file.is_empty()).then(|| {
            if source_location.is_empty() {
                source_file.to_string()
            } else {
                format!("{source_file}:{source_location}")
            }
        });
        return Some(ExternalEnrichment {
            content,
            source: "graphify",
            source_id,
            tags: vec![if is_knowledge {
                "graphify_knowledge".to_string()
            } else {
                "graphify_code".to_string()
            }],
        });
    }
    None
}

// ---------------------------------------------------------------------------
// Obsidian Vault
// ---------------------------------------------------------------------------

/// Limits for vault search.
const VAULT_LIMITS: EnrichmentLimits = EnrichmentLimits {
    max_results: 6,
    timeout: Duration::from_secs(10),
};

/// Search the Obsidian vault for notes matching the query.
///
/// `vault_root` comes from config `agents.memory_vault_root` or defaults to
/// `/sharedssd/vault`.
async fn search_vault(query_text: &str, vault_root: &str) -> Result<Vec<ExternalEnrichment>> {
    let vault = std::path::Path::new(vault_root);
    if !vault.exists() {
        return Ok(Vec::new());
    }

    let output = tokio::time::timeout(
        VAULT_LIMITS.timeout,
        async {
            let rg_result = tokio::process::Command::new("rg")
                .arg("-l")
                .arg("-i")
                .arg("--max-count")
                .arg("5")
                .arg(query_text)
                .arg(vault_root)
                .arg("--type")
                .arg("md")
                .output()
                .await;

            match rg_result {
                Ok(out) if out.status.success() => Ok(out),
                Ok(_) | Err(_) => {
                    // Fallback to find + grep
                    let escaped = query_text.replace('\'', "'\\''");
                    tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(format!(
                            "find {root} -name '*.md' -exec grep -li -m1 '{q}' {{}} \\; 2>/dev/null | head -{n}",
                            root = vault_root,
                            q = escaped,
                            n = VAULT_LIMITS.max_results,
                        ))
                        .output()
                        .await
                        .map_err(|e| anyhow::anyhow!("vault fallback grep failed: {e}"))
                }
            }
        },
    )
    .await
    .map_err(|_| anyhow::anyhow!("vault search timed out"))?
    .map_err(|e| anyhow::anyhow!("vault search failed: {e}"))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();

    for path_str in stdout.lines().take(VAULT_LIMITS.max_results) {
        let path_str = path_str.trim();
        if path_str.is_empty() {
            continue;
        }
        let path = std::path::Path::new(path_str);
        let file_name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();

        let title = extract_vault_title(path).unwrap_or_else(|| file_name.clone());
        let rel_path = path
            .strip_prefix(vault_root)
            .unwrap_or(path)
            .display()
            .to_string();

        results.push(ExternalEnrichment {
            content: format!("[{title}]({rel_path})"),
            source: "vault",
            source_id: Some(rel_path),
            tags: Vec::new(),
        });
    }

    Ok(results)
}

/// Extract YAML frontmatter `title:` from a vault markdown file.
fn extract_vault_title(path: &std::path::Path) -> Option<String> {
    use std::io::BufRead;
    let file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return None,
    };
    let reader = std::io::BufReader::new(file);
    let mut in_frontmatter = false;
    for line in reader.lines().take(10).flatten() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter && let Some(title) = trimmed.strip_prefix("title:") {
            return Some(title.trim().trim_matches('"').to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Pgvector RAG
// ---------------------------------------------------------------------------

/// Limits for pgvector RAG search.
const PGVECTOR_LIMITS: EnrichmentLimits = EnrichmentLimits {
    max_results: 6,
    timeout: Duration::from_secs(10),
};

/// Search the OrangeHat pgvector-backed vault via the `search_memory.py` script.
async fn search_pgvector(query_text: &str) -> Result<Vec<ExternalEnrichment>> {
    let script = std::path::Path::new("/sharedssd/scripts/search_memory.py");
    if !script.exists() {
        return Ok(Vec::new());
    }

    let output = tokio::time::timeout(
        PGVECTOR_LIMITS.timeout,
        tokio::process::Command::new("python3")
            .arg(script)
            .arg(query_text)
            .arg("--limit")
            .arg(PGVECTOR_LIMITS.max_results.to_string())
            .output(),
    )
    .await
    .map_err(|_| anyhow::anyhow!("pgvector search timed out"))?
    .map_err(|e| anyhow::anyhow!("failed to run search_memory.py: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        crate::logging::warn(&format!("pgvector search failed: {stderr}"));
        return Ok(Vec::new());
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut results = Vec::new();
    for line in stdout.lines().take(PGVECTOR_LIMITS.max_results) {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        results.push(ExternalEnrichment {
            content: line.to_string(),
            source: "pgvector",
            source_id: None,
            tags: Vec::new(),
        });
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Internal types and helpers
// ---------------------------------------------------------------------------

/// A single external-enrichment result, analogous to a memory retrieval hit.
#[derive(Debug, Clone)]
struct ExternalEnrichment {
    content: String,
    source: &'static str,
    source_id: Option<String>,
    tags: Vec<String>,
}

/// Configurable timeouts and limits for each enrichment source.
struct EnrichmentLimits {
    max_results: usize,
    timeout: Duration,
}

fn enrichment_or_empty(
    source: &str,
    result: Result<Vec<ExternalEnrichment>>,
) -> Vec<ExternalEnrichment> {
    match result {
        Ok(enrichments) => enrichments,
        Err(error) => {
            crate::logging::warn(&format!("{source} external enrichment failed: {error}"));
            Vec::new()
        }
    }
}

/// Convert external enrichment results into injectable MemoryEntry values.
///
/// Each entry is tagged with a unique synthetic category
/// (`MemoryCategory::Custom("external:...")`) so the sidecar and downstream code
/// can distinguish external enrichments from native jcode memories.
fn entries_from_enrichments(
    enrichments: &mut Vec<ExternalEnrichment>,
    source_label: &'static str,
    category_label: &'static str,
) -> Vec<MemoryEntry> {
    enrichments
        .drain(..)
        .map(|mut e| {
            let cat = MemoryCategory::Custom(format!("external:{category_label}"));
            let prefixed = format!("[{}] {}", source_label, e.content);
            let mut entry = MemoryEntry::new(cat, prefixed).with_source(e.source);
            if let Some(id) = e.source_id {
                entry.tags.push(id);
            }
            entry.tags.append(&mut e.tags);
            entry.tags.push(source_label.to_string());
            entry.tags.push("external_enrichment".to_string());
            entry.refresh_search_text();
            entry
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphify_output_becomes_structured_knowledge_and_code_records() {
        let output = "Traversal: BFS depth=2 | Start: ['Tenant isolation'] | 3 nodes found\n\n\
NODE Tenant isolation [src=lat.md/security.md loc=L3 community=] summary=All reads require tenant_id.\n\
NODE repository.py [src=src/repository.py loc=L1 community=]\n\
EDGE Tenant isolation --implemented_by [EXTRACTED]--> repository.py at=src/repository.py:L1\n";

        let records = parse_graphify_output(output, 10);

        assert_eq!(records.len(), 2);
        assert_eq!(
            records[0].source_id.as_deref(),
            Some("lat.md/security.md:L3")
        );
        assert!(records[0].content.contains("kind: knowledge_section"));
        assert!(
            records[0]
                .content
                .contains("summary: All reads require tenant_id.")
        );
        assert!(records[0].tags.contains(&"graphify_knowledge".to_string()));
        assert!(records[1].content.contains("kind: code_node"));
        assert!(records[1].tags.contains(&"graphify_code".to_string()));
    }

    #[test]
    fn graphify_query_truncation_is_unicode_safe_and_bounded() {
        let query = "архитектура ".repeat(40);
        let truncated = truncate_graphify_query(&query, 240);

        assert!(truncated.len() <= 240);
        assert!(query.starts_with(truncated));
        assert!(std::str::from_utf8(truncated.as_bytes()).is_ok());
    }

    #[test]
    fn structured_graphify_tags_reach_memory_entries() {
        let mut records = parse_graphify_output(
            "NODE Tenant isolation [src=lat.md/security.md loc=L3 community=] summary=Scoped reads.\n",
            10,
        );

        let entries = entries_from_enrichments(&mut records, "graphify", "graphify-codebase");

        assert_eq!(entries.len(), 1);
        assert!(entries[0].tags.contains(&"graphify_knowledge".to_string()));
        assert!(
            entries[0]
                .tags
                .contains(&"lat.md/security.md:L3".to_string())
        );
        assert!(entries[0].search_text.contains("graphify knowledge"));
        assert!(entries[0].search_text.contains("lat md security md l3"));
    }
}
