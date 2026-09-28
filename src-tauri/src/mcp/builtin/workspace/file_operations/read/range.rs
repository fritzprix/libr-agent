//! Line-range resolution for workspace__readFile.

use super::types::{BINARY_FILE_ERROR_PHRASE, EMPTY_FILE_OUT_OF_RANGE_PREFIX};

pub(super) fn is_empty_file_out_of_range_error(message: &str) -> bool {
    message.starts_with(EMPTY_FILE_OUT_OF_RANGE_PREFIX)
}

pub(super) fn is_binary_file_error(message: &str) -> bool {
    message
        .to_ascii_lowercase()
        .contains(BINARY_FILE_ERROR_PHRASE)
}

pub(super) fn parse_offset_exceeds_error(message: &str) -> Option<(usize, usize)> {
    let prefix = "Requested offset ";
    let middle = " exceeds file length of ";
    let suffix = " lines";

    let after_prefix = message.strip_prefix(prefix)?;
    let (requested_line, after_requested) = after_prefix.split_once(middle)?;
    let total_lines = after_requested.strip_suffix(suffix)?;

    let req = requested_line.parse::<usize>().ok()?;
    let tot = total_lines.parse::<usize>().ok()?;
    Some((req, tot))
}

pub(super) fn resolve_range(
    total_lines: usize,
    offset_opt: Option<isize>,
    size_opt: Option<isize>,
) -> (usize, usize) {
    match size_opt {
        Some(sz) => {
            if sz < 0 {
                let count = sz.unsigned_abs();
                match offset_opt {
                    Some(off) => {
                        if off < 0 {
                            let skip = off.unsigned_abs();
                            let end = total_lines.saturating_sub(skip);
                            let start = end.saturating_sub(count) + 1;
                            (start, end)
                        } else {
                            let end = off as usize;
                            let start = end.saturating_sub(count) + 1;
                            (start, end)
                        }
                    }
                    None => {
                        let end = total_lines;
                        let start = end.saturating_sub(count) + 1;
                        (start, end)
                    }
                }
            } else {
                let count = sz as usize;
                match offset_opt {
                    Some(off) => {
                        if off < 0 {
                            let skip = off.unsigned_abs();
                            let start = total_lines.saturating_sub(skip) + 1;
                            let end = start + count - 1;
                            (start, end)
                        } else {
                            let start = if off == 0 { 1 } else { off as usize };
                            let end = start + count - 1;
                            (start, end)
                        }
                    }
                    None => {
                        let start = 1;
                        let end = start + count - 1;
                        (start, end)
                    }
                }
            }
        }
        None => match offset_opt {
            Some(off) => {
                if off < 0 {
                    let skip = off.unsigned_abs();
                    let start = total_lines.saturating_sub(skip) + 1;
                    let end = total_lines;
                    (start, end)
                } else {
                    let start = if off == 0 { 1 } else { off as usize };
                    let end = usize::MAX;
                    (start, end)
                }
            }
            None => (1, usize::MAX),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_binary_file_error() {
        // Embedded null bytes path (decode_file_bytes_to_lines)
        assert!(is_binary_file_error(
            "Failed to read file: content appears to be binary (embedded null bytes)"
        ));
        assert!(is_binary_file_error(
            "Failed to read file: content appears to be binary (embedded null bytes). Use a specialized tool or shell commands for binary files."
        ));

        // Invalid UTF-8 / InvalidData path
        assert!(is_binary_file_error(
            "Failed to read file: Content appears to be binary or contains invalid UTF-8 characters. Please use a specialized tool for binary files."
        ));

        // False positives prevention: path or error message containing "binary"
        assert!(!is_binary_file_error(
            "Failed to read file: /usr/local/binary/test.txt: No such file or directory"
        ));
        assert!(!is_binary_file_error("No such file or directory"));
        assert!(!is_binary_file_error("Permission denied"));
    }
}
