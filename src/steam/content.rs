use std::path::Path;

use super::WorkshopContentKind;

pub fn content_kind_for_path(path: &Path) -> WorkshopContentKind {
    if path.is_dir() {
        WorkshopContentKind::Directory
    } else {
        WorkshopContentKind::SingleFile
    }
}

#[cfg(test)]
mod tests {
    use super::content_kind_for_path;

    #[test]
    fn detects_single_file_content_kind() {
        let unique = format!("yoita-steam-kind-file-{}", std::process::id());
        let root = std::env::temp_dir().join(unique);
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("mod.xml");
        std::fs::write(&file, b"test").unwrap();

        assert!(matches!(
            content_kind_for_path(&file),
            super::super::WorkshopContentKind::SingleFile
        ));

        std::fs::remove_dir_all(root).unwrap();
    }
}
