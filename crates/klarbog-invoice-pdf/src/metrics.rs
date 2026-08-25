//! Approximate Helvetica advance widths — sizes and widths in centipoints (1/100 pt).

/// Advance width in milliemes of em × size_cp / 1000 → centipoints.
pub fn text_width_cp(text: &str, size_cp: i32) -> i32 {
    let mut units: i64 = 0;
    for ch in text.chars() {
        let code = ch as u32;
        units += if code == 0x20 {
            278
        } else if (0x30..=0x39).contains(&code) {
            556
        } else if matches!(ch, 'A'..='Z' | 'Æ' | 'Ø' | 'Å') {
            667
        } else if matches!(ch, '.' | ',' | ':' | ';' | '\'' | '|' | '!') {
            280
        } else {
            540
        };
    }
    ((units * i64::from(size_cp)) / 1000) as i32
}

pub fn right_align_x_cp(text: &str, size_cp: i32, right_x_cp: i32) -> i32 {
    right_x_cp - text_width_cp(text, size_cp)
}

pub fn fit_text(text: &str, size_cp: i32, max_width_cp: i32) -> String {
    if text_width_cp(text, size_cp) <= max_width_cp {
        return text.to_string();
    }
    let mut cut = text.to_string();
    while cut.chars().count() > 1 {
        let candidate = format!("{cut}…");
        if text_width_cp(&candidate, size_cp) <= max_width_cp {
            return format!("{}…", cut.trim_end());
        }
        cut.pop();
        while cut.ends_with(char::is_whitespace) {
            cut.pop();
        }
    }
    "…".into()
}

pub fn wrap_text(text: &str, size_cp: i32, max_width_cp: i32) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let mut lines = Vec::new();
    let mut current = String::new();
    for word in words {
        let candidate = if current.is_empty() {
            word.to_string()
        } else {
            format!("{current} {word}")
        };
        if text_width_cp(&candidate, size_cp) > max_width_cp && !current.is_empty() {
            lines.push(current);
            current = word.to_string();
        } else {
            current = candidate;
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}
