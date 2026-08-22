//! OpenFight emulator adapter SDK — pluggable backends (FBNeo, Flycast, etc.)

use std::path::Path;
use std::process::Child;

/// Errors returned by adapter operations.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("emulator not detected: {0}")]
    NotDetected(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("launch failed: {0}")]
    Launch(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Pluggable emulator backend.
pub trait EmulatorAdapter: Send + Sync {
    /// Human-readable id, e.g. "fbneo".
    fn id(&self) -> &str;

    /// Whether the emulator is present in `install_dir`.
    fn detect(&self, install_dir: &Path) -> bool;

    /// Validate that `rom_path` (and required files) are usable.
    fn validate(&self, rom_path: &Path) -> Result<(), AdapterError>;

    /// Installed emulator version, if detectable.
    fn get_version(&self) -> Result<String, AdapterError>;

    /// Launch the emulator for `rom_path`; caller owns the child process.
    /// Implementations MUST NOT use shell injection; pass args directly.
    fn launch(&self, rom_path: &Path) -> Result<Child, AdapterError>;

    /// Stop a running emulator child.
    fn stop(&self, child: &mut Child) -> Result<(), AdapterError>;
}
