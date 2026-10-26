use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Errors returned to the UI. Every variant carries a message a user can act on.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("File system error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Windows API call failed: {0}")]
    Windows(String),
    #[error("{0}")]
    Other(String),
}

impl AppError {
    fn kind(&self) -> &'static str {
        match self {
            AppError::Io(_) => "io",
            AppError::Windows(_) => "windows",
            AppError::Other(_) => "other",
        }
    }
}

#[cfg(windows)]
impl From<windows::core::Error> for AppError {
    fn from(e: windows::core::Error) -> Self {
        AppError::Windows(format!("{} (0x{:08X})", e.message(), e.code().0))
    }
}

// Serialized as `{ kind, message }` so the frontend can show it directly.
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut s = serializer.serialize_struct("AppError", 2)?;
        s.serialize_field("kind", self.kind())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_kind_and_message() {
        let json = serde_json::to_value(AppError::Other("nope".into())).unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "kind": "other", "message": "nope" })
        );
    }
}
