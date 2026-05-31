fn normalize_compaction_artifact_line(line: &str) -> String {
    line.replace("```", " ").trim().to_string()
}

fn extract_known_wrapper_tag_name(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    if !(trimmed.starts_with('<') && trimmed.ends_with('>')) {
        return None;
    }

    let inner = trimmed[1..trimmed.len().saturating_sub(1)].trim();
    let inner = inner.strip_prefix('/').unwrap_or(inner);
    let tag_name = inner
        .trim_end_matches('/')
        .split_whitespace()
        .next()
        .unwrap_or_default();

    if tag_name.is_empty()
        || !tag_name
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '_' | '-'))
    {
        return None;
    }

    Some(tag_name)
}

fn extract_wrapper_block_tag_start(line: &str) -> Option<&str> {
    let normalized = normalize_compaction_artifact_line(line);
    let tag_name = match extract_known_wrapper_tag_name(&normalized)? {
        "current_datetime" => "current_datetime",
        "system_reminder" => "system_reminder",
        "system_notification" => "system_notification",
        "sql_tables" => "sql_tables",
        _ => return None,
    };

    let is_closing_tag = normalized.starts_with("</");
    if is_closing_tag {
        return None;
    }

    Some(tag_name)
}

fn contains_wrapper_block_end(line: &str, tag_name: &str) -> bool {
    let normalized = normalize_compaction_artifact_line(line);
    normalized.contains(&format!("</{}>", tag_name))
}

pub fn is_compaction_artifact_line(line: &str) -> bool {
    let normalized = normalize_compaction_artifact_line(line);
    if normalized.is_empty() {
        return true;
    }

    if normalized.starts_with("Earlier:") || normalized.starts_with("Latest included:") {
        return true;
    }

    if matches!(
        normalized.as_str(),
        "Context compacted"
            | "컨텍스트가 위에서 압축됨"
            | "Summary"
            | "요약"
            | "### Previous Conversation Summary"
            | "### Recent Tool Call Snapshot (latest 5)"
    ) {
        return true;
    }

    if let Some(prefix) = normalized.strip_suffix("messages condensed") {
        if prefix.trim().parse::<usize>().is_ok() {
            return true;
        }
    }

    if normalized.starts_with("<current_datetime>") && normalized.contains("</current_datetime>") {
        return true;
    }
    if normalized.starts_with("<sql_tables>") && normalized.contains("</sql_tables>") {
        return true;
    }
    if normalized == "<system_reminder>"
        || normalized == "</system_reminder>"
        || normalized == "<system_notification>"
        || normalized == "</system_notification>"
    {
        return true;
    }

    matches!(
        extract_known_wrapper_tag_name(&normalized),
        Some("current_datetime" | "system_reminder" | "system_notification" | "sql_tables")
    )
}

pub fn sanitize_compaction_semantic_text(text: &str) -> String {
    let mut sanitized = Vec::new();
    let mut active_wrapper_tag: Option<&str> = None;

    for raw_line in text.lines() {
        if let Some(tag_name) = active_wrapper_tag {
            if contains_wrapper_block_end(raw_line, tag_name) {
                active_wrapper_tag = None;
            }
            continue;
        }

        if let Some(tag_name) = extract_wrapper_block_tag_start(raw_line) {
            if !contains_wrapper_block_end(raw_line, tag_name) {
                active_wrapper_tag = Some(tag_name);
            }
            continue;
        }

        let normalized = normalize_compaction_artifact_line(raw_line);
        if !is_compaction_artifact_line(&normalized) {
            sanitized.push(normalized);
        }
    }

    sanitized.join("\n")
}
