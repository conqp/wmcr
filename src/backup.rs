use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{fs, io};

use log::info;

pub fn logs(server_dir: &Path) -> io::Result<()> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let backup_dir = server_dir
        .join("backups")
        .join(format!("crash-logs-{stamp}"));
    fs::create_dir_all(&backup_dir)?;

    let crash_reports = server_dir.join("crash-reports");
    if crash_reports.is_dir() {
        let destination = backup_dir.join("crash-reports");
        for entry in fs::read_dir(&crash_reports)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                fs::create_dir_all(&destination)?;
                fs::copy(entry.path(), destination.join(entry.file_name()))?;
            }
        }
    }

    let latest_log = server_dir.join("logs").join("latest.log");
    if latest_log.is_file() {
        fs::copy(latest_log, backup_dir.join("latest.log"))?;
    }

    info!("Crash logs saved to {}", backup_dir.display());
    Ok(())
}
