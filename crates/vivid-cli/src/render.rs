//! Turning results into text: a plain table and hand-written JSON (the CLI has no JSON dependency).

use std::fmt::Write;

/// A table with a header row: columns left-aligned, separated by two spaces, no trailing spaces.
#[must_use]
pub fn table(header: &[&str], rows: &[Vec<String>]) -> String {
    let columns: usize = header.len();
    let mut widths: Vec<usize> = header.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.chars().count());
        }
    }
    let mut out: String = String::new();
    let mut line = |cells: &mut dyn Iterator<Item = &str>| {
        let mut text: String = String::new();
        for (index, (cell, width)) in cells.zip(&widths).enumerate().take(columns) {
            text.push_str(cell);
            if index + 1 < columns {
                text.push_str(&" ".repeat(width.saturating_sub(cell.chars().count()) + 2));
            }
        }
        out.push_str(text.trim_end());
        out.push('\n');
    };
    line(&mut header.iter().copied());
    for row in rows {
        line(&mut row.iter().map(String::as_str));
    }
    out
}

/// A JSON string literal, with the quotes.
#[must_use]
pub fn json_string(text: &str) -> String {
    let mut out: String = String::with_capacity(text.len() + 2);
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_table_aligns_columns_and_trims_the_end() {
        let rows: Vec<Vec<String>> = vec![
            vec!["nvkms".into(), "HDMI-0".into(), "connected".into(), "512".into()],
            vec!["nvkms".into(), "DP-0".into(), "disconnected".into(), "-".into()],
        ];
        let text: String = table(&["BACKEND", "CONNECTOR", "STATE", "VIBRANCE"], &rows);
        assert_eq!(
            text,
            "BACKEND  CONNECTOR  STATE         VIBRANCE\n\
             nvkms    HDMI-0     connected     512\n\
             nvkms    DP-0       disconnected  -\n"
        );
    }

    #[test]
    fn a_table_with_no_rows_is_just_the_header() {
        assert_eq!(table(&["A", "B"], &[]), "A  B\n");
    }

    #[test]
    fn wide_characters_do_not_break_alignment_by_bytes() {
        let rows: Vec<Vec<String>> = vec![vec!["é".into(), "x".into()]];
        assert_eq!(table(&["NAME", "V"], &rows), "NAME  V\né     x\n");
    }

    #[test]
    fn json_strings_are_escaped() {
        assert_eq!(json_string("plain"), "\"plain\"");
        assert_eq!(json_string("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(json_string("line\nbreak\ttab\r"), "\"line\\nbreak\\ttab\\r\"");
        assert_eq!(json_string("\u{1}"), "\"\\u0001\"");
        assert_eq!(json_string("café"), "\"café\"");
        assert_eq!(json_string(""), "\"\"");
    }
}
