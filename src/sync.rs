use anyhow::{Context, Result, bail};
use colored::*;
use std::fs;
use std::io::Write;
use std::path::Path;

use crate::backup::create_backup;
use crate::bitwarden::{ensure_unlocked, get_item, update_item_notes};
use crate::diff::print_diff;
use crate::parser::extract_and_strip_header;

pub fn handle_sync(
    file: &Path,
    explicit_id: Option<String>,
    diff_only: bool,
    dry_run: bool,
    no_backup: bool,
    keep_header: bool,
) -> Result<()> {
    if !file.exists() {
        bail!("Target file does not exist: {}", file.display());
    }

    let local_content = fs::read_to_string(file)
        .with_context(|| format!("Failed to read file: {}", file.display()))?;

    // Extract Bitwarden Item ID and stripped content
    let (extracted_id, stripped_content) = extract_and_strip_header(&local_content);

    let item_id = match explicit_id {
        Some(id) => id,
        None => match extracted_id {
            Some(id) => id,
            None => {
                bail!(
                    "Could not find Bitwarden item ID in header of '{}'.\n\
                     Please add a header comment (e.g. '# bw_id: <UUID>' or '// bw_id: <UUID>') \
                     to the first lines of the file, or specify '--id <UUID>'.",
                    file.display()
                );
            }
        },
    };

    let target_content = if keep_header {
        &local_content
    } else {
        &stripped_content
    };

    // Authenticate / check lock
    let env_session = std::env::var("BW_SESSION").ok();
    let session = ensure_unlocked(env_session.as_deref())?;
    let active_session = session.as_deref().or(env_session.as_deref());

    println!("🔍 Fetching Bitwarden item {}...", item_id.cyan().bold());
    let item = get_item(&item_id, active_session)?;
    let item_name = item
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown Item")
        .to_string();

    let old_notes = item
        .get("notes")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    // Check if contents are identical
    if old_notes == *target_content {
        println!(
            "{}",
            format!(
                "✓ File '{}' and Bitwarden item '{}' notes are already identical. Nothing to sync.",
                file.display(),
                item_name
            )
            .green()
        );
        return Ok(());
    }

    // Display diff if requested or during dry run
    if diff_only || dry_run {
        println!("\n{}", "Differences detected:".bold());
        print_diff(&old_notes, target_content, &file.display().to_string());
    }

    if diff_only {
        return Ok(());
    }

    if dry_run {
        println!(
            "\n{}",
            "⚡ [Dry Run] Changes not applied to Bitwarden."
                .yellow()
                .bold()
        );
        return Ok(());
    }

    // Create backup before overwrite
    if !no_backup && !old_notes.is_empty() {
        let backup_path = create_backup(&item_id, &old_notes)?;
        println!(
            "📦 Saved backup of previous notes to: {}",
            backup_path.display().to_string().dimmed()
        );
    }

    // Push update to Bitwarden
    print!("🚀 Updating Bitwarden item '{}'...", item_name);
    std::io::stdout().flush().ok();
    update_item_notes(&item_id, item, target_content, active_session)?;
    println!(" {}", "Done!".green().bold());

    if !keep_header {
        println!(
            "{}",
            "ℹ️ Stripped bw_id header line before saving to Bitwarden notes.".dimmed()
        );
    }

    println!(
        "\n{}",
        format!(
            "✓ Successfully synced '{}' -> Bitwarden item '{}' ({})",
            file.display(),
            item_name,
            item_id
        )
        .green()
        .bold()
    );

    Ok(())
}
