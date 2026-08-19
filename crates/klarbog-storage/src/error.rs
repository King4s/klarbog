use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StorageError {
    #[error("object key must not be empty")]
    EmptyKey,
    #[error("object key contains '..'")]
    ParentDirInKey,
    #[error("object key must be relative (no leading slash)")]
    AbsoluteKey,
    #[error("storage root must be absolute")]
    NotAbsolute,
    #[error("storage root contains '..'")]
    ParentDirInRoot,
    #[error("storage root outside allowlist")]
    OutsideAllowlist,
    #[error("resolved path escapes storage root")]
    PathEscape,
    #[error("object not found: {0}")]
    NotFound(String),
    #[error("io error: {0}")]
    Io(String),
    #[error("missing env var: {0}")]
    MissingEnv(&'static str),
    #[error("invalid R2 config: {0}")]
    InvalidR2Config(String),
    #[error("R2 endpoint/jurisdiction is not EU; set KLARBOG_R2_ALLOW_NON_EU=1 to override")]
    NonEuJurisdiction,
    #[error("R2 object store not implemented in DEV scaffold")]
    NotImplementedInDev,
}

impl From<std::io::Error> for StorageError {
    fn from(err: std::io::Error) -> Self {
        StorageError::Io(err.to_string())
    }
}
