use super::tools::{LogFile, ToolRequest, execute};

/// Focus the first window on the cause and nearby stack; tools can expand it.
pub fn initial_window(files: &[LogFile]) -> String {
    let mut ordered = files.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|file| match file.source_type.as_str() {
        "crash_report" => 0,
        "jvm_crash" => 1,
        _ => 2,
    });
    let mut sections = Vec::new();
    let mut budget = 12_000usize;
    for file in ordered {
        if budget == 0 {
            break;
        }
        let focus = file
            .lines
            .iter()
            .position(|line| {
                let line = line.to_ascii_lowercase();
                [
                    "caused by:",
                    "exception",
                    "fatal",
                    "error",
                    "problematic frame",
                ]
                .iter()
                .any(|needle| line.contains(needle))
            })
            .unwrap_or_else(|| file.lines.len().saturating_sub(50));
        let start = file.first_line + focus.saturating_sub(12);
        let value = execute(
            std::slice::from_ref(file),
            &ToolRequest {
                name: "read_log_file".into(),
                file_id: file.id.clone(),
                start_line: Some(start),
                end_line: Some(start + 79),
                query: None,
                context: None,
            },
        );
        let text = value.to_string();
        if text.chars().count() > budget {
            break;
        }
        budget = budget.saturating_sub(text.chars().count());
        sections.push(text);
    }
    sections.join("\n")
}
