pub mod loader;

pub use loader::{
    GameDefError, GameDefinition, LaunchConfig, Metadata, ValidationConfig, load_all_from_dir,
    load_from_path, load_from_str,
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn sanity_load_sfiii3_example() {
        let toml = r#"
schema_version = 1
id = "sfiii3"
name = "Street Fighter III: 3rd Strike"
emulator = "fbneo"
[launch]
args = ["-rom", "{rom}", "-window"]
[validation]
required_files = ["sfiii3.zip", "neogeo.zip"]
bios = "neogeo.zip"
[metadata]
year = 1999
developer = "Capcom"
players = 2
"#;
        let def = load_from_str(toml, "sfiii3.toml").expect("valid sfiii3");
        assert_eq!(def.id, "sfiii3");
        assert_eq!(def.metadata.year, Some(1999));
        let rendered = def.render_args(Path::new("/roms/sfiii3.zip"));
        assert!(rendered.contains(&"/roms/sfiii3.zip".to_string()));
    }
}
