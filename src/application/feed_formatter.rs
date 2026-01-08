use super::feed::FeedManager;
use chrono::Local;

pub struct FeedFormatter;

impl FeedFormatter {
    pub fn format_active(manager: &FeedManager) -> String {
        let mut content = String::from("**🚀 Thinking & doing...**\n");
        if let Some(task) = &manager.current_task {
            // Only show first line, no label
            let summary = task.lines().next().unwrap_or(task);
            content.push_str(&format!("{}\n", summary));
        }

        let total = manager.entries.len();
        let start_index = total.saturating_sub(10);
        let slice = &manager.entries[start_index..];
        
        for (i, entry) in slice.iter().enumerate() {
            if entry.kind == super::feed::FeedEntryKind::Activity || entry.kind == super::feed::FeedEntryKind::Checkpoint {
                // is_last here means "is the very last item in the entire feed"
                // The item index in full list is start_index + i
                let is_last = (start_index + i) == (total - 1);
                content.push_str(&entry.format_active(is_last));
            }
        }
        
        // Always show the last thought if present
        if let Some(thought) = &manager.last_agent_thought {
             content.push_str(&format!("\n> {}\n", thought.replace('\n', "\n> ")));
        }

        content
    }

    pub fn format_squashed(manager: &FeedManager) -> String {
        let mut content = String::from("**🚀 Task Complete**\n");
        if let Some(task) = &manager.current_task {
            content.push_str(&format!("{}\n", task));
        }

        // Limit detail to prevent M_TOO_LARGE
        // Strategy: 
        // 1. Always show completion message if present.
        // 2. Add entries until we approach limit (safe: 32KB).
        // 3. If entries exceed, show first 10, skip middle, show last 20.
        
        // We build a temporary buffer for entries
        let mut entries_buffer = String::new();
        for entry in &manager.entries {
            entries_buffer.push_str(&entry.format_squashed());
        }

        // Check size (conservative 30000 chars)
        if entries_buffer.len() > 3000 || manager.entries.len() > 10 {
            let formatted_entries: Vec<String> = manager.entries.iter()
                .map(|e| e.format_squashed()).collect();
            
            if formatted_entries.len() > 10 {
                // Keep only the last 10
                for s in &formatted_entries[formatted_entries.len()-10..] {
                    content.push_str(s);
                }
            } else {
                 content.push_str(&entries_buffer);
            }
        } else {
            content.push_str(&entries_buffer);
        }

        if let Some(msg) = &manager.completion_message {
            content.push_str(&format!("\n{}\n", msg));
        }

        content
    }

    pub fn format_final(manager: &FeedManager) -> String {
        let mut content = String::from("**✅ Execution Complete**\n\n");
        if let Some(task) = &manager.current_task {
            content.push_str(&format!("{}\n\n", task));
        }
        content.push_str("**Summary**:\n");
        for entry in &manager.entries {
            if entry.kind == super::feed::FeedEntryKind::Checkpoint {
                if entry.label == "Completed" {
                     content.push_str(&format!("• {}\n", entry.content));
                } else {
                     content.push_str(&format!("• {}: {}\n", entry.label, entry.content));
                }
            }
        }
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
        content.push_str(&format!("\n*Generated at {}*", timestamp));
        content
    }

    pub fn format_wizard(manager: &FeedManager) -> String {
        let mut content = format!("**{}**\n\n", manager.title);

        for entry in &manager.entries {
            content.push_str(&entry.format_active(false)); // Wizard items usually static or just list
        }
        content
    }

    pub fn format_conversational(manager: &FeedManager) -> String {
        let mut content = String::new();

        // Check if we have meaningful activity (ignore just "Task Started")
        let has_real_activity = manager
            .recent_activities
            .iter()
            .any(|a| !a.contains("Task Started"));

        if has_real_activity {
            content.push_str("**Activity**:\n");
            for activity in &manager.recent_activities {
                content.push_str(&format!("{}\n", activity));
            }
            content.push_str("\n---\n");
        }

        if let Some(msg) = &manager.completion_message {
            content.push_str(msg);
        } else {
            content.push_str("Thinking...");
        }

        content
    }

    pub fn format_plan_review(manager: &FeedManager) -> String {
        let mut content = String::new();
        // Maybe header?
        content.push_str("**📋 Plan Generated**\n\n");

        if let Some(r) = &manager.roadmap_content {
            content.push_str("### Roadmap\n");
            content.push_str(r);
            content.push_str("\n\n");
        }

        if let Some(p) = &manager.plan_content {
            if manager.roadmap_content.is_some() {
                content.push_str("---\n\n");
            }
            content.push_str("### Implementation Plan\n");
            content.push_str(p);
            content.push_str("\n\n");
        }

        content.push_str("✅ **Plan Generated**: Type `.start` to proceed or `.ask` to refine.");
        content
    }
}
