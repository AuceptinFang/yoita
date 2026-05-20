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
    let report = app.sync(&config).await?;

    tracing::info!(
        config_path = %config_path,
        state_path = %report.state_path.display(),
        cache_dir = %layout.cache_dir.display(),
        staging_dir = %layout.staging_dir.display(),
        mount_dir = %layout.mount_dir.display(),
        synced = report.mods.len(),
        removed = report.removed_mounts.len(),
        "yoita sync completed"
    );

    for item in &report.mods {
        tracing::info!(
            name = %item.name,
            source = %item.source_path.display(),
            mount = %item.mount_path.display(),
            bytes = item.bytes,
            "synced mod"
        );
    }

    for path in &report.removed_mounts {
        tracing::info!(mount = %path.display(), "removed stale mount");
    }

    Ok(())
}
