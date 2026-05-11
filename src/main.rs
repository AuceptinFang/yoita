use std::process::ExitCode;

use yoita::{Yoita, config::ModSource, error::Result, toml};

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error:#}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<()> {
    tracing_subscriber::fmt().init();

    let config = toml::load_config("yoita.toml")?;
    let app = Yoita::from_config(&config)?;
    let mods = app.enabled_mods(&config)?;
    let layout = app.layout();

    tracing::info!(
        cache_dir = %layout.cache_dir.display(),
        staging_dir = %layout.staging_dir.display(),
        mount_dir = %layout.mount_dir.display(),
        mods = mods.len(),
        "yoita ready. Now start working!"
    );

    for item in &mods {
        let download_url = app.download_url(item)?;

        match &item.source {
            ModSource::Steam { workshop_id } => tracing::info!(
                name = %item.name,
                version = ?item.version,
                workshop_id = %workshop_id,
                download_url = %download_url,
                "planned steam mod"
            ),
            ModSource::Custom { .. } => tracing::info!(
                name = %item.name,
                version = ?item.version,
                download_url = %download_url,
                "planned custom mod"
            ),
        }
    }

    Ok(())
}
