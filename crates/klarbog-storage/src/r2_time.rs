//! AMZ date formatting helpers for R2 SigV4 (no chrono dep).

use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn amz_date_now() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format_amz_date(secs)
}

pub(crate) fn format_amz_date(unix_secs: u64) -> String {
    let days_since_epoch = unix_secs / 86_400;
    let time_of_day = unix_secs % 86_400;
    let (y, m, d) = civil_from_days(days_since_epoch as i64);
    let hh = time_of_day / 3600;
    let mm = (time_of_day % 3600) / 60;
    let ss = time_of_day % 60;
    format!("{y:04}{m:02}{d:02}T{hh:02}{mm:02}{ss:02}Z")
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let y = y + if m <= 2 { 1 } else { 0 };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::format_amz_date;

    #[test]
    fn formats_known_unix_epoch_day() {
        // 0 -> 1970-01-01T00:00:00Z
        assert_eq!(format_amz_date(0), "19700101T000000Z");
    }
}
