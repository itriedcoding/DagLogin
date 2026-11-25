use serde::{Serialize, Serializer};

/// Unified error type for every fallible operation in the engine.
///
/// The `Display` implementation is user facing: the desktop UI shows these
/// strings directly, so they must stay readable and actionable.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("file system error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("encryption error: {0}")]
    Crypto(String),
    #[error("archive error: {0}")]
    Zip(#[from] zip::result::ZipError),
    #[error("network error: {0}")]
    Network(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("validation error: {0}")]
    Validation(String),
}

impl AppError {
    pub fn msg(value: impl Into<String>) -> Self {
        AppError::Message(value.into())
    }

    pub fn validation(value: impl Into<String>) -> Self {
        AppError::Validation(value.into())
    }

    pub fn not_found(value: impl Into<String>) -> Self {
        AppError::NotFound(value.into())
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_human_readable() {
        let err = AppError::validation("profile name is required");
        assert_eq!(err.to_string(), "validation error: profile name is required");
        assert_eq!(
            serde_json::to_string(&err).unwrap(),
            "\"validation error: profile name is required\""
        );
    }
}
