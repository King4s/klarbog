//! SHA-256 helpers for backup manifests.

use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DigestError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub fn sha256_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

pub fn sha256_file(path: &Path) -> Result<String, DigestError> {
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn hashes_known_content() {
        let mut tmp = NamedTempFile::new().unwrap();
        write!(tmp, "klarbog").unwrap();
        let digest = sha256_file(tmp.path()).unwrap();
        assert_eq!(digest.len(), 64);
    }
}
