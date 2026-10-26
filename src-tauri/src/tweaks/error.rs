use crate::guard::GuardViolation;
use crate::sys::SysError;

#[derive(Debug, Clone, thiserror::Error)]
pub enum TweakError {
    #[error(transparent)]
    Refused(#[from] GuardViolation),
    #[error("Not available on this PC: {0}")]
    NotApplicable(String),
    #[error("Windows refused: {0}")]
    Sys(#[from] SysError),
    #[error("{0}")]
    Verify(String),
    #[error("Couldn't save the backup, so nothing was changed: {0}")]
    Journal(String),
    #[error("{0}")]
    NoBackup(String),
    #[error("A restore point is needed before the first change: {0}")]
    RestorePointRequired(String),
    #[error("{tweak} failed and was undone. {cause}")]
    RolledBack {
        tweak: String,
        cause: Box<TweakError>,
    },
    #[error(
        "{tweak} failed and could not be fully undone. {cause} These settings may need your attention: {}",
        failures.join("; ")
    )]
    RollbackFailed {
        tweak: String,
        cause: Box<TweakError>,
        failures: Vec<String>,
    },
}

impl TweakError {
    /// Stable identifier for the UI.
    pub fn kind(&self) -> &'static str {
        match self {
            TweakError::Refused(_) => "refused",
            TweakError::NotApplicable(_) => "notApplicable",
            TweakError::Sys(_) => "windows",
            TweakError::Verify(_) => "verify",
            TweakError::Journal(_) => "journal",
            TweakError::NoBackup(_) => "noBackup",
            TweakError::RestorePointRequired(_) => "restorePointRequired",
            TweakError::RolledBack { .. } => "rolledBack",
            TweakError::RollbackFailed { .. } => "rollbackFailed",
        }
    }
}
