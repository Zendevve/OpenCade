use opencade_emulator_sdk::{
    AdapterCapabilities, AdapterError, DetectedEmulator, EmulatorAdapter, NetplayMode,
    StdProcessLauncher, ValidationReport, spawn_validated,
};
use std::path::{Path, PathBuf};
use std::process::Child;

const EXECUTABLE: &str = "fcadefbneo.exe";
const CONFIG_FILE: &str = "fcadefbneo.default.ini";
const EXPECTED_VERSION: &str = "2.1.45";

#[derive(Debug, Clone)]
pub struct FbneoAdapter {
    install_root: PathBuf,
}

impl FbneoAdapter {
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

    fn installed_version(&self) -> Option<String> {
        std::fs::read_to_string(self.install_root.join("VERSION.txt"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    }
}

impl EmulatorAdapter for FbneoAdapter {
    fn id(&self) -> &str {
        "fbneo"
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            local_play: true,
            netplay: NetplayMode::BlockedNoPublicInterface,
        }
    }

    fn detect(&self, install_dir: &Path) -> Result<DetectedEmulator, AdapterError> {
        let executable = install_dir.join(EXECUTABLE);
        let config = install_dir.join(CONFIG_FILE);
        if !executable.is_file() || !config.is_file() {
            return Err(AdapterError::NotDetected(format!(
                "expected {EXECUTABLE} and {CONFIG_FILE} below the FBNeo root"
            )));
        }
        Ok(DetectedEmulator {
            id: self.id().into(),
            executable: executable.canonicalize().map_err(AdapterError::Io)?,
            install_root: install_dir.canonicalize().map_err(AdapterError::Io)?,
            version: std::fs::read_to_string(install_dir.join("VERSION.txt"))
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        })
    }

    fn validate(&self, rom_path: &Path) -> Result<ValidationReport, AdapterError> {
        opencade_emulator_sdk::canonicalize_below(rom_path, &self.rom_root())?;
        if rom_path
            .extension()
            .and_then(|extension| extension.to_str())
            != Some("zip")
        {
            return Err(AdapterError::Validation(
                "FBNeo ROM must be a .zip file".into(),
            ));
        }
        if !self.rom_root().join("neogeo.zip").is_file() {
            return Err(AdapterError::Validation(
                "required BIOS file neogeo.zip is missing".into(),
            ));
        }
        let mut report = ValidationReport::valid();
        match self.installed_version() {
            Some(version) if version != EXPECTED_VERSION => report.warnings.push(format!(
                "FBNeo version {version} differs from tested version {EXPECTED_VERSION}"
            )),
            None => report
                .warnings
                .push("VERSION.txt is missing; compatibility is unknown".into()),
            Some(_) => {}
        }
        Ok(report)
    }

    fn get_version(&self) -> Result<String, AdapterError> {
        self.installed_version()
            .ok_or_else(|| AdapterError::NotDetected("VERSION.txt not found".into()))
    }

    fn launch(&self, rom_path: &Path) -> Result<Child, AdapterError> {
        self.detect(&self.install_root)?;
        self.validate(rom_path)?;
        spawn_validated(
            &StdProcessLauncher,
            &self.executable(),
            &self.install_root,
            rom_path,
            &self.rom_root(),
            &[],
        )
    }

    fn stop(&self, child: &mut Child) -> Result<(), AdapterError> {
        child.kill().map_err(AdapterError::Io)?;
        child.wait().map_err(AdapterError::Io)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn fixture() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "opencade-fbneo-{}-{}",
            std::process::id(),
            FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(root.join("ROMs")).expect("fixture directories");
        std::fs::write(root.join(EXECUTABLE), b"mock").expect("mock executable");
        std::fs::write(root.join(CONFIG_FILE), b"mock").expect("mock config");
        std::fs::write(root.join("VERSION.txt"), EXPECTED_VERSION).expect("mock version");
        std::fs::write(root.join("ROMs/neogeo.zip"), b"mock").expect("mock bios");
        std::fs::write(root.join("ROMs/sfiii3.zip"), b"mock").expect("mock rom");
        root
    }

    #[test]
    fn detects_and_validates_a_fixture_without_claiming_netplay() {
        let root = fixture();
        let adapter = FbneoAdapter::new(&root);
        assert!(adapter.detect(&root).is_ok());
        assert_eq!(adapter.get_version().expect("version"), EXPECTED_VERSION);
        assert_eq!(
            adapter
                .validate(&root.join("ROMs/sfiii3.zip"))
                .expect("validation"),
            ValidationReport::valid()
        );
        assert_eq!(
            adapter.capabilities().netplay,
            NetplayMode::BlockedNoPublicInterface
        );
        assert!(!adapter.capabilities().supports_netplay());
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    #[test]
    fn rejects_roms_outside_the_configured_root() {
        let root = fixture();
        let outside = root.parent().expect("fixture parent").join("outside.zip");
        std::fs::write(&outside, b"mock").expect("outside fixture");
        let adapter = FbneoAdapter::new(&root);
        assert!(adapter.validate(&outside).is_err());
        std::fs::remove_file(outside).expect("remove outside fixture");
        std::fs::remove_dir_all(root).expect("remove fixture");
    }
}
