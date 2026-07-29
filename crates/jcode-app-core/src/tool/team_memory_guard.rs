use anyhow::{Result, bail};
use std::path::Path;

const MAX_ENTRY_LINES: usize = 50;
const REQUIRED_SECTION_MARKERS: &[&str] = &[
    "summary",
    "what was done",
    "completed work",
    "changes",
    "decisions",
    "problems",
    "files",
    "validation",
    "next steps",
    "next suggested steps",
];

pub(crate) fn is_team_memory_session_log(path: &Path) -> bool {
    let parts = path
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>();
    parts.len() >= 2
        && parts[parts.len() - 2].eq_ignore_ascii_case("TEAM_MEMORY")
        && parts[parts.len() - 1].eq_ignore_ascii_case("SESSION_LOG.md")
}

pub(crate) fn validate_team_memory_session_log_update(
    path: &Path,
    old_content: &str,
    new_content: &str,
    writer_allowed: bool,
) -> Result<()> {
    if !is_team_memory_session_log(path) {
        return Ok(());
    }
    if !writer_allowed {
        bail!(
            "Only the root or coordinator session may modify TEAM_MEMORY/SESSION_LOG.md; swarm workers must report findings to their coordinator"
        )
    }

    let lowered = new_content.to_ascii_lowercase();
    if lowered.contains("session ended") {
        bail!(
            "TEAM_MEMORY entries must be substantive summaries; generic 'Session ended' stubs are forbidden"
        )
    }
    if contains_session_id(new_content) {
        bail!("TEAM_MEMORY must not contain Jcode session IDs")
    }
    if lowered.contains("transcript link")
        || lowered.contains("/sessions/")
        || lowered.contains("jcode://session")
    {
        bail!("TEAM_MEMORY must not contain transcript or session links")
    }

    let added = added_suffix(old_content, new_content);
    if added.trim().is_empty() {
        return Ok(());
    }
    validate_new_entries(added)?;
    reject_duplicate_entries(old_content, added)
}

fn added_suffix<'a>(old_content: &str, new_content: &'a str) -> &'a str {
    new_content.strip_prefix(old_content).unwrap_or(new_content)
}

fn contains_session_id(text: &str) -> bool {
    text.split(|ch: char| {
        ch.is_whitespace() || matches!(ch, '*' | '`' | '(' | ')' | '[' | ']' | ',')
    })
    .any(|word| {
        let word = word.trim_matches(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_');
        word.starts_with("session_") && word.len() > "session_".len() + 6
    })
}

fn validate_new_entries(added: &str) -> Result<()> {
    let mut entries = Vec::new();
    let mut current = Vec::new();
    for line in added.lines() {
        if line.starts_with("## ") && !current.is_empty() {
            entries.push(current);
            current = Vec::new();
        }
        current.push(line);
    }
    if !current.is_empty() {
        entries.push(current);
    }

    for entry in entries {
        let nonempty = entry.iter().filter(|line| !line.trim().is_empty()).count();
        if nonempty == 0 {
            continue;
        }
        if entry.len() > MAX_ENTRY_LINES {
            bail!("Each TEAM_MEMORY session summary must be at most {MAX_ENTRY_LINES} lines")
        }
        let text = entry.join("\n").to_ascii_lowercase();
        let bullet_count = entry
            .iter()
            .filter(|line| line.trim_start().starts_with("- "))
            .count();
        let section_count = REQUIRED_SECTION_MARKERS
            .iter()
            .filter(|marker| text.contains(**marker))
            .count();
        if nonempty < 5 || bullet_count < 2 || section_count < 2 {
            bail!(
                "TEAM_MEMORY entries must be substantive summaries with at least two named sections and two concrete bullet points"
            )
        }
    }
    Ok(())
}

fn reject_duplicate_entries(old_content: &str, added: &str) -> Result<()> {
    let existing = split_entries(old_content)
        .into_iter()
        .map(normalize_entry)
        .filter(|entry| !entry.is_empty())
        .collect::<Vec<_>>();
    for entry in split_entries(added) {
        let normalized = normalize_entry(entry);
        if !normalized.is_empty() && existing.iter().any(|candidate| candidate == &normalized) {
            bail!("TEAM_MEMORY must not append duplicate task or session summaries")
        }
    }
    Ok(())
}

fn split_entries(text: &str) -> Vec<&str> {
    let mut entries = Vec::new();
    let mut start = None;
    for (offset, line) in text.match_indices("## ") {
        if (offset == 0 || text.as_bytes().get(offset - 1) == Some(&b'\n')) && line == "## " {
            if let Some(previous) = start.replace(offset) {
                entries.push(&text[previous..offset]);
            }
        }
    }
    if let Some(start) = start {
        entries.push(&text[start..]);
    }
    entries
}

fn normalize_entry(entry: &str) -> String {
    entry
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path() -> &'static Path {
        Path::new("/repo/TEAM_MEMORY/SESSION_LOG.md")
    }

    #[test]
    fn rejects_worker_writes() {
        let err =
            validate_team_memory_session_log_update(path(), "", "# Log\n", false).unwrap_err();
        assert!(err.to_string().contains("root or coordinator"));
    }

    #[test]
    fn rejects_session_stub_and_identifiers() {
        let stub = "## 2026-07-29 — Session ended\n\n**Session:** session_fox_123456789\n";
        assert!(validate_team_memory_session_log_update(path(), "", stub, true).is_err());
    }

    #[test]
    fn rejects_insubstantial_entry() {
        let entry = "## 2026-07-29 — Work\n\n- Done.\n";
        assert!(validate_team_memory_session_log_update(path(), "", entry, true).is_err());
    }

    #[test]
    fn rejects_duplicate_summary() {
        let entry = "## 2026-07-29 — Runtime consolidation\n\n### What was done\n- Unified output generation.\n- Removed placeholder navigation.\n\n### Validation\n- Full tests passed.\n";
        let new_content = format!("{entry}\n{entry}");
        assert!(
            validate_team_memory_session_log_update(path(), entry, &new_content, true).is_err()
        );
    }

    #[test]
    fn accepts_concise_substantive_summary() {
        let entry = "## 2026-07-29 — Runtime consolidation\n\n### What was done\n- Unified output generation.\n- Removed placeholder navigation.\n\n### Validation\n- Full tests passed.\n";
        validate_team_memory_session_log_update(path(), "", entry, true).unwrap();
    }

    #[test]
    fn ignores_unrelated_files() {
        validate_team_memory_session_log_update(
            Path::new("notes.md"),
            "",
            "session_bad_1234567",
            false,
        )
        .unwrap();
    }
}
