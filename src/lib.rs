//! SigmaOS notebooks component — Jupyter notebook (.ipynb) handling.
//!
//! Parses notebook JSON into structured cells, extracts code/markdown,
//! and computes notebook statistics. Safe Rust, no unsafe code.
//! Uses only `serde_json`-free minimal parsing via manual JSON walk —
//! kept dependency-free for portability.

#![forbid(unsafe_code)]

use std::fmt;

/// Errors produced while handling notebooks.
#[derive(Debug)]
pub enum NotebookError {
    /// Input was not valid JSON.
    InvalidJson(String),
    /// Required notebook field missing.
    MissingField(&'static str),
    /// Cell structure malformed.
    MalformedCell(String),
}

impl fmt::Display for NotebookError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson(m) => write!(f, "invalid JSON: {m}"),
            Self::MissingField(name) => write!(f, "missing field: {}", name),
            Self::MalformedCell(m) => write!(f, "malformed cell: {m}"),
        }
    }
}

impl std::error::Error for NotebookError {}

/// Notebook cell kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    Code,
    Markdown,
    Raw,
}

/// A single notebook cell.
#[derive(Debug, Clone)]
pub struct Cell {
    /// Cell type.
    pub cell_type: CellType,
    /// Source lines (joined).
    pub source: String,
    /// Execution count for code cells.
    pub execution_count: Option<u64>,
}

/// Parsed notebook.
#[derive(Debug, Clone)]
pub struct Notebook {
    /// Notebook format version (nbformat).
    pub nbformat: u32,
    /// Minor version.
    pub nbformat_minor: u32,
    /// Cells in order.
    pub cells: Vec<Cell>,
}

impl Notebook {
    /// Count cells by type.
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut code = 0;
        let mut md = 0;
        let mut raw = 0;
        for c in &self.cells {
            match c.cell_type {
                CellType::Code => code += 1,
                CellType::Markdown => md += 1,
                CellType::Raw => raw += 1,
            }
        }
        (code, md, raw)
    }

    /// All code concatenated (for export/lint).
    pub fn code_source(&self) -> String {
        self.source_of(CellType::Code)
    }

    /// All cells of one type concatenated with blank-line separators.
    pub fn source_of(&self, cell_type: CellType) -> String {
        self.cells
            .iter()
            .filter(|c| c.cell_type == cell_type)
            .map(|c| c.source.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Total lines across all cells.
    pub fn line_count(&self) -> usize {
        self.cells.iter().map(|c| c.source.lines().count()).sum()
    }
}

/// Minimal JSON value for dependency-free parsing.
#[derive(Debug, Clone)]
pub enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn get(&self, key: &str) -> Option<&Json> {
        if let Json::Obj(pairs) = self {
            pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
        } else {
            None
        }
    }
    fn as_str(&self) -> Option<&str> {
        if let Json::Str(s) = self {
            Some(s)
        } else {
            None
        }
    }
    fn as_num(&self) -> Option<f64> {
        if let Json::Num(n) = self {
            Some(*n)
        } else {
            None
        }
    }
    fn as_arr(&self) -> Option<&[Json]> {
        if let Json::Arr(a) = self {
            Some(a)
        } else {
            None
        }
    }
}

/// Parse JSON text into our minimal Json value.
pub fn parse_json(input: &str) -> Result<Json, NotebookError> {
    let mut p = JsonParser {
        s: input.as_bytes(),
        i: 0,
    };
    p.skip_ws();
    let v = p.value()?;
    p.skip_ws();
    if p.i != p.s.len() {
        return Err(NotebookError::InvalidJson("trailing data".into()));
    }
    Ok(v)
}

struct JsonParser<'a> {
    s: &'a [u8],
    i: usize,
}

