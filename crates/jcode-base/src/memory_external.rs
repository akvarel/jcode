//! External memory enrichment sources: graphify, Obsidian vault, pgvector RAG.
//!
//! The AGENTS.md rules instruct the agent to query these sources before answering
//! codebase questions. This module implements automatic enrichment so the memory
//! pipeline injects relevant external context without relying on the agent to read
//! those instructions — it calls:
//!
//! - **Graphify** — AST-based codebase knowledge graph (`graphify query`)
//! - **Obsidian vault** — Zettelkasten notes under `/sharedssd/vault/`
//! - **Pgvector RAG** — vector DB-backed vault search (OrangeHat infra)
//!
//! Each source is controlled by a config flag (opt-in, default off) and runs with
//! a configurable timeout.

use anyhow::Result;
use std::time::Duration;

use crate::config::config;
use crate::memory::{MemoryCategory, MemoryEntry};

/// A single external-enrichment result, analogous to a memory retrieval hit.
#[derive(Debug, Clone)]
pub struct ExternalEnrichment {
    /// The content text.
    pub content: String,
    /// Source label for display ("graphify", "vault", "pgvector").
    pub source: &'static str,
    /// Optional short identifier (e.g. file path, node id).
    pub source_id: Option<String>,
    /// Optional relevance / similarity hint (range 0.0-1.0).
    pub relevance: Option<f32>,
}

