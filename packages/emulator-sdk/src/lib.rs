//! OpenCade emulator adapter SDK — pluggable backends (FBNeo, Flycast, etc.)

use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;

/// Errors returned by adapter operations.
#[derive(Debug, thiserror::Error)]
pub enum AdapterError {
    #[error("emulator not detected: {0}")]
    NotDetected(String),
    #[error("validation failed: {0}")]
    Validation(String),
    #[error("launch failed: {0}")]
    Launch(String),
    #[error("match preparation failed: {0}")]
    MatchPreparation(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetectedEmulator {
    pub id: String,
    pub executable: PathBuf,
    pub install_root: PathBuf,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidationReport {
    pub valid: bool,
    pub warnings: Vec<String>,
}

impl ValidationReport {
    pub fn valid() -> Self {
        Self {
            valid: true,
            warnings: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaunchSpec {
    pub executable: PathBuf,
    pub current_dir: PathBuf,
    pub args: Vec<OsString>,
}

pub trait ProcessLauncher {
    type Handle;

    fn spawn(&self, spec: &LaunchSpec) -> Result<Self::Handle, AdapterError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct StdProcessLauncher;

impl ProcessLauncher for StdProcessLauncher {
    type Handle = Child;

    fn spawn(&self, spec: &LaunchSpec) -> Result<Self::Handle, AdapterError> {
        Command::new(&spec.executable)
            .args(&spec.args)
            .current_dir(&spec.current_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(AdapterError::Io)
    }
}

pub fn canonicalize_below(path: &Path, root: &Path) -> Result<PathBuf, AdapterError> {
    let root = root.canonicalize().map_err(AdapterError::Io)?;
    let path = path.canonicalize().map_err(AdapterError::Io)?;
    if path == root || !path.starts_with(&root) {
        return Err(AdapterError::Validation(
            "path must resolve below the configured root".into(),
        ));
    }
    Ok(path)
}

pub fn spawn_validated<L: ProcessLauncher>(
    launcher: &L,
    executable: &Path,
    executable_root: &Path,
    rom: &Path,
    rom_root: &Path,
    additional_args: &[OsString],
) -> Result<L::Handle, AdapterError> {
    let executable = canonicalize_below(executable, executable_root)?;
    let rom = canonicalize_below(rom, rom_root)?;
    let current_dir = executable
        .parent()
        .ok_or_else(|| AdapterError::Validation("emulator has no parent directory".into()))?
        .to_path_buf();
    let mut args = Vec::with_capacity(additional_args.len() + 1);
    args.extend_from_slice(additional_args);
    args.push(rom.into_os_string());
    launcher.spawn(&LaunchSpec {
        executable,
        current_dir,
        args,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PeerRole {
    Host,
    Guest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransportKind {
    InMemory,
    DirectUdp,
    HolePunch,
    Stun,
    Relay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransportLease {
    pub kind: TransportKind,
    pub local_endpoint: SocketAddr,
    pub peer_endpoint: SocketAddr,
}

impl TransportLease {
    pub fn native_process(
        kind: TransportKind,
        local_endpoint: SocketAddr,
        peer_endpoint: SocketAddr,
    ) -> Result<Self, AdapterError> {
        if kind != TransportKind::DirectUdp {
            return Err(AdapterError::MatchPreparation(
                "native-process netplay requires an explicitly authorized direct route".into(),
            ));
        }
        if local_endpoint.port() == 0 || peer_endpoint.port() == 0 {
            return Err(AdapterError::MatchPreparation(
                "transport lease endpoints require non-zero ports".into(),
            ));
        }
        Ok(Self {
            kind,
            local_endpoint,
            peer_endpoint,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MatchDescriptor {
    pub room_id: String,
    pub game_id: String,
    pub local_user_id: String,
    pub peer_user_id: String,
    pub role: PeerRole,
    pub transport: TransportKind,
    pub local_endpoint: SocketAddr,
    pub peer_endpoint: SocketAddr,
    pub input_delay_frames: u8,
}

impl MatchDescriptor {
    pub fn validate(&self) -> Result<(), AdapterError> {
        if self.room_id.trim().is_empty()
            || self.game_id.trim().is_empty()
            || self.local_user_id.trim().is_empty()
            || self.peer_user_id.trim().is_empty()
        {
            return Err(AdapterError::MatchPreparation(
                "match identifiers must not be empty".into(),
            ));
        }
        if self.local_user_id == self.peer_user_id {
            return Err(AdapterError::MatchPreparation(
                "local and peer users must be different".into(),
            ));
        }
        if self.input_delay_frames > 15 {
            return Err(AdapterError::MatchPreparation(
                "input delay must be between 0 and 15 frames".into(),
            ));
        }
        Ok(())
    }

    pub fn transport_lease(&self) -> Result<TransportLease, AdapterError> {
        TransportLease::native_process(self.transport, self.local_endpoint, self.peer_endpoint)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AdapterCapabilities {
    pub local_play: bool,
    pub netplay: NetplayMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetplayMode {
    /// OpenCade owns the deterministic input-frame data plane.
    OpenCadeFrames,
    /// A separately installed emulator owns netplay through a documented process interface.
    NativeProcess,
    BlockedNoPublicInterface,
}

impl AdapterCapabilities {
    pub fn supports_netplay(self) -> bool {
        self.netplay != NetplayMode::BlockedNoPublicInterface
    }
}

/// Pluggable emulator backend.
pub trait EmulatorAdapter: Send + Sync {
    /// Human-readable id, e.g. "fbneo".
    fn id(&self) -> &str;

    /// Explicit capabilities prevent local launch from being mistaken for netplay support.
    fn capabilities(&self) -> AdapterCapabilities;

    /// Whether the emulator is present in `install_dir`.
    fn detect(&self, install_dir: &Path) -> Result<DetectedEmulator, AdapterError>;

    /// Validate that `rom_path` (and required files) are usable.
    fn validate(&self, rom_path: &Path) -> Result<ValidationReport, AdapterError>;

    /// Installed emulator version, if detectable.
    fn get_version(&self) -> Result<String, AdapterError>;

    /// Validate and prepare a match before starting the emulator process.
    fn prepare_match(&self, descriptor: &MatchDescriptor) -> Result<(), AdapterError> {
        descriptor.validate()?;
        if !self.capabilities().supports_netplay() {
            return Err(AdapterError::MatchPreparation(format!(
                "adapter '{}' does not provide netplay",
                self.id()
            )));
        }
        Ok(())
    }

    /// Launch the emulator for `rom_path`; caller owns the child process.
    /// Implementations MUST NOT use shell injection; pass args directly.
    fn launch(&self, rom_path: &Path) -> Result<Child, AdapterError>;

    /// Launch a netplay match. Adapters with a native process interface override this method.
    fn launch_match(
        &self,
        rom_path: &Path,
        descriptor: &MatchDescriptor,
    ) -> Result<Child, AdapterError> {
        self.prepare_match(descriptor)?;
        self.launch(rom_path)
    }

    /// Stop a running emulator child.
    fn stop(&self, child: &mut Child) -> Result<(), AdapterError>;
}

/// Deterministic adapter used by Proof-of-Match tests. It never starts an external process.
#[derive(Debug, Default)]
pub struct MockAdapter {
    prepared: Mutex<Vec<MatchDescriptor>>,
}

impl MockAdapter {
    pub fn prepared_matches(&self) -> Result<Vec<MatchDescriptor>, AdapterError> {
        self.prepared
            .lock()
            .map(|matches| matches.clone())
            .map_err(|_| AdapterError::MatchPreparation("mock adapter lock poisoned".into()))
    }
}

impl EmulatorAdapter for MockAdapter {
    fn id(&self) -> &str {
        "mock"
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            local_play: false,
            netplay: NetplayMode::OpenCadeFrames,
        }
    }

    fn detect(&self, install_dir: &Path) -> Result<DetectedEmulator, AdapterError> {
        Ok(DetectedEmulator {
            id: self.id().into(),
            executable: install_dir.join("mock-adapter"),
            install_root: install_dir.to_path_buf(),
            version: Some("test".into()),
        })
    }

    fn validate(&self, _rom_path: &Path) -> Result<ValidationReport, AdapterError> {
        Ok(ValidationReport::valid())
    }

    fn get_version(&self) -> Result<String, AdapterError> {
        Ok("test".into())
    }

    fn prepare_match(&self, descriptor: &MatchDescriptor) -> Result<(), AdapterError> {
        descriptor.validate()?;
        self.prepared
            .lock()
            .map_err(|_| AdapterError::MatchPreparation("mock adapter lock poisoned".into()))?
            .push(descriptor.clone());
        Ok(())
    }

    fn launch(&self, _rom_path: &Path) -> Result<Child, AdapterError> {
        Err(AdapterError::Launch(
            "mock adapter does not launch an external process".into(),
        ))
    }

    fn stop(&self, _child: &mut Child) -> Result<(), AdapterError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> MatchDescriptor {
        MatchDescriptor {
            room_id: "room-1".into(),
            game_id: "sfiii3".into(),
            local_user_id: "user-a".into(),
            peer_user_id: "user-b".into(),
            role: PeerRole::Host,
            transport: TransportKind::InMemory,
            local_endpoint: "127.0.0.1:41000".parse().expect("valid endpoint"),
            peer_endpoint: "127.0.0.1:41001".parse().expect("valid endpoint"),
            input_delay_frames: 2,
        }
    }

    #[test]
    fn accepts_a_complete_match_descriptor() {
        assert!(descriptor().validate().is_ok());
    }

    #[test]
    fn rejects_self_matches_and_excessive_delay() {
        let mut value = descriptor();
        value.peer_user_id.clone_from(&value.local_user_id);
        assert!(value.validate().is_err());

        let mut value = descriptor();
        value.input_delay_frames = 16;
        assert!(value.validate().is_err());
    }

    #[test]
    fn native_transport_lease_rejects_probe_only_relay() {
        let mut descriptor = descriptor();
        descriptor.transport = TransportKind::Relay;
        assert!(descriptor.transport_lease().is_err());
    }

    #[derive(Default)]
    struct RecordingLauncher;

    impl ProcessLauncher for RecordingLauncher {
        type Handle = LaunchSpec;

        fn spawn(&self, spec: &LaunchSpec) -> Result<Self::Handle, AdapterError> {
            Ok(spec.clone())
        }
    }

    #[test]
    fn safe_launch_preserves_paths_with_spaces_and_rejects_escape() {
        let raw_thread = std::thread::current()
            .name()
            .unwrap_or("test")
            .replace("::", "_")
            .replace(
                |c: char| !c.is_ascii_alphanumeric() && c != '_' && c != '-',
                "_",
            );
        let fixture = std::env::temp_dir().join(format!(
            "opencade-sdk-fixture-{}-{}",
            std::process::id(),
            raw_thread
        ));
        let emulator_root = fixture.join("emulator root");
        let rom_root = fixture.join("ROMs");
        std::fs::create_dir_all(&emulator_root).expect("emulator fixture");
        std::fs::create_dir_all(&rom_root).expect("rom fixture");
        let executable = emulator_root.join("mock emulator.exe");
        let rom = rom_root.join("game with spaces.zip");
        let outside = fixture.join("outside.zip");
        std::fs::write(&executable, b"mock").expect("mock executable");
        std::fs::write(&rom, b"mock").expect("mock rom");
        std::fs::write(&outside, b"mock").expect("outside fixture");

        let spec = spawn_validated(
            &RecordingLauncher,
            &executable,
            &emulator_root,
            &rom,
            &rom_root,
            &[OsString::from("--rom")],
        )
        .expect("safe launch");
        assert_eq!(
            spec.executable,
            executable.canonicalize().expect("canonical executable")
        );
        assert_eq!(
            spec.args.last(),
            Some(&rom.canonicalize().expect("canonical rom").into_os_string())
        );
        assert!(
            spawn_validated(
                &RecordingLauncher,
                &executable,
                &emulator_root,
                &outside,
                &rom_root,
                &[],
            )
            .is_err()
        );

        std::fs::remove_dir_all(fixture).expect("remove fixture");
    }
}
