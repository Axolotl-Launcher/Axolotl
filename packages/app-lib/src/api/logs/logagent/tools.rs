use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeSet;

#[derive(Clone, Serialize)]
pub struct LogFile {
    pub id: String,
    pub name: String,
    pub source_type: String,
    pub first_line: usize,
    pub line_count: usize,
    pub truncated: bool,
    #[serde(skip)]
    pub lines: Vec<String>,
}

#[derive(Deserialize)]
pub struct ToolRequest {
    pub name: String,
    #[serde(default)]
    pub file_id: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub query: Option<String>,
    pub context: Option<usize>,
}

pub fn execute(files: &[LogFile], request: &ToolRequest) -> Value {
    if request.name == "list_log_files" {
        return json!(files);
    }
    let Some(file) = files.iter().find(|file| file.id == request.file_id)
    else {
        return json!({ "error": "Unknown file_id. Use list_log_files; filesystem paths are not accepted." });
    };
    let mut truncated;
    let selected: BTreeSet<usize> = match request.name.as_str() {
        "read_log_file" => {
            let start = request
                .start_line
                .unwrap_or(file.first_line)
                .max(file.first_line);
            let requested_end =
                request.end_line.unwrap_or(start.saturating_add(79));
            let end = requested_end.min(start.saturating_add(159));
            truncated = requested_end > end;
            (start.saturating_sub(file.first_line)
                ..=end.saturating_sub(file.first_line))
                .take(160)
                .filter(|index| *index < file.lines.len())
                .collect()
        }
        "grep_log_file" => {
            let query = request.query.as_deref().unwrap_or_default();
            if query.is_empty() || query.len() > 1024 {
                return json!({ "error": "Supply a nonempty literal query of at most 256 characters." });
            }
            let query = query.to_lowercase();
            let context = request.context.unwrap_or(3).min(10);
            let mut matches = file
                .lines
                .iter()
                .enumerate()
                .filter(|(_, line)| line.to_lowercase().contains(&query));
            let selected = matches
                .by_ref()
                .take(30)
                .flat_map(|(index, _)| {
                    index.saturating_sub(context)
                        ..=index
                            .saturating_add(context)
                            .min(file.lines.len().saturating_sub(1))
                })
                .collect();
            truncated = matches.next().is_some();
            selected
        }
        _ => return json!({ "error": "Unsupported diagnostic tool" }),
    };
    let mut remaining = 12_000usize;
    let mut lines = Vec::new();
    for index in selected {
        let text = &file.lines[index];
        let excerpt =
            text.chars().take(remaining.min(2000)).collect::<String>();
        remaining = remaining.saturating_sub(excerpt.chars().count());
        truncated |= excerpt.len() < text.len();
        lines.push(json!({ "line": file.first_line + index, "text": excerpt }));
        if remaining == 0 || lines.len() >= 160 {
            truncated = true;
            break;
        }
    }
    json!({ "file_id": file.id, "name": file.name, "first_line": file.first_line, "lines": lines, "truncated": truncated })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file() -> LogFile {
        LogFile {
            id: "log-1".into(),
            name: "latest.log".into(),
            source_type: "minecraft_log".into(),
            first_line: 200,
            line_count: 5,
            truncated: true,
            lines: ["start", "ERROR one", "cause", "error two", "end"]
                .map(str::to_string)
                .into(),
        }
    }

    #[test]
    fn grep_merges_context_and_preserves_original_lines() {
        let result = execute(
            &[file()],
            &ToolRequest {
                name: "grep_log_file".into(),
                file_id: "log-1".into(),
                start_line: None,
                end_line: None,
                query: Some("error".into()),
                context: Some(1),
            },
        );
        let lines = result["lines"].as_array().unwrap();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0]["line"], 200);
        assert_eq!(lines[4]["line"], 204);
    }

    #[test]
    fn rejects_paths_and_bounds_long_lines() {
        let mut request = ToolRequest {
            name: "read_log_file".into(),
            file_id: "../../secret".into(),
            start_line: Some(200),
            end_line: Some(usize::MAX),
            query: None,
            context: None,
        };
        assert!(execute(&[file()], &request).get("error").is_some());
        request.file_id = "log-1".into();
        let mut file = file();
        file.lines = vec!["字".repeat(20000)];
        let result = execute(&[file], &request);
        assert_eq!(
            result["lines"][0]["text"].as_str().unwrap().chars().count(),
            2000
        );
        assert_eq!(result["truncated"], true);
        request.start_line = Some(usize::MAX);
        assert_eq!(
            execute(&[self::file()], &request)["lines"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }
}
