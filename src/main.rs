//! Launches a Java server process and saves its logs before restarting after a failure.

use std::env;
use std::error::Error;
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use log::{error, info, warn};

mod backup;

const RESTART_DELAY: Duration = Duration::from_secs(5);
const MIN_SERVER_UPTIME: Duration = Duration::from_mins(5);

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_secs()
        .init();
    if let Err(error) = run() {
        error!("wmcr: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let (interrupt_tx, interrupt_rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = interrupt_tx.send(());
    })?;

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
        if interrupt_rx.try_recv().is_ok() {
            info!("Ctrl+C received; stopping launcher.");
            break;
        }

        let started_at = Instant::now();
        let status = Command::new("java")
            .args(&java_args)
            .current_dir(&server_dir)
            .status()?;
        let uptime = started_at.elapsed();

        if interrupt_rx.try_recv().is_ok() {
            info!("Ctrl+C received; Minecraft server stopped.");
            break;
        }

        if status.success() {
            info!("Minecraft server stopped normally.");
            break;
        }

        warn!("Minecraft server exited with {status}; saving logs before restart.");
        if let Err(error) = backup::logs(&server_dir) {
            error!("Could not save all crash logs: {error}");
        }

        if uptime < MIN_SERVER_UPTIME {
            return Err(format!(
                "Minecraft server failed after running for {:.1} seconds (less than the {:.0}-minute minimum); stopping instead of restarting",
                uptime.as_secs_f64(),
                MIN_SERVER_UPTIME.as_secs_f64() / 60.0
            )
            .into());
        }

        info!(
            "Restarting in {} seconds. Press Ctrl+C to stop.",
            RESTART_DELAY.as_secs()
        );
        if interrupt_rx.recv_timeout(RESTART_DELAY).is_ok() {
            info!("Ctrl+C received; stopping launcher.");
            break;
        }
    }

    Ok(())
}
