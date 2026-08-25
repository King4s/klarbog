//! Danish number presentation from i64 minor units (øre).

/// Format øre as Danish "1.234,56".
pub fn format_danish_minor(minor: i64) -> String {
    let negative = minor < 0;
    let abs = minor.unsigned_abs();
    let whole = abs / 100;
    let frac = abs % 100;
    let whole_s = whole.to_string();
    let mut grouped = String::new();
    let digits: Vec<char> = whole_s.chars().collect();
    for (i, ch) in digits.iter().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            grouped.push('.');
        }
        grouped.push(*ch);
    }
    format!("{}{},{:02}", if negative { "-" } else { "" }, grouped, frac)
}

pub fn format_danish_dkk_minor(minor: i64, currency: &str) -> String {
    format!(
        "{} {}",
        format_danish_minor(minor),
        currency.trim().to_uppercase()
    )
}

/// Parse issued-snapshot amount strings ("125.00") into minor units.
pub fn parse_amount_str_to_minor(s: &str) -> Option<i64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let negative = t.starts_with('-');
    let unsigned = if negative { &t[1..] } else { t };
    let mut parts = unsigned.split('.');
    let whole: i64 = parts.next()?.parse().ok()?;
    let frac_str = parts.next().unwrap_or("00");
    let frac_padded = format!("{frac_str:0<2}");
    let frac: i64 = frac_padded[..2].parse().ok()?;
    let minor = whole.checked_mul(100)?.checked_add(frac)?;
    Some(if negative { -minor } else { minor })
}

/// Format milli-quantity (1000 = 1) without floats.
pub fn format_quantity_milli(milli: i64) -> String {
    if milli % 1000 == 0 {
        return (milli / 1000).to_string();
    }
    let whole = milli / 1000;
    let frac = (milli % 1000).unsigned_abs();
    // trim trailing zeros in fractional part
    let mut frac_s = format!("{frac:03}");
    while frac_s.ends_with('0') {
        frac_s.pop();
    }
    format!("{whole}.{frac_s}")
}

/// Format micro FX rate (×1_000_000) as fixed 6 decimals.
pub fn format_fx_micro(micro: i64) -> String {
    let negative = micro < 0;
    let abs = micro.unsigned_abs();
    let whole = abs / 1_000_000;
    let frac = abs % 1_000_000;
    format!("{}{}.{:06}", if negative { "-" } else { "" }, whole, frac)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_thousands() {
        assert_eq!(format_danish_minor(123_456), "1.234,56");
        assert_eq!(format_danish_minor(-100_000), "-1.000,00");
    }

    #[test]
    fn parses_amount_strings() {
        assert_eq!(parse_amount_str_to_minor("125.00"), Some(12_500));
        assert_eq!(parse_amount_str_to_minor("100.5"), Some(10_050));
    }
}
