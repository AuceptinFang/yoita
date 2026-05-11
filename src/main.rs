use std::process::ExitCode;

use yoita::{Yoita, error::Result, toml};

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

    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "yoita.toml".to_owned());
    let config = toml::load_config(&config_path)?;
    let app = Yoita::from_config(&config)?;
    let layout = app.layout();
    let cached = app.sync(&config).await?;

    tracing::info!(
        config_path = %config_path,
        cache_dir = %layout.cache_dir.display(),
        staging_dir = %layout.staging_dir.display(),
        mount_dir = %layout.mount_dir.display(),
        mods = cached.len(),
        "yoita synced mods"
    );

    for item in &cached {
        tracing::info!(
            name = %item.name,
            bytes = item.bytes,
            archive = %item.archive_path.display(),
            download_url = %item.download_url,
            "cached mod archive"
        );
    }

    Ok(())
}
