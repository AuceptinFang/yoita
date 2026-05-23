use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use yoita::state::SyncState;

const WORKSHOP_ID: &str = "2572385079";
const MOD_ID: &str = "wand_dbg";

struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(name: &str) -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path =
            std::env::temp_dir().join(format!("yoita-{name}-{}-{unique}", std::process::id()));
        fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

fn resolve_steamcmd_path() -> PathBuf {
    if let Ok(path) = env::var("YOITA_TEST_STEAMCMD_PATH") {
        return absolutize_program_path(PathBuf::from(path));
    }

    let tmp = PathBuf::from("/tmp/steamcmd.sh");
    if tmp.is_file() {
        return tmp;
    }

    resolve_program_on_path("steamcmd").unwrap_or_else(|| PathBuf::from("steamcmd"))
}

fn absolutize_program_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() || path.components().count() > 1 {
        path
    } else {
        let cwd_candidate = std::env::current_dir().unwrap().join(&path);
        if cwd_candidate.is_file() {
            cwd_candidate
        } else {
            resolve_program_on_path(path.to_string_lossy().as_ref()).unwrap_or(path)
        }
    }
}

fn resolve_program_on_path(program: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    let names = candidate_program_names(program);

    for dir in env::split_paths(&path_var) {
        for name in &names {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}

fn candidate_program_names(program: &str) -> Vec<String> {
    if Path::new(program).extension().is_some() {
        return vec![program.to_owned()];
    }

    if !cfg!(windows) {
        return vec![program.to_owned()];
    }

    let mut names = vec![program.to_owned()];
    let exts = env::var("PATHEXT")
        .ok()
        .map(|raw| {
            raw.split(';')
                .map(str::trim)
                .filter(|ext| !ext.is_empty())
                .map(|ext| ext.to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|exts| !exts.is_empty())
        .unwrap_or_else(|| {
            vec![
                ".COM".to_owned(),
                ".EXE".to_owned(),
                ".BAT".to_owned(),
                ".CMD".to_owned(),
            ]
        });

    names.extend(exts.into_iter().map(|ext| format!("{program}{ext}")));
    names
}

fn config_path(root: &Path) -> PathBuf {
    root.join("yoita.toml")
}

fn force_install_dir(root: &Path) -> PathBuf {
    root.join("steam-downloads")
}

fn mount_dir(root: &Path) -> PathBuf {
    root.join("mods")
}

fn state_path(root: &Path) -> PathBuf {
    root.join(".yoita").join("state.toml")
}

fn expected_source_path(root: &Path) -> PathBuf {
    force_install_dir(root)
        .join("steamapps")
        .join("workshop")
        .join("content")
        .join("881100")
        .join(WORKSHOP_ID)
}

fn render_config(steamcmd_path: &Path, explicit_enabled: bool) -> String {
    let explicit_line = if explicit_enabled {
        format!("wanddbg-explicit = {{ id = '{WORKSHOP_ID}' }}")
    } else {
        format!("wanddbg-explicit = {{ id = '{WORKSHOP_ID}', enabled = false }}")
    };

    format!(
        r#"[config]
mount_dir = 'mods'

[steam]
steamcmd_path = '{steamcmd_path}'
force_install_dir = 'steam-downloads'

[mods]
wanddbg = {{}}
{explicit_line}
ignored-disabled = {{ id = '{WORKSHOP_ID}', enabled = false }}
"#,
        steamcmd_path = steamcmd_path.display(),
    )
}

fn write_config(root: &Path, steamcmd_path: &Path, explicit_enabled: bool) {
    fs::write(
        config_path(root),
        render_config(steamcmd_path, explicit_enabled),
    )
    .unwrap();
}

fn read_mod_id(path: &Path) -> String {
    fs::read_to_string(path.join("mod_id.txt"))
        .unwrap()
        .trim()
        .to_owned()
}

fn run_yoita(root: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_yoita"))
        .arg("yoita.toml")
        .current_dir(root)
        .output()
        .unwrap_or_else(|source| panic!("failed to run yoita binary: {source}"));

    if !output.status.success() {
        panic!(
            "yoita exited with {status}\nstdout:\n{stdout}\nstderr:\n{stderr}",
            status = output.status,
            stdout = String::from_utf8_lossy(&output.stdout),
            stderr = String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
#[ignore = "requires steamcmd and network; performs a real end-to-end sync in a temp workspace"]
fn syncs_real_workshop_mods_end_to_end_and_cleans_up_removed_mounts() {
    let root = TempDirGuard::new("steamcmd-e2e");
    let steamcmd_path = resolve_steamcmd_path();
    let source_path = expected_source_path(root.path());
    let explicit_mount_path = mount_dir(root.path()).join("wanddbg-explicit");
    let search_mount_path = mount_dir(root.path()).join("wanddbg");
    let disabled_mount_path = mount_dir(root.path()).join("ignored-disabled");

    write_config(root.path(), &steamcmd_path, true);

    assert!(!source_path.exists());
    assert!(!search_mount_path.exists());
    assert!(!explicit_mount_path.exists());
    assert!(!disabled_mount_path.exists());

    run_yoita(root.path());

    assert!(source_path.is_dir());
    assert!(search_mount_path.join("mod.xml").is_file());
    assert_eq!(read_mod_id(&search_mount_path), MOD_ID);
    assert!(explicit_mount_path.join("mod.xml").is_file());
    assert_eq!(read_mod_id(&explicit_mount_path), MOD_ID);
    assert!(!disabled_mount_path.exists());

    let first_state = SyncState::load(&state_path(root.path())).unwrap();
    assert_eq!(first_state.mods.len(), 2);
    assert_eq!(first_state.mods["wanddbg"].source_kind, "steam");
    assert_eq!(first_state.mods["wanddbg"].source_id, "wanddbg");
    assert_eq!(first_state.mods["wanddbg"].source_path, source_path);
    assert_eq!(
        first_state.mods["wanddbg"].mount_path,
        PathBuf::from("mods/wanddbg")
    );
    assert_eq!(first_state.mods["wanddbg-explicit"].source_kind, "steam");
    assert_eq!(first_state.mods["wanddbg-explicit"].source_id, WORKSHOP_ID);
    assert_eq!(
        first_state.mods["wanddbg-explicit"].source_path,
        source_path
    );
    assert_eq!(
        first_state.mods["wanddbg-explicit"].mount_path,
        PathBuf::from("mods/wanddbg-explicit")
    );
    assert!(!first_state.mods.contains_key("ignored-disabled"));

    write_config(root.path(), &steamcmd_path, false);
    run_yoita(root.path());

    assert!(search_mount_path.is_dir());
    assert!(!explicit_mount_path.exists());

    let second_state = SyncState::load(&state_path(root.path())).unwrap();
    assert_eq!(second_state.mods.len(), 1);
    assert!(second_state.mods.contains_key("wanddbg"));
    assert!(!second_state.mods.contains_key("wanddbg-explicit"));
    assert!(!second_state.mods.contains_key("ignored-disabled"));

    let root_path = root.path().to_path_buf();
    drop(root);
    assert!(!root_path.exists());
}
