//! Launches a Java server process and saves its logs before restarting after a failure.

use std::env;
use std::error::Error;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
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
    let java_args: Vec<_> = env::args_os().skip(1).collect();
    let server_dir = server_directory()?;
    let interrupt_rx = install_interrupt_handler()?;
    let input_rx = forward_console_input();

    info!("Starting Minecraft server in {}", server_dir.display());
    let mut stop_requested = false;
    while !stop_requested && interrupt_rx.try_recv().is_err() {
        let started_at = Instant::now();
        let child = spawn_java(&java_args, &server_dir)?;
        let (status, requested_stop) = wait_for_java(child, &interrupt_rx, &input_rx)?;
        stop_requested |= requested_stop;

        if stop_requested {
            info!("Minecraft server stopped after the stop command.");
            break;
        }
        let uptime = started_at.elapsed();

        if status.success() {
            info!("Minecraft server stopped normally.");
            break;
        }

        handle_failed_exit(&server_dir, status, uptime)?;
    }

    Ok(())
}

fn server_directory() -> Result<std::path::PathBuf, Box<dyn Error>> {
    let server_dir = env::current_exe()?
        .parent()
        .ok_or("could not determine the launcher directory")?
        .to_path_buf();

    if !server_dir.is_dir() {
        return Err(format!("server directory does not exist: {}", server_dir.display()).into());
    }
    Ok(server_dir)
}

fn install_interrupt_handler() -> Result<Receiver<()>, Box<dyn Error>> {
    let (interrupt_tx, interrupt_rx) = mpsc::channel();
    ctrlc::set_handler(move || {
        let _ = interrupt_tx.send(());
    })?;
    Ok(interrupt_rx)
}

fn forward_console_input() -> Receiver<String> {
    let (input_tx, input_rx) = mpsc::channel();
    thread::spawn(move || {
        for line in io::stdin().lock().lines() {
            match line {
                Ok(line) => {
                    if input_tx.send(line).is_err() {
                        break;
                    }
                }
                Err(error) => {
                    error!("Could not read server console input: {error}");
                    break;
                }
            }
        }
    });
    input_rx
}

fn spawn_java(java_args: &[std::ffi::OsString], server_dir: &Path) -> Result<Child, io::Error> {
    let mut command = Command::new("java");
    command
        .args(java_args)
        .current_dir(server_dir)
        .stdin(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;

        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        command.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }

    command.spawn()
}

fn wait_for_java(
    mut child: Child,
    interrupt_rx: &Receiver<()>,
    input_rx: &Receiver<String>,
) -> Result<(ExitStatus, bool), Box<dyn Error>> {
    let mut child_stdin = child
        .stdin
        .take()
        .ok_or("could not open Minecraft server console input")?;

    loop {
        if interrupt_rx.try_recv().is_ok() {
            info!("Ctrl+C received; sending the stop command to Minecraft.");
            if let Err(error) = writeln!(child_stdin, "stop") {
                warn!("Could not send the stop command to Minecraft: {error}");
            }
            if let Err(error) = child_stdin.flush() {
                warn!("Could not flush the stop command to Minecraft: {error}");
            }
            return Ok((child.wait()?, true));
        }

        while let Ok(line) = input_rx.try_recv() {
            writeln!(child_stdin, "{line}")?;
        }

        if let Some(status) = child.try_wait()? {
            return Ok((status, false));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn handle_failed_exit(
    server_dir: &Path,
    status: ExitStatus,
    uptime: Duration,
) -> Result<(), Box<dyn Error>> {
    warn!("Minecraft server exited with {status}; saving logs before restart.");
    if let Err(error) = backup::logs(server_dir) {
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

    info!("Restarting in {} seconds.", RESTART_DELAY.as_secs());
    thread::sleep(RESTART_DELAY);
    Ok(())
}