/// Configurable timeouts and limits for each enrichment source.
struct EnrichmentLimits {
    pub max_results: usize,
    pub timeout: Duration,
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
/// Sources are only queried when their corresponding config flag is `true`:
/// - `agents.memory_graphify_enabled`
/// - `agents.memory_vault_enabled`
/// - `agents.memory_pgvector_enabled`
pub async fn enrich_context(context: &str) -> Vec<MemoryEntry> {
    let cfg = config();
    let mut all: Vec<MemoryEntry> = Vec::new();

    if cfg.agents.memory_graphify_enabled {
        let mut res = query_graphify(context).await.unwrap_or_default();
        all.append(&mut entries_from_enrichments(
            &mut res,
            "graphify",
            "graphify-codebase",
        ));
    }

    if cfg.agents.memory_vault_enabled {
        let mut res = search_vault(context).await.unwrap_or_default();
        all.append(&mut entries_from_enrichments(
            &mut res,
            "vault",
            "vault-obsidian",
        ));
    }

    if cfg.agents.memory_pgvector_enabled {
        let mut res = search_pgvector(context).await.unwrap_or_default();
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
///
/// The `graphify` binary must be on `$PATH`. Returns structured results parsed
/// from the CLI output (node path, content snippet, community label).
async fn query_graphify(query_text: &str) -> Result<Vec<ExternalEnrichment>> {
    // Truncate long queries to avoid shell-argument blow-up.
    let truncated = if query_text.len() > 240 {
        &query_text[..240]
    } else {
        query_text
    };

    let output = tokio::time::timeout(
        GRAPHIFY_LIMITS.timeout,
        tokio::process::Command::new("graphify")
            .arg("query")
            .arg(truncated)
            .arg("--format")
            .arg("compact")
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
    let mut results: Vec<ExternalEnrichment> = Vec::new();

    for line in stdout.lines().take(GRAPHIFY_LIMITS.max_results) {
        let line = line.trim();
        if line.is_empty() || line.starts_with("NODE") {
            continue;
        }
        // graphify compact format: "path:content"
        let content = line.to_string();
        results.push(ExternalEnrichment {
            content,
            source: "graphify",
            source_id: None,
            relevance: None,
        });
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Obsidian Vault
// ---------------------------------------------------------------------------

/// Limits for vault search.
const VAULT_LIMITS: EnrichmentLimits = EnrichmentLimits {
    max_results: 8,
    timeout: Duration::from_secs(10),
};

const VAULT_ROOT: &str = "/sharedssd/vault";

/// Search the Obsidian vault for notes matching the query.
///
/// Performs a content search over `.md` files under `/sharedssd/vault/`,
/// returning matching file paths and their title (from YAML frontmatter).
async fn search_vault(query_text: &str) -> Result<Vec<ExternalEnrichment>> {
    let vault = std::path::Path::new(VAULT_ROOT);
    if !vault.exists() {
        return Ok(Vec::new());
    }

    // Use ripgrep for fast content search; fall back to a slow find+grep.
    let output = tokio::time::timeout(
        VAULT_LIMITS.timeout,
        async {
            // Try rg first
            let rg_result = tokio::process::Command::new("rg")
                .arg("-l")
                .arg("-i")
                .arg("--max-count")
                .arg("5")
                .arg(query_text)
                .arg(VAULT_ROOT)
                .arg("--type")
                .arg("md")
                .output()
                .await;

            match rg_result {
                Ok(out) if out.status.success() => Ok(out),
                Ok(_) | Err(_) => {
                    // Fallback to find + grep
                    tokio::process::Command::new("sh")
                        .arg("-c")
                        .arg(format!(
                            "find {VAULT_ROOT} -name '*.md' -exec grep -li -m1 '{q}' {{}} \\; 2>/dev/null | head -{n}",
                            q = query_text.replace('\'', "'\\''"),
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

        // Extract title from YAML frontmatter (first 3 lines)
        let title = extract_vault_title(path).unwrap_or_else(|| file_name.clone());
        let rel_path = path
            .strip_prefix(VAULT_ROOT)
            .unwrap_or(path)
            .display()
            .to_string();

        results.push(ExternalEnrichment {
            content: format!("[{title}]({rel_path})"),
            source: "vault",
            source_id: Some(rel_path),
            relevance: None,
        });
    }

    Ok(results)
}

/// Extract YAML frontmatter `title:` from a vault markdown file.
fn extract_vault_title(path: &std::path::Path) -> Option<String> {
    use std::io::BufRead;
    let file = std::fs::File::open(path).ok()?;
    let reader = std::io::BufReader::new(file);
    let mut in_frontmatter = false;
    for line in reader.lines().take(10).flatten() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if in_frontmatter {
                break; // end of frontmatter
            }
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if let Some(title) = trimmed.strip_prefix("title:") {
                return Some(title.trim().trim_matches('"').to_string());
            }
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Pgvector RAG
// ---------------------------------------------------------------------------

/// Limits for pgvector RAG search.
const PGVECTOR_LIMITS: EnrichmentLimits = EnrichmentLimits {
    max_results: 8,
    timeout: Duration::from_secs(10),
};

/// Search the OrangeHat pgvector-backed vault via the `search_memory.py` script.
///
/// The script lives at `/sharedssd/scripts/search_memory.py` and accepts a
/// query string argument, returning results as newline-separated text.
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
            relevance: None,
        });
    }

    Ok(results)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Convert external enrichment results into injectable MemoryEntry values.
///
/// Each entry is tagged with a unique synthetic category
/// (`MemoryCategory::External`) so the sidecar and downstream code can
/// distinguish external enrichments from native jcode memories.
fn entries_from_enrichments(
    enrichments: &mut Vec<ExternalEnrichment>,
    source_label: &'static str,
    category_label: &'static str,
) -> Vec<MemoryEntry> {
    enrichments
        .drain(..)
        .map(|e| {
            let cat = MemoryCategory::Custom(format!("external:{category_label}"));
            let prefixed = format!("[{}] {}", source_label, e.content);
            let mut entry = MemoryEntry::new(cat, prefixed)
                .with_source(e.source);
            if let Some(id) = e.source_id {
                entry.tags.push(id);
            }
            entry.tags.push(source_label.to_string());
            entry.tags.push("external_enrichment".to_string());
            entry
        })
        .collect()
}
