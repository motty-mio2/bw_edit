use anyhow::{Context, Result};
use chrono::Local;
use std::fs;
use std::path::PathBuf;

/// Save previous note content to a timestamped backup file in state dir.
pub fn create_backup(item_id: &str, old_notes: &str) -> Result<PathBuf> {
    let state_dir = dirs::state_dir()
        .unwrap_or_else(|| dirs::home_dir().unwrap().join(".local").join("state"))
        .join("bw_edit")
        .join("backups");

    fs::create_dir_all(&state_dir)
        .with_context(|| format!("Failed to create backup directory at {:?}", state_dir))?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let backup_file = state_dir.join(format!("{}_{}.bak", item_id, timestamp));

    fs::write(&backup_file, old_notes)
        .with_context(|| format!("Failed to write backup file to {:?}", backup_file))?;

    Ok(backup_file)
}