impl<'a> JsonParser<'a> {
    fn skip_ws(&mut self) {
        while self.i < self.s.len() && matches!(self.s[self.i], b' ' | b'\t' | b'\n' | b'\r') {
            self.i += 1;
        }
    }
    fn peek(&self) -> Option<u8> {
        self.s.get(self.i).copied()
    }
    fn value(&mut self) -> Result<Json, NotebookError> {
        self.skip_ws();
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => {
                self.lit("true")?;
                Ok(Json::Bool(true))
            }
            Some(b'f') => {
                self.lit("false")?;
                Ok(Json::Bool(false))
            }
            Some(b'n') => {
                self.lit("null")?;
                Ok(Json::Null)
            }
            Some(c) if c == b'-' || c.is_ascii_digit() => self.number(),
            _ => Err(NotebookError::InvalidJson(format!("at byte {}", self.i))),
        }
    }
    fn lit(&mut self, w: &str) -> Result<(), NotebookError> {
        if self.s[self.i..].starts_with(w.as_bytes()) {
            self.i += w.len();
            Ok(())
        } else {
            Err(NotebookError::InvalidJson("bad literal".into()))
        }
    }
    fn object(&mut self) -> Result<Json, NotebookError> {
        self.i += 1; // {
        let mut pairs = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b'}') {
            self.i += 1;
            return Ok(Json::Obj(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            if self.peek() != Some(b':') {
                return Err(NotebookError::InvalidJson("expected ':'".into()));
            }
            self.i += 1;
            let v = self.value()?;
            pairs.push((key, v));
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b'}') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(NotebookError::InvalidJson("expected ',' or '}'".into())),
            }
        }
        Ok(Json::Obj(pairs))
    }
    fn array(&mut self) -> Result<Json, NotebookError> {
        self.i += 1; // [
        let mut items = Vec::new();
        self.skip_ws();
        if self.peek() == Some(b']') {
            self.i += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.i += 1;
                }
                Some(b']') => {
                    self.i += 1;
                    break;
                }
                _ => return Err(NotebookError::InvalidJson("expected ',' or ']'".into())),
            }
        }
        Ok(Json::Arr(items))
    }
    fn string(&mut self) -> Result<String, NotebookError> {
        if self.peek() != Some(b'"') {
            return Err(NotebookError::InvalidJson("expected string".into()));
        }
        self.i += 1;
        let mut out = String::new();
        while let Some(c) = self.peek() {
            self.i += 1;
            match c {
                b'"' => return Ok(out),
                b'\\' => {
                    let e = self
                        .peek()
                        .ok_or_else(|| NotebookError::InvalidJson("bad escape".into()))?;
                    self.i += 1;
                    match e {
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'u' => {
                            if self.i + 4 > self.s.len() {
                                return Err(NotebookError::InvalidJson("bad \\u".into()));
                            }
                            let hex = std::str::from_utf8(&self.s[self.i..self.i + 4])
                                .map_err(|_| NotebookError::InvalidJson("bad \\u".into()))?;
                            let cp = u32::from_str_radix(hex, 16)
                                .map_err(|_| NotebookError::InvalidJson("bad \\u".into()))?;
                            self.i += 4;
                            out.push(char::from_u32(cp).unwrap_or('\u{FFFD}'));
                        }
                        _ => return Err(NotebookError::InvalidJson("bad escape".into())),
                    }
                }
                _ => {
                    // handle UTF-8 multibyte by re-decoding from bytes
                    let start = self.i - 1;
                    let len = utf8_len(c);
                    let end = (start + len).min(self.s.len());
                    if let Ok(s) = std::str::from_utf8(&self.s[start..end]) {
                        out.push_str(s);
                    }
                    self.i = end;
                }
            }
        }
        Err(NotebookError::InvalidJson("unterminated string".into()))
    }
    fn number(&mut self) -> Result<Json, NotebookError> {
        let start = self.i;
        if self.peek() == Some(b'-') {
            self.i += 1;
        }
        while matches!(self.peek(), Some(c) if c.is_ascii_digit() || c == b'.' || c == b'e' || c == b'E' || c == b'+' || c == b'-')
        {
            self.i += 1;
        }
        let s = std::str::from_utf8(&self.s[start..self.i])
            .map_err(|_| NotebookError::InvalidJson("bad number".into()))?;
        s.parse::<f64>()
            .map(Json::Num)
            .map_err(|_| NotebookError::InvalidJson("bad number".into()))
    }
}

fn utf8_len(b: u8) -> usize {
    if b < 0x80 {
        1
    } else if b >> 5 == 0b110 {
        2
    } else if b >> 4 == 0b1110 {
        3
    } else {
        4
    }
}

