//! Launches a Java server process and saves its logs before restarting after a failure.

use std::error::Error;
use std::process::Command;
use std::time::Duration;
use std::{env, thread};

use log::{error, info, warn};

mod backup;

const RESTART_DELAY: Duration = Duration::from_secs(5);

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    if let Err(error) = run() {
        error!("wmcr: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let java_args: Vec<_> = env::args_os().skip(1).collect();
    let server_dir = env::current_exe()?
        .parent()
        .ok_or("could not determine the launcher directory")?
        .to_path_buf();

    if !server_dir.is_dir() {
        return Err(format!("server directory does not exist: {}", server_dir.display()).into());
    }
    info!("Starting Minecraft server in {}", server_dir.display());
    loop {
        let status = Command::new("java")
            .args(&java_args)
            .current_dir(&server_dir)
            .status()?;

        if status.success() {
            info!("Minecraft server stopped normally.");
            break;
        }

        warn!("Minecraft server exited with {status}; saving logs before restart.");
        if let Err(error) = backup::logs(&server_dir) {
            error!("Could not save all crash logs: {error}");
        }
        info!(
            "Restarting in {} seconds. Press Ctrl+C to stop.",
            RESTART_DELAY.as_secs()
        );
        thread::sleep(RESTART_DELAY);
    }

    Ok(())
}
