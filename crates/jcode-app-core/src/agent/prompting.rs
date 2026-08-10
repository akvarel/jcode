use super::Agent;
use crate::logging;
use crate::message::{Message, ToolDefinition};

impl Agent {
    fn merge_pending_memory(
        pending: Option<crate::memory::PendingMemory>,
        current: Option<crate::memory::PendingMemory>,
    ) -> Option<crate::memory::PendingMemory> {
        match (pending, current) {
            (None, None) => None,
            (Some(memory), None) | (None, Some(memory)) => Some(memory),
            (Some(mut pending), Some(current)) => {
                pending.prompt.push_str("\n\n");
                pending.prompt.push_str(&current.prompt);
                pending.display_prompt = match (pending.display_prompt, current.display_prompt) {
                    (Some(mut left), Some(right)) => {
                        left.push_str("\n\n");
                        left.push_str(&right);
                        Some(left)
                    }
                    (left @ Some(_), None) => left,
                    (None, right) => right,
                };
                pending.count = pending.count.saturating_add(current.count);
                for id in current.memory_ids {
                    if !pending.memory_ids.contains(&id) {
                        pending.memory_ids.push(id);
                    }
                }
                Some(pending)
            }
        }
    }

    async fn graphify_memory_for_current_turn(
        &self,
        messages: &[Message],
    ) -> Option<crate::memory::PendingMemory> {
        let focused_query = crate::memory::format_focused_query_for_relevance(messages);
        let entries =
            crate::memory_external::graphify_context_for_current_turn(&focused_query).await;
        if entries.is_empty() {
            return None;
        }

        let prompt = crate::memory::format_relevant_prompt(&entries, entries.len())?;
        let display_prompt = crate::memory::format_relevant_display_prompt(&entries, entries.len());
        Some(crate::memory::PendingMemory {
            prompt,
            display_prompt,
            computed_at: std::time::Instant::now(),
            count: entries.len(),
            memory_ids: entries.into_iter().map(|entry| entry.id).collect(),
        })
    }

    pub(super) async fn build_memory_prompt_for_current_turn(
        &self,
        messages: std::sync::Arc<[Message]>,
        memory_event_tx: Option<crate::memory::MemoryEventSink>,
    ) -> Option<crate::memory::PendingMemory> {
        if !self.memory_enabled {
            return None;
        }

        let fresh_user_turn = crate::message::ends_with_fresh_user_turn(&messages);
        let pending = self.build_memory_prompt_nonblocking_shared(
            std::sync::Arc::clone(&messages),
            memory_event_tx,
        );
        if !fresh_user_turn {
            return pending;
        }

        let current = self.graphify_memory_for_current_turn(&messages).await;
        Self::merge_pending_memory(pending, current)
    }

    pub(super) fn log_prompt_prefix_accounting(
        &self,
        split: &crate::prompt::SplitSystemPrompt,
        tools: &[ToolDefinition],
    ) {
        let system_tokens = split.estimated_tokens();
        let tool_tokens = ToolDefinition::aggregate_prompt_token_estimate(tools);
        let prefix_tokens = system_tokens + tool_tokens;
        logging::info(&format!(
            "Prompt prefix estimate: total={} tokens (system={} tools={})",
            prefix_tokens, system_tokens, tool_tokens
        ));
    }

    pub(super) fn build_memory_prompt_nonblocking_shared(
        &self,
        messages: std::sync::Arc<[Message]>,
        _memory_event_tx: Option<crate::memory::MemoryEventSink>,
    ) -> Option<crate::memory::PendingMemory> {
        if !self.memory_enabled {
            return None;
        }

        let session_id = &self.session.id;

        let fresh_user_turn = crate::message::ends_with_fresh_user_turn(&messages);
        let pending = if fresh_user_turn {
            crate::memory::take_pending_memory(session_id)
        } else {
            None
        };

        // Use the persistent memory-agent pipeline as the single source of truth.
        // Running both this and the legacy MemoryManager background retrieval path
        // can prepare overlapping pending prompts for the same turn, which makes
        // memory injection feel overly aggressive.
        // Relevance results are consumed only at the start of a fresh user turn.
        // Enqueuing again after every tool result runs the local embedding model
        // for each provider continuation without creating an additional injection
        // opportunity. One update per user turn keeps memory current while avoiding
        // redundant 512-token inference during tool-heavy agent loops.
        if fresh_user_turn {
            crate::memory_agent::update_context_sync_with_dir(
                session_id,
                messages,
                self.session.working_dir.clone(),
            );
        }

        pending
    }

