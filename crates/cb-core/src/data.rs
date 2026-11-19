//! Data for batch cards: spreadsheets saved as CSV / TSV, and `{Column}` placeholders.

use crate::doc::DataSet;

/// Read CSV or TSV text (as saved by Excel, LibreOffice or Google Sheets): the first row names
/// the columns. The separator (comma, semicolon, tab or bar) is detected; quoted fields may
/// contain separators, line breaks and doubled quotes. Empty rows are skipped.
pub fn parse_csv(text: &str) -> Result<DataSet, String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let sep = detect_separator(text);
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let mut at_field_start = true;
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if at_field_start => {
                quoted = true;
                at_field_start = false;
            }
            c if c == sep => {
                row.push(std::mem::take(&mut field));
                at_field_start = true;
            }
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
                at_field_start = true;
            }
            c => {
                field.push(c);
                at_field_start = false;
            }
        }
    }
    if quoted {
        return Err("a quoted field is never closed".into());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows.retain(|r| r.iter().any(|f| !f.trim().is_empty()));
    let mut it = rows.into_iter();
    let header = it.next().ok_or("the file is empty")?;
    let mut columns: Vec<String> = Vec::new();
    for (i, h) in header.iter().enumerate() {
        let mut name = h.trim().to_string();
        if name.is_empty() {
            name = format!("Column {}", i + 1);
        }
        // keep names unique so every placeholder means one column
        let base = name.clone();
        let mut n = 2;
        while columns.iter().any(|c| c.eq_ignore_ascii_case(&name)) {
            name = format!("{base} {n}");
            n += 1;
        }
        columns.push(name);
    }
    let mut rows: Vec<Vec<String>> = it.collect();
    let width = columns.len().max(rows.iter().map(Vec::len).max().unwrap_or(0));
    while columns.len() < width {
        columns.push(format!("Column {}", columns.len() + 1));
    }
    for r in &mut rows {
        r.resize(width, String::new());
    }
    Ok(DataSet { name: String::new(), columns, rows })
}

/// the separator used in the first line (outside quotes)
fn detect_separator(text: &str) -> char {
    let first = text.lines().next().unwrap_or("");
    let mut counts = [(',', 0), (';', 0), ('\t', 0), ('|', 0)];
    let mut quoted = false;
    for c in first.chars() {
        if c == '"' {
            quoted = !quoted;
        } else if !quoted {
            for (s, n) in counts.iter_mut() {
                if c == *s {
                    *n += 1;
                }
            }
        }
    }
    counts.iter().max_by_key(|(_, n)| *n).filter(|(_, n)| *n > 0).map(|(s, _)| *s).unwrap_or(',')
}

/// data as CSV (comma separated, quoted where needed)
pub fn to_csv(data: &DataSet) -> String {
    let q = |s: &str| {
        if s.contains([',', '"', '\n', '\r']) || s.starts_with(' ') || s.ends_with(' ') {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    };
    let mut out = data.columns.iter().map(|c| q(c)).collect::<Vec<_>>().join(",");
    out.push('\n');
    for r in &data.rows {
        out += &r.iter().map(|c| q(c)).collect::<Vec<_>>().join(",");
        out.push('\n');
    }
    out
}

/// Fill `{Column}` placeholders with the values of row `row` (column names match without regard
/// to case or surrounding spaces). Unknown placeholders stay as typed; `{{` and `}}` are literal braces.
pub fn fill(text: &str, data: &DataSet, row: usize) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find(['{', '}']) {
        out.push_str(&rest[..i]);
        let tail = &rest[i..];
        if tail.starts_with("{{") || tail.starts_with("}}") {
            out.push_str(&tail[..1]);
            rest = &tail[2..];
            continue;
        }
        if tail.starts_with('{') {
            if let Some(end) = tail.find('}') {
                let name = &tail[1..end];
                if !name.contains('{') {
                    if let Some(v) = data.value(row, name) {
                        out.push_str(v);
                        rest = &tail[end + 1..];
                        continue;
                    }
                }
            }
        }
        out.push_str(&tail[..1]);
        rest = &tail[1..];
    }
    out.push_str(rest);
    out
}

/// the placeholder names used in `text`
pub fn placeholders(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(i) = rest.find('{') {
        let tail = &rest[i + 1..];
        if let Some(stripped) = tail.strip_prefix('{') {
            rest = stripped;
            continue;
        }
        match tail.find(['}', '{']) {
            Some(end) if tail.as_bytes()[end] == b'}' => {
                let name = tail[..end].trim().to_string();
                if !name.is_empty() && !out.contains(&name) {
                    out.push(name);
                }
                rest = &tail[end + 1..];
            }
            Some(end) => rest = &tail[end..],
            None => break,
        }
    }
    out
}

/// a small sample list to try batch cards with
pub fn sample() -> DataSet {
    let rows = [
        ["Alex Morgan", "Card designer", "+60 12-345 6789", "alex@example.com"],
        ["Priya Nair", "Head of Sales", "+60 13-222 1010", "priya@example.com"],
        ["Daniel Tan", "Engineer", "+60 17-808 4455", "daniel@example.com"],
        ["Sofia Rossi", "Marketing Lead", "+39 333 123 4567", "sofia@example.com"],
        ["Kenji Sato", "Photographer", "+81 90-1234-5678", "kenji@example.com"],
    ];
    DataSet {
        name: "Sample team".into(),
        columns: ["Name", "Title", "Phone", "Email"].iter().map(|s| s.to_string()).collect(),
        rows: rows.iter().map(|r| r.iter().map(|s| s.to_string()).collect()).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_csv_with_quotes_and_line_breaks() {
        let d = parse_csv("\u{feff}Name,Note\r\n\"Morgan, Alex\",\"says \"\"hi\"\"\nthen leaves\"\r\nBen,\r\n\r\n").unwrap();
        assert_eq!(d.columns, vec!["Name", "Note"]);
        assert_eq!(d.rows, vec![vec!["Morgan, Alex".to_string(), "says \"hi\"\nthen leaves".to_string()], vec!["Ben".to_string(), String::new()]]);
        assert_eq!(parse_csv(&to_csv(&d)).unwrap(), d);
    }

    #[test]
    fn detects_semicolons_and_tabs_and_pads_rows() {
        let d = parse_csv("a;b;c\n1;2\n").unwrap();
        assert_eq!(d.columns, vec!["a", "b", "c"]);
        assert_eq!(d.rows[0], vec!["1", "2", ""]);
        let d = parse_csv("Name\tCity\nAna\tLisbon\tExtra\n").unwrap();
        assert_eq!(d.columns, vec!["Name", "City", "Column 3"]);
        let d = parse_csv("x,X,\n1,2,3").unwrap();
        assert_eq!(d.columns, vec!["x", "X 2", "Column 3"]);
        assert!(parse_csv("").is_err());
        assert!(parse_csv("\"open").is_err());
    }

    #[test]
    fn fills_placeholders() {
        let d = sample();
        assert_eq!(fill("Hi {name}, {{literal}} {Unknown} {Phone}", &d, 1), "Hi Priya Nair, {literal} {Unknown} +60 13-222 1010");
        assert_eq!(fill("{ Title }!", &d, 0), "Card designer!");
        assert_eq!(fill("{a {Name}", &d, 0), "{a Alex Morgan");
        assert_eq!(placeholders("{Name} — {Title} {{x}} {Name} {bad"), vec!["Name", "Title"]);
    }
}
