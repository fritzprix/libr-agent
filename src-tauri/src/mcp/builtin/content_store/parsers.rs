use calamine::{open_workbook, Reader, Xlsx};
use docx_rs::*;
use lopdf::Document;
use std::path::Path;
use tokio::fs;

/// Result type for document parsing operations
#[derive(Debug)]
pub enum ParseResult {
    Text(String),
    Error(String),
}

/// Main document parser that handles different file formats
pub struct DocumentParser;

impl DocumentParser {
    /// Parse a file based on its MIME type
    pub async fn parse_file(file_path: &Path, mime_type: &str) -> ParseResult {
        match mime_type {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => {
                Self::parse_docx(file_path).await
            }
            "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => {
                Self::parse_xlsx(file_path).await
            }
            "application/pdf" => Self::parse_pdf(file_path).await,
            "text/plain" | "text/markdown" | "text/csv" => Self::parse_text(file_path).await,
            _ => ParseResult::Error(format!("Unsupported MIME type: {mime_type}")),
        }
    }

    /// Parse DOCX files using docx-rs
    async fn parse_docx(file_path: &Path) -> ParseResult {
        match std::fs::read(file_path) {
            Ok(data) => {
                match docx_rs::read_docx(&data) {
                    Ok(docx) => {
                        let mut content = String::new();

                        // Extract text from document body
                        for child in &docx.document.children {
                            Self::extract_text_from_element(child, &mut content);
                        }

                        if content.is_empty() {
                            ParseResult::Error("No text content found in DOCX file".to_string())
                        } else {
                            ParseResult::Text(content.trim().to_string())
                        }
                    }
                    Err(e) => ParseResult::Error(format!("Failed to parse DOCX: {e}")),
                }
            }
            Err(e) => ParseResult::Error(format!("Failed to read DOCX file: {e}")),
        }
    }

    /// Extract text from DOCX elements recursively
    fn extract_text_from_element(element: &DocumentChild, content: &mut String) {
        match element {
            DocumentChild::Paragraph(paragraph) => {
                for child in &paragraph.children {
                    Self::extract_text_from_paragraph_child(child, content);
                }
                content.push('\n');
            }
            DocumentChild::Table(table) => {
                Self::extract_text_from_table(table, content);
            }
            // Handle other document elements by ignoring them for now
            _ => {}
        }
    }

    /// Extract text from paragraph children
    fn extract_text_from_paragraph_child(child: &ParagraphChild, content: &mut String) {
        if let ParagraphChild::Run(run) = child {
            for run_child in &run.children {
                if let RunChild::Text(text) = run_child {
                    content.push_str(&text.text);
                }
            }
        }
    }

    /// Extract text from DOCX tables and convert to markdown format
    fn extract_text_from_table(table: &Table, content: &mut String) {
        // Build a 2D vector of cell texts
        let mut rows_text: Vec<Vec<String>> = Vec::new();

        for row_child in &table.rows {
            let TableChild::TableRow(row) = row_child;
            let mut row_cells: Vec<String> = Vec::new();
            for cell_child in &row.cells {
                let TableRowChild::TableCell(cell) = cell_child;
                let mut cell_text = String::new();
                for cell_content in &cell.children {
                    match cell_content {
                        TableCellContent::Paragraph(p) => {
                            let mut p_text = String::new();
                            for ch in &p.children {
                                Self::extract_text_from_paragraph_child(ch, &mut p_text);
                            }
                            let p_text = p_text.trim();
                            if !p_text.is_empty() {
                                if !cell_text.is_empty() {
                                    cell_text.push('\n');
                                }
                                cell_text.push_str(p_text);
                            }
                        }
                        TableCellContent::Table(inner) => {
                            // Represent nested tables succinctly to avoid overly long output
                            let r = inner.rows.len();
                            if !cell_text.is_empty() {
                                cell_text.push('\n');
                            }
                            cell_text.push_str(&format!("[Nested table: {r} rows]"));
                        }
                        TableCellContent::StructuredDataTag(_sdt) => {
                            // Skip SDT content for now
                        }
                        TableCellContent::TableOfContents(_toc) => {
                            // Skip TOC entries in cells
                        }
                    }
                }
                row_cells.push(cell_text.trim().to_string());
            }

            if !row_cells.is_empty() {
                rows_text.push(row_cells);
            }
        }

        if rows_text.is_empty() {
            content.push_str("\n[Empty table]\n\n");
            return;
        }

        // Determine column count for formatting
        let cols = rows_text.iter().map(|r| r.len()).max().unwrap_or(0);
        if cols == 0 {
            content.push('\n');
            content.push_str("[Empty table]\n\n");
            return;
        }

        // Emit Markdown table with the first row as header
        content.push('\n');

        // Header row
        content.push('|');
        for i in 0..cols {
            let cell = rows_text[0].get(i).map(String::as_str).unwrap_or("");
            content.push(' ');
            content.push_str(cell);
            content.push(' ');
            content.push('|');
        }
        content.push('\n');

        // Separator row
        content.push('|');
        for _ in 0..cols {
            content.push_str(" --- |");
        }
        content.push('\n');

        // Body rows
        for row in rows_text.iter().skip(1) {
            content.push('|');
            for i in 0..cols {
                let cell = row.get(i).map(String::as_str).unwrap_or("");
                content.push(' ');
                content.push_str(cell);
                content.push(' ');
                content.push('|');
            }
            content.push('\n');
        }
        content.push('\n');
        content.push('\n');
    }

    /// Parse XLSX files using calamine
    async fn parse_xlsx(file_path: &Path) -> ParseResult {
        match open_workbook::<Xlsx<_>, &Path>(file_path) {
            Ok(mut workbook) => {
                let mut content = String::new();

                // Process each worksheet
                for sheet_name in workbook.sheet_names().clone() {
                    if let Ok(range) = workbook.worksheet_range(&sheet_name) {
                        content.push_str(&format!("Sheet: {sheet_name}\n"));

                        // Extract cell data
                        for row in range.rows() {
                            let row_data: Vec<String> = row
                                .iter()
                                .map(|cell| match cell {
                                    calamine::Data::String(s) => s.clone(),
                                    calamine::Data::Float(f) => f.to_string(),
                                    calamine::Data::Int(i) => i.to_string(),
                                    calamine::Data::Bool(b) => b.to_string(),
                                    calamine::Data::DateTime(d) => d.to_string(),
                                    calamine::Data::DateTimeIso(s) => s.clone(),
                                    calamine::Data::DurationIso(s) => s.clone(),
                                    calamine::Data::Empty => String::new(),
                                    calamine::Data::Error(e) => format!("Error: {e:?}"),
                                })
                                .collect();

                            if !row_data.is_empty() {
                                content.push_str(&row_data.join("\t"));
                                content.push('\n');
                            }
                        }
                        content.push('\n');
                    }
                }

                if content.is_empty() {
                    ParseResult::Error("No content found in XLSX file".to_string())
                } else {
                    ParseResult::Text(content.trim().to_string())
                }
            }
            Err(e) => ParseResult::Error(format!("Failed to parse XLSX: {e}")),
        }
    }

    /// Parse PDF files using pdf-extract (primary) with lopdf fallback
    async fn parse_pdf(file_path: &Path) -> ParseResult {
        // First try pdf-extract for better text extraction
        match pdf_extract::extract_text(file_path) {
            Ok(extracted_text) => {
                let content = extracted_text.trim();
                if !content.is_empty() {
                    return ParseResult::Text(content.to_string());
                }
                // If pdf-extract returns empty content, fall back to lopdf
            }
            Err(e) => {
                log::warn!("pdf-extract failed: {e}, falling back to lopdf");
                // Fall back to lopdf
            }
        }

        // Fallback to lopdf implementation
        match Document::load(file_path) {
            Ok(doc) => {
                let mut content = String::new();
                let mut page_count = 0;

                // Get all page object IDs
                let pages = doc.get_pages();

                // Extract text from each page
                for (page_num, &page_id) in pages.iter() {
                    page_count += 1;
                    if let Ok(page_content) = doc.get_page_content(page_id) {
                        // Try to extract text from page content with better parsing
                        if let Ok(text) = String::from_utf8(page_content) {
                            let mut page_text = String::new();

                            // Split content into operations
                            let operations: Vec<&str> = text.split_whitespace().collect();

                            let mut i = 0;
                            while i < operations.len() {
                                let op = operations[i];

                                // Look for text showing operators
                                if op == "Tj" || op == "TJ" || op == "'" || op == "\"" {
                                    // Try to extract text from previous operations
                                    let mut text_parts = Vec::new();

                                    // Look backwards for text content (between parentheses or angle brackets)
                                    let mut j = i.saturating_sub(1);
                                    while j > 0 && text_parts.len() < 5 {
                                        // Limit search to avoid infinite loops
                                        let prev_op = operations[j];

                                        // Check for string literals (enclosed in parentheses)
                                        if prev_op.starts_with('(') && prev_op.ends_with(')') {
                                            let text_content =
                                                &prev_op[1..prev_op.len().saturating_sub(1)];
                                            if !text_content.is_empty() {
                                                // Try to decode PDF string (basic handling)
                                                let decoded = Self::decode_pdf_string(text_content);
                                                if !decoded.is_empty() {
                                                    text_parts.push(decoded);
                                                }
                                            }
                                        }
                                        // Check for hex strings (enclosed in angle brackets)
                                        else if prev_op.starts_with('<') && prev_op.ends_with('>')
                                        {
                                            let hex_content =
                                                &prev_op[1..prev_op.len().saturating_sub(1)];
                                            if !hex_content.is_empty() {
                                                if let Ok(decoded) =
                                                    Self::decode_hex_string(hex_content)
                                                {
                                                    if !decoded.is_empty() {
                                                        text_parts.push(decoded);
                                                    }
                                                }
                                            }
                                        }

                                        j = j.saturating_sub(1);
                                    }

                                    // Add extracted text parts
                                    for part in text_parts.into_iter().rev() {
                                        if !page_text.contains(&part) {
                                            // Avoid duplicates
                                            page_text.push_str(&part);
                                            page_text.push(' ');
                                        }
                                    }
                                }

                                i += 1;
                            }

                            // Clean up page text
                            let page_text = page_text.trim();
                            if !page_text.is_empty() {
                                content.push_str(&format!("\n=== Page {} ===\n", page_num + 1));
                                content.push_str(page_text);
                                content.push('\n');
                            }
                        }
                    }
                }

                // Final cleanup
                let content = content.trim();
                if content.is_empty() {
                    ParseResult::Error(format!(
                        "No text content found in PDF file ({page_count} pages processed)"
                    ))
                } else {
                    ParseResult::Text(content.to_string())
                }
            }
            Err(e) => ParseResult::Error(format!("Failed to parse PDF: {e}")),
        }
    }

    /// Decode PDF string literals (basic implementation)
    fn decode_pdf_string(input: &str) -> String {
        let mut result = String::new();
        let mut chars = input.chars().peekable();

        while let Some(ch) = chars.next() {
            match ch {
                '\\' => {
                    // Handle escape sequences
                    match chars.next() {
                        Some('n') => result.push('\n'),
                        Some('r') => result.push('\r'),
                        Some('t') => result.push('\t'),
                        Some('b') => result.push('\x08'),
                        Some('f') => result.push('\x0c'),
                        Some('(') => result.push('('),
                        Some(')') => result.push(')'),
                        Some('\\') => result.push('\\'),
                        Some(ch @ '0'..='7') => {
                            // Octal escape sequence (up to 3 digits)
                            let mut octal = String::new();
                            octal.push(ch);
                            for _ in 0..2 {
                                if let Some(ch @ '0'..='7') = chars.peek() {
                                    octal.push(*ch);
                                    chars.next();
                                } else {
                                    break;
                                }
                            }
                            if let Ok(code) = u8::from_str_radix(&octal, 8) {
                                result.push(code as char);
                            }
                        }
                        Some(other) => result.push(other),
                        None => {}
                    }
                }
                _ => result.push(ch),
            }
        }

        result
    }

    /// Decode hex string to UTF-8
    fn decode_hex_string(hex: &str) -> Result<String, std::string::FromUtf8Error> {
        let mut bytes = Vec::new();
        let mut i = 0;
        let chars: Vec<char> = hex.chars().collect();

        while i < chars.len() {
            if chars[i].is_whitespace() {
                i += 1;
                continue;
            }

            if i + 1 < chars.len() {
                let byte_str = format!("{}{}", chars[i], chars[i + 1]);
                if let Ok(byte) = u8::from_str_radix(&byte_str, 16) {
                    bytes.push(byte);
                }
                i += 2;
            } else {
                break;
            }
        }

        String::from_utf8(bytes)
    }

    /// Parse plain text files
    async fn parse_text(file_path: &Path) -> ParseResult {
        match fs::read_to_string(file_path).await {
            Ok(content) => {
                if content.is_empty() {
                    ParseResult::Error("Text file is empty".to_string())
                } else {
                    ParseResult::Text(content)
                }
            }
            Err(e) => ParseResult::Error(format!("Failed to read text file: {e}")),
        }
    }

    /// Get file size for validation
    #[allow(dead_code)]
    pub async fn get_file_size(file_path: &Path) -> Result<u64, String> {
        match fs::metadata(file_path).await {
            Ok(metadata) => Ok(metadata.len()),
            Err(e) => Err(format!("Failed to get file metadata: {e}")),
        }
    }

    /// Validate file before parsing
    #[allow(dead_code)]
    pub async fn validate_file(file_path: &Path, max_size_mb: u64) -> Result<(), String> {
        let max_size_bytes = max_size_mb * 1024 * 1024;

        match Self::get_file_size(file_path).await {
            Ok(size) => {
                if size > max_size_bytes {
                    let size_mb = size / (1024 * 1024);
                    return Err(format!(
                        "File size {size_mb}MB exceeds maximum allowed size {max_size_mb}MB"
                    ));
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    #[tokio::test]
    async fn test_text_file_parsing() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        let test_content = "Hello, World!\nSecond line.";

        fs::write(&file_path, test_content).await.unwrap();

        let result = DocumentParser::parse_file(&file_path, "text/plain").await;
        match result {
            ParseResult::Text(content) => {
                assert_eq!(content, test_content);
            }
            ParseResult::Error(e) => panic!("Parsing failed: {}", e),
        }
    }

    #[tokio::test]
    async fn test_file_validation() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        // Create a small file
        fs::write(&file_path, "test").await.unwrap();

        // Should pass validation for 1MB limit
        assert!(DocumentParser::validate_file(&file_path, 1).await.is_ok());
    }

    #[tokio::test]
    async fn test_unsupported_mime_type() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.unknown");

        fs::write(&file_path, "test").await.unwrap();

        let result = DocumentParser::parse_file(&file_path, "application/unknown").await;
        match result {
            ParseResult::Error(msg) => {
                assert!(msg.contains("Unsupported MIME type"));
            }
            _ => panic!("Expected error for unsupported MIME type"),
        }
    }
}