    fn append_current_turn_system_reminder(&self, split: &mut crate::prompt::SplitSystemPrompt) {
        let Some(reminder) = self
            .current_turn_system_reminder
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            return;
        };

        if !split.dynamic_part.is_empty() {
            split.dynamic_part.push_str("\n\n");
        }
        split.dynamic_part.push_str("# System Reminder\n\n");
        split.dynamic_part.push_str(reminder);
    }

    /// Build split system prompt for better caching
    /// Returns static (cacheable) and dynamic (not cached) parts separately
    pub(super) fn build_system_prompt_split(
        &self,
        memory_prompt: Option<&str>,
    ) -> crate::prompt::SplitSystemPrompt {
        if let Some(ref override_prompt) = self.system_prompt_override {
            return crate::prompt::SplitSystemPrompt {
                static_part: override_prompt.clone(),
                dynamic_part: String::new(),
            };
        }

        let skills = self.current_skills_snapshot();
        let skill_prompt = self
            .active_skill
            .as_ref()
            .and_then(|name| skills.get(name).map(|skill| skill.get_prompt().to_string()));

        let available_skills: Vec<crate::prompt::SkillInfo> = self
            .current_skills_snapshot()
            .list()
            .iter()
            .map(|skill| crate::prompt::SkillInfo {
                name: skill.name.clone(),
                description: skill.description.clone(),
            })
            .collect();

        let working_dir = self
            .session
            .working_dir
            .as_ref()
            .map(std::path::PathBuf::from);

        let (mut split, _context_info) = crate::prompt::build_system_prompt_split(
            skill_prompt.as_deref(),
            &available_skills,
            self.session.is_canary,
            memory_prompt,
            working_dir.as_deref(),
        );

        self.append_current_turn_system_reminder(&mut split);
        crate::prompt::append_swarm_effort_directive(
            &mut split,
            self.provider.reasoning_effort().as_deref(),
        );

        split
    }

    /// Non-blocking memory prompt - takes pending result and spawns check for next turn
    #[cfg(test)]
    pub(super) fn build_memory_prompt_nonblocking(
        &self,
        messages: &[Message],
        _memory_event_tx: Option<crate::memory::MemoryEventSink>,
    ) -> Option<crate::memory::PendingMemory> {
        self.build_memory_prompt_nonblocking_shared(messages.to_vec().into(), _memory_event_tx)
    }
}

#[cfg(test)]
mod tests {
    use super::Agent;
    use crate::memory::PendingMemory;
    use std::time::Instant;

    fn pending(prompt: &str, ids: &[&str]) -> PendingMemory {
        PendingMemory {
            prompt: prompt.to_string(),
            display_prompt: Some(format!("display:{prompt}")),
            computed_at: Instant::now(),
            count: ids.len(),
            memory_ids: ids.iter().map(|id| (*id).to_string()).collect(),
        }
    }

    #[test]
    fn merge_pending_memory_combines_async_and_current_turn_context() {
        let merged = Agent::merge_pending_memory(
            Some(pending("personal", &["memory-1"])),
            Some(pending("graph", &["graph-1"])),
        )
        .expect("merged memory");

        assert_eq!(merged.prompt, "personal\n\ngraph");
        assert_eq!(
            merged.display_prompt.as_deref(),
            Some("display:personal\n\ndisplay:graph")
        );
        assert_eq!(merged.count, 2);
        assert_eq!(merged.memory_ids, ["memory-1", "graph-1"]);
    }

    #[test]
    fn merge_pending_memory_deduplicates_memory_ids() {
        let merged = Agent::merge_pending_memory(
            Some(pending("older", &["shared"])),
            Some(pending("current", &["shared"])),
        )
        .expect("merged memory");

        assert_eq!(merged.memory_ids, ["shared"]);
        assert_eq!(merged.count, 2);
    }
}
