//! Bounded views of already censored log documents. Offsets index the censored
//! UTF-8 document and must never be reused as offsets into raw log files.
use serde_json::{Value, json};

pub fn valid_filename(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && !name.ends_with([' ', '.'])
        && !name.chars().any(|c| {
            c.is_control()
                || matches!(
                    c,
                    '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*'
                )
        })
        && [".log", ".txt", ".log.gz", ".txt.gz"]
            .iter()
            .any(|ext| name.ends_with(ext))
}

pub fn read(
    text: &str,
    offset: usize,
    max_bytes: usize,
    source_truncated: bool,
) -> Value {
    let mut start = offset.min(text.len());
    while !text.is_char_boundary(start) {
        start += 1;
    }
    let mut end = start
        .saturating_add(max_bytes.clamp(4, 32768))
        .min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    json!({"text":&text[start..end],"offset":start,"next_offset":if end < text.len() {Some(end)} else {None},"total_bytes":text.len(),"source_truncated":source_truncated})
}

pub fn search(
    text: &str,
    query: &str,
    start_line: usize,
    limit: usize,
    source_truncated: bool,
) -> Value {
    let query = query.to_lowercase();
    let mut matches = Vec::new();
    let mut next_line = None;
    for (line, content) in text.lines().enumerate().skip(start_line) {
        let Some(position) = content.to_lowercase().find(&query) else {
            continue;
        };
        if matches.len() >= limit.clamp(1, 100) {
            next_line = Some(line);
            break;
        }
        // Bound each excerpt as well as match count: one log line may be huge.
        // Lowercasing can change UTF-8 byte length (for example İ -> i + dot).
        // Map the match back to the original line before selecting an excerpt.
        let mut lower_offset = 0;
        let matched = content
            .char_indices()
            .find_map(|(offset, c)| {
                lower_offset +=
                    c.to_lowercase().map(char::len_utf8).sum::<usize>();
                (lower_offset > position).then_some(offset)
            })
            .unwrap_or(0);
        let mut start = matched.saturating_sub(64);
        while !content.is_char_boundary(start) {
            start += 1;
        }
        let mut end = start.saturating_add(256).min(content.len());
        while !content.is_char_boundary(end) {
            end -= 1;
        }
        matches.push(json!({"line":line,"text":&content[start..end],"byte_offset":start,"truncated":start > 0 || end < content.len()}));
    }
    json!({"matches":matches,"next_line":next_line,"source_truncated":source_truncated})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filenames_cannot_escape_or_use_windows_streams() {
        for name in [
            "../secret.txt",
            "..\\secret.txt",
            "C:\\secret.log",
            "file.log:secret",
            ".log",
            "file.log.",
            "file.log ",
            "file\n.log",
            "file.json",
            "a/b.log",
        ] {
            assert!(!valid_filename(name), "{name}");
        }
        for name in [
            "latest.log",
            "2026-01-01.log.gz",
            "crash-测试.txt",
            "hs_err_pid123.log",
        ] {
            assert!(valid_filename(name));
        }
    }
    #[test]
    fn unicode_pages_progress_without_splitting_utf8() {
        let text = "首行\n🦎中文测试\nfinal";
        let mut offset = 0;
        let mut output = String::new();
        loop {
            let page = read(text, offset, 4, true);
            output.push_str(page["text"].as_str().unwrap());
            assert_eq!(page["source_truncated"], true);
            let Some(next) = page["next_offset"].as_u64() else {
                break;
            };
            assert!(next as usize > offset);
            offset = next as usize;
        }
        assert_eq!(output, text);
        assert_eq!(read(text, usize::MAX, 100, false)["text"], "");
        assert_eq!(read("中文", 1, 4, false)["text"], "文");
    }
    #[test]
    fn searches_are_literal_paginated_and_bounded() {
        let text =
            format!("Error first\nOK\nERROR {}\nError last", "a".repeat(10000));
        let first = search(&text, "error", 0, 2, true);
        assert_eq!(first["matches"].as_array().unwrap().len(), 2);
        assert_eq!(first["next_line"], 3);
        assert_eq!(first["matches"][1]["truncated"], true);
        assert!(first["matches"][1]["text"].as_str().unwrap().len() <= 256);
        let last = search(&text, "error", 3, 2, false);
        assert_eq!(last["matches"][0]["line"], 3);
        assert!(last["next_line"].is_null());
        assert!(
            search("plain", ".*", 0, 10, false)["matches"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let tail = search(
            &format!("{}İERROR", "界".repeat(1000)),
            "error",
            0,
            10,
            false,
        );
        assert!(
            tail["matches"][0]["text"]
                .as_str()
                .unwrap()
                .contains("ERROR")
        );
        assert!(tail["matches"][0]["byte_offset"].as_u64().unwrap() > 0);
    }
}
