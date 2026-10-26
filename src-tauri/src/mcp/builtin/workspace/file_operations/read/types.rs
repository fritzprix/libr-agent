//! Types and constants for workspace__readFile.

#[derive(Debug)]
pub(super) struct ReadFileChunk {
    pub(super) content: String,
    /// Total lines in the file (not just the displayed window).
    pub(super) total_lines: usize,
    pub(super) displayed_start_line: usize,
    pub(super) displayed_end_line: usize,
    pub(super) displayed_line_count: usize,
    pub(super) truncated: bool,
    pub(super) next_start_line: Option<usize>,
    pub(super) suggested_end_line: Option<usize>,
    /// True when the first line in the requested range was wider than the
    /// inline limit. The `content` field will still contain a hard byte-cut
    /// preview of that line rather than being empty.
    pub(super) next_line_too_large: bool,
    /// When `next_line_too_large` is true, this holds the number of Unicode
    /// scalar values (chars) already shown in the hard-cut preview so the
    /// agent can continue reading from the correct character offset within
    /// the same line.
    pub(super) hard_cut_chars_shown: usize,
    /// Tail reads only: last line of the earlier part of the range that was cut to fit
    /// the inline limit (the range start is `displayed_start_line` before the cut).
    pub(super) earlier_end_line: Option<usize>,
    /// Tail reads only: first line of the requested range, to bound the earlier part.
    pub(super) range_start_line: usize,
}

#[derive(Debug, Clone, Copy)]
pub enum ReadMode {
    Range { start: usize, end: usize },
    Tail(usize),
}

pub(super) const READ_FILE_BASE_HEADROOM_BYTES: usize = 1024;
pub(super) const READ_FILE_ANCHOR_HEADROOM_BYTES: usize = 2 * 1024;
pub(super) const READ_FILE_MIN_VISIBLE_CONTENT_BYTES: usize = 1024;
pub(super) const EMPTY_FILE_OUT_OF_RANGE_PREFIX: &str = "File is empty (0 lines);";
pub(super) const BINARY_FILE_ERROR_PHRASE: &str = "appears to be binary";
