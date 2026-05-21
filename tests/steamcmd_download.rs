use std::{
    fs,
    path::{Path, PathBuf},
};

use yoita::{Yoita, toml};

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

#[tokio::test]
//#[ignore = "requires steamcmd and network; preserves files under tests/.artifacts/"]
async fn downloads_workshop_item_from_example_config() {
    let mut config = toml::load_config(fixture_path("yoita.download.example.toml")).unwrap();
    let steam = config.steam.as_mut().unwrap();
    steam.steamcmd_path = resolve_steamcmd_path();

    let app = Yoita::from_config(&config).unwrap();
    let report = app.sync(&config).await.unwrap();

    assert_eq!(report.mods.len(), 1);

    let synced = &report.mods[0];
    assert_eq!(synced.name, "wanddbg");
    assert!(
        synced
            .source_path
            .ends_with(Path::new("steamapps/workshop/content/881100/2572385079"))
    );
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
}

#[tokio::test]
async fn downloads_workshop_item_from_name_config() {
    let mut config = toml::load_config(fixture_path("yoita.name.toml")).unwrap();
    let steam = config.steam.as_mut().unwrap();
    steam.steamcmd_path = resolve_steamcmd_path();

    let app = Yoita::from_config(&config).unwrap();
    let report = app.sync(&config).await.unwrap();

    assert_eq!(report.mods.len(), 1);

    let synced = &report.mods[0];
    assert_eq!(synced.name, "wanddbg");
    assert!(
        synced
            .source_path
            .ends_with(Path::new("steamapps/workshop/content/881100/2572385079"))
    );
    assert!(synced.source_path.join("mod.xml").is_file());
    assert!(synced.source_path.join("mod_id.txt").is_file());
    assert_eq!(
        fs::read_to_string(synced.source_path.join("mod_id.txt"))
            .unwrap()
            .trim(),
        "wand_dbg"
    );
}
