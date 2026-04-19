#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CssRange {
    pub start_line: u32,
    pub start_column: u32,
    pub end_line: u32,
    pub end_column: u32,
}

impl CssRange {
    pub const fn whole_document() -> Self {
        Self { start_line: 1, start_column: 1, end_line: u32::MAX, end_column: u32::MAX }
    }
}

pub fn leading_trimmed_len(input: &str) -> usize {
    input.len() - input.trim_start().len()
}

pub fn trailing_trimmed_len(input: &str) -> usize {
    input.len() - input.trim_end().len()
}

pub struct SourceMap<'a> {
    source: &'a str,
    line_starts: Vec<usize>,
}

impl<'a> SourceMap<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (offset, ch) in source.char_indices() {
            if ch == '\n' {
                line_starts.push(offset + 1);
            }
        }
        Self { source, line_starts }
    }

    pub fn range(&self, start: usize, end: usize) -> CssRange {
        let (start_line, start_column) = self.position(start);
        let (end_line, end_column) = self.position(end);
        CssRange { start_line, start_column, end_line, end_column }
    }

    pub fn position(&self, offset: usize) -> (u32, u32) {
        let line_index = match self.line_starts.binary_search(&offset) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };
        let line_start = self.line_starts[line_index];
        let column = self.source[line_start..offset].chars().count() as u32 + 1;
        (line_index as u32 + 1, column)
    }
}
