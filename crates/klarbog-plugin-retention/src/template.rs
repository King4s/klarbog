//! UTF-8 expense memo template under `templates/expense_memo.md` (slice 9).

use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const TEMPLATE_REL_PATH: &str = "templates/expense_memo.md";

/// Short markdown template — placeholders replaced by agents/UI later.
pub const EXPENSE_MEMO_TEMPLATE: &str = r#"# Udgift — {{date}}

Beløb: {{amount_minor}} {{currency}}
Konto: {{account}}
Formål:

Bilag:
"#;

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

fn template_path(company: &Path) -> PathBuf {
    company.join(TEMPLATE_REL_PATH)
}

/// Write default expense memo template if missing (company init hook).
pub fn ensure_expense_memo_template(company: &Path) -> Result<(), TemplateError> {
    let path = template_path(company);
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, EXPENSE_MEMO_TEMPLATE)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn writes_utf8_template_once() {
        let dir = tempdir().unwrap();
        let co = dir.path().join("co");
        fs::create_dir_all(&co).unwrap();
        ensure_expense_memo_template(&co).unwrap();
        let text = fs::read_to_string(template_path(&co)).unwrap();
        assert!(text.contains("{{date}}"));
        ensure_expense_memo_template(&co).unwrap();
    }
}
