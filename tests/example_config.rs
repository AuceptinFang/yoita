use std::path::PathBuf;

use yoita::{
    Yoita,
    config::{ModSource, SteamLoginConfig},
    toml,
};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

#[test]
fn loads_example_config_from_tests_fixture() {
    let config = toml::load_config(fixture_path("yoita.example.toml")).unwrap();

    assert_eq!(config.config.cache_dir, PathBuf::from(".yoita/cache"));
    assert_eq!(config.config.staging_dir, PathBuf::from(".yoita/staging"));
    assert_eq!(config.config.mount_dir, PathBuf::from("./mods"));

    let steam = config.steam.as_ref().unwrap();
    assert_eq!(steam.app_id, 881100);
    assert_eq!(steam.login, SteamLoginConfig::Anonymous);

    assert_eq!(config.mods.len(), 3);
    assert!(matches!(
        &config.mods[0].source,
        ModSource::Steam { workshop_id } if workshop_id == "2572385079"
    ));
    assert!(!config.mods[1].enabled);
    assert!(matches!(config.mods[2].source, ModSource::Custom { .. }));

    let app = Yoita::from_config(&config).unwrap();
    let enabled = app.enabled_mods(&config).unwrap();

    assert_eq!(enabled.len(), 2);
    assert_eq!(enabled[0].name, "wanddbg");
    assert_eq!(enabled[1].name, "custom-pack");
}