/// Parse an .ipynb file's JSON into a Notebook.
pub fn parse_notebook(json_text: &str) -> Result<Notebook, NotebookError> {
    let root = parse_json(json_text)?;
    let nbformat = root
        .get("nbformat")
        .and_then(Json::as_num)
        .ok_or(NotebookError::MissingField("nbformat"))? as u32;
    let nbformat_minor = root
        .get("nbformat_minor")
        .and_then(Json::as_num)
        .unwrap_or(0.0) as u32;
    let cells_json = root
        .get("cells")
        .and_then(Json::as_arr)
        .ok_or(NotebookError::MissingField("cells"))?;
    let mut cells = Vec::new();
    for cj in cells_json {
        let ct = cj.get("cell_type").and_then(Json::as_str).unwrap_or("");
        let cell_type = match ct {
            "code" => CellType::Code,
            "markdown" => CellType::Markdown,
            "raw" => CellType::Raw,
            _ => return Err(NotebookError::MalformedCell(format!("unknown type {ct}"))),
        };
        // source is string or array of strings
        let source = match cj.get("source") {
            Some(Json::Str(s)) => s.clone(),
            Some(Json::Arr(parts)) => parts
                .iter()
                .filter_map(Json::as_str)
                .collect::<Vec<_>>()
                .join(""),
            _ => String::new(),
        };
        let execution_count = cj
            .get("execution_count")
            .and_then(Json::as_num)
            .map(|n| n as u64);
        cells.push(Cell {
            cell_type,
            source,
            execution_count,
        });
    }
    Ok(Notebook {
        nbformat,
        nbformat_minor,
        cells,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_notebook() {
        let json = r##"{"nbformat":4,"nbformat_minor":5,"cells":[
            {"cell_type":"markdown","source":["# Title\n","text"]},
            {"cell_type":"code","execution_count":1,"source":"print(1)","outputs":[]}
        ]}"##;
        let nb = parse_notebook(json).unwrap();
        assert_eq!(nb.nbformat, 4);
        assert_eq!(nb.cells.len(), 2);
        assert_eq!(nb.counts(), (1, 1, 0));
        assert_eq!(nb.code_source(), "print(1)");
    }

    #[test]
    fn rejects_invalid() {
        assert!(parse_notebook("not json").is_err());
        assert!(parse_notebook("{}").is_err());
    }

    #[test]
    fn source_of_separates_cells() {
        let json = r##"{"nbformat":4,"nbformat_minor":5,"cells":[
            {"cell_type":"code","source":"a=1"},
            {"cell_type":"markdown","source":"# Hi"},
            {"cell_type":"code","source":"b=2"}
        ]}"##;
        let nb = parse_notebook(json).unwrap();
        assert_eq!(nb.code_source(), "a=1\n\nb=2");
        assert_eq!(nb.source_of(CellType::Markdown), "# Hi");
        assert_eq!(nb.source_of(CellType::Raw), "");
    }

    #[test]
    fn parses_escapes_and_unicode() {
        let json = r##"{"nbformat":4,"nbformat_minor":5,"cells":[
            {"cell_type":"code","source":"s=\"a\nb\té\""}
        ]}"##;
        let nb = parse_notebook(json).unwrap();
        assert_eq!(nb.code_source(), "s=\"a\nb\té\"");
    }

    #[test]
    fn test_parse_notebook_rejects_unknown_cell_type_aaa() {
        // Arrange
        let json =
            r#"{"nbformat":4,"nbformat_minor":0,"cells":[{"cell_type":"unknown","source":"x"}]}"#;

        // Act
        let res = parse_notebook(json);

        // Assert
        assert!(matches!(res, Err(NotebookError::MalformedCell(_))));
    }

    #[test]
    fn test_parse_notebook_counts_all_cell_types_and_lines_aaa() {
        // Arrange
        let json = r##"{"nbformat":4,"nbformat_minor":2,"cells":[
            {"cell_type":"code","execution_count":1,"source":"a = 1\nb = 2"},
            {"cell_type":"markdown","source":"# Header"},
            {"cell_type":"raw","source":"raw\ntext"}
        ]}"##;

        // Act
        let nb = parse_notebook(json).unwrap();

        // Assert
        assert_eq!(nb.counts(), (1, 1, 1));
        assert_eq!(nb.line_count(), 5);
        assert_eq!(nb.cells[0].execution_count, Some(1));
    }

    #[test]
    fn test_parse_json_handles_unicode_escape_and_trailing_data_aaa() {
        // Arrange
        let valid = r#"{"text":"\u0041\u0042"}"#;
        let trailing = r#"{"a":1} extra"#;

        // Act
        let parsed = parse_json(valid).unwrap();
        let err = parse_json(trailing);

        // Assert
        assert_eq!(parsed.get("text").and_then(Json::as_str), Some("AB"));
        assert!(matches!(err, Err(NotebookError::InvalidJson(_))));
    }

    #[test]
    fn test_parse_notebook_handles_empty_cells_and_null_exec_count_aaa() {
        // Arrange
        let json = r#"{"nbformat":4,"nbformat_minor":5,"cells":[{"cell_type":"code","execution_count":null,"source":[]}]}"#;

        // Act
        let nb = parse_notebook(json).unwrap();

        // Assert
        assert_eq!(nb.cells.len(), 1);
        assert_eq!(nb.cells[0].source, "");
        assert_eq!(nb.cells[0].execution_count, None);
    }
}
