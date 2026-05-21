use std::{fs, path::Path};

use crate::{
    config::{RawYoitaConfig, YoitaConfig},
    error::{ConfigOrigin, Result, TomlConfigError},
};

pub fn load_config(path: impl AsRef<Path>) -> Result<YoitaConfig> {
    let path = path.as_ref();
    let content = fs::read_to_string(path).map_err(|source| TomlConfigError::read(path, source))?;

    parse_config_with_origin(&content, ConfigOrigin::file(path)).map_err(Into::into)
}

pub fn parse_config(input: &str) -> std::result::Result<YoitaConfig, TomlConfigError> {
    parse_config_with_origin(input, ConfigOrigin::Inline)
}

fn parse_config_with_origin(
    input: &str,
    origin: ConfigOrigin,
) -> std::result::Result<YoitaConfig, TomlConfigError> {
    let raw: RawYoitaConfig = toml::from_str(input)
        .map_err(|source| TomlConfigError::parse(origin.clone(), input, source))?;

    raw.try_into()
        .map_err(|source| TomlConfigError::validation(origin, source))
}

#[cfg(test)]
mod tests {
    use crate::{
        config::{ModSource, SteamLoginConfig},
        error::TomlConfigError,
    };

    #[test]
    fn parses_steam_id_shorthand_and_defaults() {
        let input = r#"
            [mods]
            edit-always = {}
            wanddbg = "2572385079"
        "#;

        let config = crate::toml::parse_config(input).unwrap();
        assert_eq!(config.mods.len(), 2);
        assert_eq!(config.mods[0].name, "edit-always");
        assert!(matches!(
            &config.mods[0].source,
            ModSource::Steam { id } if id == "edit-always"
        ));
        assert_eq!(config.mods[1].version, None);
        assert!(matches!(
            &config.mods[1].source,
            ModSource::Steam { id } if id == "2572385079"
        ));
    }

    #[test]
    fn parses_custom_mod_when_url_is_explicit() {
        let input = r#"
            [mods]
            noita-utility-box = { kind = "custom", url = "https://example.invalid/noita/mods/example-custom-mod.zip" }
        "#;

        let config = crate::toml::parse_config(input).unwrap();
        assert_eq!(config.mods.len(), 1);
        assert!(matches!(config.mods[0].source, ModSource::Custom { .. }));
    }

    #[test]
    fn parses_steam_mod_id_inline_table() {
        let input = r#"
            [mods]
            wanddbg = { id = "2572385079" }
        "#;

        let config = crate::toml::parse_config(input).unwrap();
        assert_eq!(config.mods.len(), 1);
        assert!(matches!(
            &config.mods[0].source,
            ModSource::Steam { id } if id == "2572385079"
        ));
    }

    #[test]
    fn rejects_legacy_workshop_id_field() {
        let input = r#"
            [mods]
            wanddbg = { workshop_id = "2572385079" }
        "#;

        let error = crate::toml::parse_config(input).unwrap_err();
        assert!(matches!(error, TomlConfigError::Validation { .. }));
        assert!(error.to_string().contains("workshop_id"));
        assert!(error.to_string().contains("write `id` instead"));
    }

    #[test]
    fn parses_steamcmd_config_defaults() {
        let input = r#"
            [steam]
            backend = "steamcmd"

            [mods]
            edit-always = {}
        "#;

        let config = crate::toml::parse_config(input).unwrap();
        let steam = config.steam.unwrap();

        assert_eq!(steam.app_id, 881100);
        assert_eq!(steam.timeout_secs, 300);
        assert_eq!(steam.steamcmd_path, std::path::PathBuf::from("steamcmd"));
        assert_eq!(
            steam.force_install_dir,
            std::path::PathBuf::from(".yoita/steamcmd")
        );
        assert_eq!(steam.login, SteamLoginConfig::Anonymous);
    }

    #[test]
    fn parses_legacy_mod_array() {
        let input = r#"
            [[mods]]
            name = "wanddbg"
            version = "1.0.0"

            [mods.source]
            kind = "custom"
            url = "https://example.invalid/noita/mods/example-custom-mod.zip"
        "#;

        let config = crate::toml::parse_config(input).unwrap();
        assert_eq!(config.mods.len(), 1);
        assert!(matches!(config.mods[0].source, ModSource::Custom { .. }));
    }

    #[test]
    fn rejects_invalid_custom_url() {
        let input = r#"
            [mods]
            wanddbg = { kind = "custom", url = "not a url" }
        "#;

        let error = crate::toml::parse_config(input).unwrap_err();
        assert!(matches!(error, TomlConfigError::Validation { .. }));
        assert!(error.to_string().contains("mods.wanddbg"));
    }

    #[test]
    fn rejects_account_login_without_password_env() {
        let input = r#"
            [steam]
            login = "account"
            username = "alice"

            [mods]
            wanddbg = {}
        "#;

        let error = crate::toml::parse_config(input).unwrap_err();
        assert!(matches!(error, TomlConfigError::Validation { .. }));
        assert!(error.to_string().contains("steam.password_env"));
    }

    #[test]
    fn surfaces_syntax_location() {
        let input = r#"
            [mods]
            wanddbg = { version = "1.0.0", url = "https://example.invalid/noita/mods/example-custom-mod.zip"
        "#;

        let error = crate::toml::parse_config(input).unwrap_err();
        assert!(matches!(
            error,
            TomlConfigError::Parse {
                location: Some(_),
                ..
            }
        ));
        assert!(error.to_string().contains("line"));
    }
}
