use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum KlarbogError {
    #[error("{0}")]
    Message(String),
    #[error("confirm token required")]
    ConfirmRequired,
    #[error("unbalanced journal entry")]
    Unbalanced,
    #[error("invalid journal entry")]
    InvalidEntry,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope<T> {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<T>,
    pub errors: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub applied_rules: Vec<String>,
}

impl<T> Envelope<T> {
    pub fn ok(data: T) -> Self {
        Self {
            ok: true,
            data: Some(data),
            errors: vec![],
            applied_rules: vec![],
        }
    }

    pub fn ok_with_rules(data: T, applied_rules: Vec<String>) -> Self {
        Self {
            ok: true,
            data: Some(data),
            errors: vec![],
            applied_rules,
        }
    }

    pub fn err(errors: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self {
            ok: false,
            data: None,
            errors: errors.into_iter().map(Into::into).collect(),
            applied_rules: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_ok_and_err() {
        let ok = Envelope::ok(1);
        assert!(ok.ok && ok.errors.is_empty());
        let err: Envelope<()> = Envelope::err(["no"]);
        assert!(!err.ok && err.data.is_none());
    }
}
