use opencade_emulator_sdk::{
    AdapterCapabilities, AdapterError, DetectedEmulator, EmulatorAdapter, LaunchSpec,
    MatchDescriptor, NetplayMode, PeerRole, ProcessLauncher, StdProcessLauncher, ValidationReport,
    canonicalize_below,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Child;

const MAX_HASHED_FILE_BYTES: u64 = 512 * 1024 * 1024;

#[cfg(target_os = "windows")]
const EXECUTABLE: &str = "retroarch.exe";
#[cfg(target_os = "macos")]
const EXECUTABLE: &str = "retroarch";
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
const EXECUTABLE: &str = "retroarch";

#[cfg(target_os = "windows")]
const FBNEO_CORE: &str = "fbneo_libretro.dll";
#[cfg(target_os = "macos")]
const FBNEO_CORE: &str = "fbneo_libretro.dylib";
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
const FBNEO_CORE: &str = "fbneo_libretro.so";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompatibilityFingerprint {
    pub retroarch_version: Option<String>,
    pub executable_sha256: String,
    pub core_sha256: String,
    pub content_sha256: String,
}

#[derive(Debug, Clone)]
pub struct RetroarchAdapter {
    install_root: PathBuf,
}

impl RetroarchAdapter {
    pub fn new(install_root: impl Into<PathBuf>) -> Self {
        Self {
            install_root: install_root.into(),
        }
    }

    pub fn rom_root(&self) -> PathBuf {
        self.install_root.join("ROMs")
    }

    fn executable(&self) -> PathBuf {
        self.install_root.join(EXECUTABLE)
    }

    fn core(&self) -> PathBuf {
        self.install_root.join("cores").join(FBNEO_CORE)
    }

    fn installed_version(&self) -> Option<String> {
        std::fs::read_to_string(self.install_root.join("VERSION.txt"))
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }

    pub fn fingerprint(&self, rom_path: &Path) -> Result<CompatibilityFingerprint, AdapterError> {
        let executable = canonicalize_below(&self.executable(), &self.install_root)?;
        let core = canonicalize_below(&self.core(), &self.install_root)?;
        let rom = canonicalize_below(rom_path, &self.rom_root())?;
        Ok(CompatibilityFingerprint {
            retroarch_version: self.installed_version(),
            executable_sha256: sha256_file(&executable)?,
            core_sha256: sha256_file(&core)?,
            content_sha256: sha256_file(&rom)?,
        })
    }

    pub fn match_launch_spec(
        &self,
        rom_path: &Path,
        descriptor: &MatchDescriptor,
    ) -> Result<LaunchSpec, AdapterError> {
        self.prepare_match(descriptor)?;
        let transport = descriptor.transport_lease()?;
        self.validate(rom_path)?;
        let executable = canonicalize_below(&self.executable(), &self.install_root)?;
        let core = canonicalize_below(&self.core(), &self.install_root)?;
        let rom = canonicalize_below(rom_path, &self.rom_root())?;
        let current_dir = executable
            .parent()
            .ok_or_else(|| AdapterError::Validation("RetroArch has no parent directory".into()))?
            .to_path_buf();

        let mut args = vec![OsString::from("-L"), core.into_os_string()];
        match descriptor.role {
            PeerRole::Host => args.push(OsString::from("--host")),
            PeerRole::Guest => {
                args.push(OsString::from("--connect"));
                args.push(OsString::from(transport.peer_endpoint.ip().to_string()));
            }
        }
        let netplay_port = match descriptor.role {
            PeerRole::Host => transport.local_endpoint.port(),
            PeerRole::Guest => transport.peer_endpoint.port(),
        };
        args.extend([
            OsString::from("--port"),
            OsString::from(netplay_port.to_string()),
            OsString::from("--frames"),
            OsString::from(descriptor.input_delay_frames.to_string()),
            OsString::from("--verbose"),
            rom.into_os_string(),
        ]);
        Ok(LaunchSpec {
            executable,
            current_dir,
            args,
        })
    }

    pub fn launch_match_with<L: ProcessLauncher>(
        &self,
        launcher: &L,
        rom_path: &Path,
        descriptor: &MatchDescriptor,
    ) -> Result<L::Handle, AdapterError> {
        launcher.spawn(&self.match_launch_spec(rom_path, descriptor)?)
    }
}

impl EmulatorAdapter for RetroarchAdapter {
    fn id(&self) -> &str {
        "retroarch_fbneo"
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            local_play: true,
            netplay: NetplayMode::NativeProcess,
        }
    }

    fn detect(&self, install_dir: &Path) -> Result<DetectedEmulator, AdapterError> {
        let executable = install_dir.join(EXECUTABLE);
        let core = install_dir.join("cores").join(FBNEO_CORE);
        if !executable.is_file() || !core.is_file() {
            return Err(AdapterError::NotDetected(format!(
                "expected {EXECUTABLE} and cores/{FBNEO_CORE} below the RetroArch root"
            )));
        }
        Ok(DetectedEmulator {
            id: self.id().into(),
            executable: executable.canonicalize().map_err(AdapterError::Io)?,
            install_root: install_dir.canonicalize().map_err(AdapterError::Io)?,
            version: self.installed_version(),
        })
    }

    fn validate(&self, rom_path: &Path) -> Result<ValidationReport, AdapterError> {
        self.detect(&self.install_root)?;
        canonicalize_below(rom_path, &self.rom_root())?;
        if rom_path.extension().and_then(|value| value.to_str()) != Some("zip") {
            return Err(AdapterError::Validation(
                "RetroArch FBNeo content must be a .zip file".into(),
            ));
        }
        let mut report = ValidationReport::valid();
        if self.installed_version().is_none() {
            report.warnings.push(
                "VERSION.txt is missing; exact RetroArch version will not appear in evidence"
                    .into(),
            );
        }
        Ok(report)
    }

    fn get_version(&self) -> Result<String, AdapterError> {
        self.installed_version()
            .ok_or_else(|| AdapterError::NotDetected("VERSION.txt not found".into()))
    }

    fn launch(&self, rom_path: &Path) -> Result<Child, AdapterError> {
        self.validate(rom_path)?;
        let executable = canonicalize_below(&self.executable(), &self.install_root)?;
        let core = canonicalize_below(&self.core(), &self.install_root)?;
        let rom = canonicalize_below(rom_path, &self.rom_root())?;
        let current_dir = executable
            .parent()
            .ok_or_else(|| AdapterError::Validation("RetroArch has no parent directory".into()))?
            .to_path_buf();
        let spec = LaunchSpec {
            executable,
            current_dir,
            args: vec![
                OsString::from("-L"),
                core.into_os_string(),
                OsString::from("--verbose"),
                rom.into_os_string(),
            ],
        };
        StdProcessLauncher.spawn(&spec)
    }

    fn launch_match(
        &self,
        rom_path: &Path,
        descriptor: &MatchDescriptor,
    ) -> Result<Child, AdapterError> {
        self.launch_match_with(&StdProcessLauncher, rom_path, descriptor)
    }

    fn stop(&self, child: &mut Child) -> Result<(), AdapterError> {
        child.kill().map_err(AdapterError::Io)?;
        child.wait().map_err(AdapterError::Io)?;
        Ok(())
    }
}

