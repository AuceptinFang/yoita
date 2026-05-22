use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use yoita::{Yoita, config::YoitaConfig, toml};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn resolve_steamcmd_path() -> PathBuf {
    if let Ok(path) = std::env::var("YOITA_TEST_STEAMCMD_PATH") {
        return PathBuf::from(path);
    }

    let tmp = PathBuf::from("/tmp/steamcmd.sh");
    if tmp.is_file() {
        return tmp;
    }

    PathBuf::from("steamcmd")
}

fn unique_test_root(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("yoita-{name}-{}-{unique}", std::process::id()))
}

fn configure_isolated_workspace(config: &mut YoitaConfig, test_name: &str) -> PathBuf {
    let root = unique_test_root(test_name);
    config.config.cache_dir = root.join("cache");
    config.config.staging_dir = root.join("staging");
    config.config.mount_dir = root.join("mods");

    let steam = config.steam.as_mut().unwrap();
    steam.steamcmd_path = resolve_steamcmd_path();
    steam.force_install_dir = root.join("steamcmd");

    root
}

fn expected_source_path(root: &Path) -> PathBuf {
    root.join("steamcmd")
        .join("steamapps")
        .join("workshop")
        .join("content")
        .join("881100")
        .join("2572385079")
}

#[tokio::test]
#[ignore = "requires steamcmd and network; downloads from scratch into a temp workspace"]
async fn downloads_workshop_item_from_example_config() {
    let mut config = toml::load_config(fixture_path("yoita.download.example.toml")).unwrap();
    let root = configure_isolated_workspace(&mut config, "steamcmd-download-example");
    let source_path = expected_source_path(&root);

    assert!(!source_path.exists());

    let app = Yoita::from_config(&config).unwrap();
    let report = app.sync(&config).await.unwrap();

    assert_eq!(report.mods.len(), 1);

    let synced = &report.mods[0];
    assert_eq!(synced.name, "wanddbg");
    assert_eq!(synced.source_path, source_path);
    assert!(synced.source_path.join("mod.xml").is_file());
    assert!(synced.source_path.join("mod_id.txt").is_file());
    assert_eq!(
        fs::read_to_string(synced.source_path.join("mod_id.txt"))
            .unwrap()
            .trim(),
        "wand_dbg"
    );

    assert!(synced.mount_path.is_dir());
    assert!(synced.mount_path.join("mod.xml").is_file());
    assert!(synced.mount_path.join("mod_id.txt").is_file());

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires steamcmd and network; downloads from scratch into a temp workspace"]
async fn downloads_workshop_item_from_name_config() {
    let mut config = toml::load_config(fixture_path("yoita.name.toml")).unwrap();
    let root = configure_isolated_workspace(&mut config, "steamcmd-download-name");
    let source_path = expected_source_path(&root);

    assert!(!source_path.exists());

    let app = Yoita::from_config(&config).unwrap();
    let report = app.sync(&config).await.unwrap();

    assert_eq!(report.mods.len(), 1);

    let synced = &report.mods[0];
    assert_eq!(synced.name, "wanddbg");
    assert_eq!(synced.source_path, source_path);
    assert!(synced.source_path.join("mod.xml").is_file());
    assert!(synced.source_path.join("mod_id.txt").is_file());
    assert_eq!(
        fs::read_to_string(synced.source_path.join("mod_id.txt"))
            .unwrap()
            .trim(),
        "wand_dbg"
    );

    assert!(synced.mount_path.is_dir());
    assert!(synced.mount_path.join("mod.xml").is_file());
    assert!(synced.mount_path.join("mod_id.txt").is_file());

    fs::remove_dir_all(root).unwrap();
}
