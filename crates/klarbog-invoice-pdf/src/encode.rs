//! WinAnsi (CP1252) encoding for PDF literal strings.

/// Code points that differ between Unicode and WinAnsi byte values.
fn winansi_override(code: u32) -> Option<u8> {
    Some(match code {
        0x20ac => 0x80, // €
        0x201a => 0x82,
        0x0192 => 0x83,
        0x201e => 0x84,
        0x2026 => 0x85, // …
        0x2020 => 0x86,
        0x2021 => 0x87,
        0x02c6 => 0x88,
        0x2030 => 0x89,
        0x0160 => 0x8a,
        0x2039 => 0x8b,
        0x0152 => 0x8c, // Œ
        0x017d => 0x8e,
        0x2018 => 0x91,
        0x2019 => 0x92, // ’
        0x201c => 0x93,
        0x201d => 0x94,
        0x2022 => 0x95, // •
        0x2013 => 0x96, // –
        0x2014 => 0x97, // —
        0x02dc => 0x98,
        0x2122 => 0x99,
        0x0161 => 0x9a,
        0x203a => 0x9b,
        0x0153 => 0x9c, // œ
        0x017e => 0x9e,
        0x0178 => 0x9f,
        _ => return None,
    })
}

/// Encode Unicode text to WinAnsi bytes (as a Latin-1 string of those bytes).
pub fn encode_winansi(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        let code = ch as u32;
        if code <= 0xff {
            out.push(ch);
        } else if let Some(b) = winansi_override(code) {
            out.push(char::from(b));
        } else {
            out.push('?');
        }
    }
    out
}

/// Escape a WinAnsi byte string for a PDF literal `( ... )` string.
pub fn escape_pdf_text(value: &str) -> String {
    let encoded = encode_winansi(value);
    let mut out = String::with_capacity(encoded.len() + 8);
    for ch in encoded.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\r' | '\n' => out.push(' '),
            _ => out.push(ch),
        }
    }
    out
}

pub fn compact(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn danish_letters_pass_through() {
        let s = encode_winansi("Sælger Køber Århus");
        assert!(s.contains('æ') && s.contains('ø') && s.contains('Å'));
    }

    #[test]
    fn escapes_parens() {
        assert_eq!(escape_pdf_text("a(b)c"), r"a\(b\)c");
    }
}