fn sha256_file(path: &Path) -> Result<String, AdapterError> {
    let mut file = File::open(path).map_err(AdapterError::Io)?;
    let length = file.metadata().map_err(AdapterError::Io)?.len();
    if length > MAX_HASHED_FILE_BYTES {
        return Err(AdapterError::Validation(
            "compatibility input exceeds 512 MiB hashing limit".into(),
        ));
    }
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(AdapterError::Io)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}")
            .map_err(|_| AdapterError::Validation("failed to encode compatibility hash".into()))?;
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests {
    use super::*;
    use opencade_emulator_sdk::{TransportKind, ValidationReport};
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "opencade-retroarch-{}-{}",
            std::process::id(),
            FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("cores")).expect("core directory");
        std::fs::create_dir_all(root.join("ROMs")).expect("ROM directory");
        std::fs::write(root.join(EXECUTABLE), b"retroarch").expect("executable");
        std::fs::write(root.join("cores").join(FBNEO_CORE), b"fbneo-core").expect("core");
        std::fs::write(root.join("ROMs/sfiii3.zip"), b"content").expect("content");
        std::fs::write(root.join("VERSION.txt"), "1.22.0\n").expect("version");
        root
    }

    fn descriptor(role: PeerRole) -> MatchDescriptor {
        MatchDescriptor {
            room_id: "room-1".into(),
            game_id: "sfiii3".into(),
            local_user_id: "user-a".into(),
            peer_user_id: "user-b".into(),
            role,
            transport: TransportKind::DirectUdp,
            local_endpoint: "192.168.1.10:42000".parse().expect("endpoint"),
            peer_endpoint: "192.168.1.11:42001".parse().expect("endpoint"),
            input_delay_frames: 2,
        }
    }

    #[test]
    fn detects_native_netplay_capability_and_fingerprints_inputs() {
        let root = fixture();
        let adapter = RetroarchAdapter::new(&root);
        assert_eq!(adapter.capabilities().netplay, NetplayMode::NativeProcess);
        assert_eq!(
            adapter
                .validate(&root.join("ROMs/sfiii3.zip"))
                .expect("valid"),
            ValidationReport::valid()
        );
        let fingerprint = adapter
            .fingerprint(&root.join("ROMs/sfiii3.zip"))
            .expect("fingerprint");
        assert_eq!(fingerprint.retroarch_version.as_deref(), Some("1.22.0"));
        assert_eq!(fingerprint.executable_sha256.len(), 64);
        assert_eq!(fingerprint.core_sha256.len(), 64);
        assert_eq!(fingerprint.content_sha256.len(), 64);
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn builds_host_and_guest_arguments_without_a_shell() {
        let root = fixture();
        let adapter = RetroarchAdapter::new(&root);
        let host = adapter
            .match_launch_spec(&root.join("ROMs/sfiii3.zip"), &descriptor(PeerRole::Host))
            .expect("host spec");
        assert!(host.args.iter().any(|arg| arg == "--host"));
        assert!(host.args.iter().any(|arg| arg == "42000"));

        let guest = adapter
            .match_launch_spec(&root.join("ROMs/sfiii3.zip"), &descriptor(PeerRole::Guest))
            .expect("guest spec");
        let args = guest
            .args
            .iter()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>();
        assert!(
            args.windows(2)
                .any(|pair| pair == ["--connect", "192.168.1.11"])
        );
        assert_eq!(
            guest.args.last(),
            Some(
                &root
                    .join("ROMs/sfiii3.zip")
                    .canonicalize()
                    .expect("rom")
                    .into_os_string()
            )
        );
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn rejects_content_outside_the_rom_root() {
        let root = fixture();
        let outside = root.parent().expect("parent").join("outside-retroarch.zip");
        std::fs::write(&outside, b"outside").expect("outside content");
        let adapter = RetroarchAdapter::new(&root);
        assert!(adapter.fingerprint(&outside).is_err());
        std::fs::remove_file(outside).expect("remove outside content");
        std::fs::remove_dir_all(root).expect("remove fixture");
    }
}
