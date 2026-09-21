use anyhow::{bail, Context, Result};
use base64::prelude::*;
use chrono::Local;
use clap::{Parser, Subcommand};
use colored::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Parser)]
#[command(
    name = "bw_edit",
    version,
    about = "Sync local files directly to Bitwarden item notes"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Sync a local file to its corresponding Bitwarden item note
    Sync {
        /// Path to the local file to sync
        file: PathBuf,

        /// Explicitly specify the Bitwarden item UUID instead of reading from header
        #[arg(long)]
        id: Option<String>,

        /// Preview changes as unified diff without syncing
        #[arg(long)]
        diff: bool,

        /// Do not apply changes to Bitwarden (dry run)
        #[arg(long)]
        dry_run: bool,

        /// Skip creating a backup of the previous note content
        #[arg(long)]
        no_backup: bool,

        /// Keep the bw_id header in Bitwarden notes instead of stripping it
        #[arg(long)]
        keep_header: bool,
    },
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct BwStatus {
    server_url: Option<String>,
    user_email: Option<String>,
    user_id: Option<String>,
    status: String,
}

/// Extract Bitwarden item UUID from the first few lines and return the stripped content (with header line removed).
fn extract_and_strip_header(content: &str) -> (Option<String>, String) {
    let re_bw_id = Regex::new(
        r"(?i)\bbw(?:_id)?\s*[:=]\s*([a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})\b",
    )
    .unwrap();

    let re_chezmoi = Regex::new(
        r#"(?i)bitwarden\s+"item"\s+"([a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12})""#,
    )
    .unwrap();

    let lines: Vec<&str> = content.lines().collect();
    let mut item_id = None;
    let mut header_indices = Vec::new();

    for (idx, line) in lines.iter().take(15).enumerate() {
        if let Some(caps) = re_bw_id.captures(line) {
            if let Some(m) = caps.get(1) {
                if item_id.is_none() {
                    item_id = Some(m.as_str().to_string());
                }
                header_indices.push(idx);
            }
        } else if let Some(caps) = re_chezmoi.captures(line) {
            if let Some(m) = caps.get(1) {
                if item_id.is_none() {
                    item_id = Some(m.as_str().to_string());
                }
                header_indices.push(idx);
            }
        }
    }

    let stripped = if !header_indices.is_empty() {
        let remaining_lines: Vec<&str> = lines
            .into_iter()
            .enumerate()
            .filter_map(|(i, l)| {
                if !header_indices.contains(&i) {
                    Some(l)
                } else {
                    None
                }
            })
            .collect();
        let mut res = remaining_lines.join("\n");
        if content.ends_with('\n') && !res.is_empty() {
            res.push('\n');
        }
        res
    } else {
        content.to_string()
    };

    (item_id, stripped)
}

/// Check Bitwarden vault status and unlock interactively if needed, returning an active session key if unlocked.
fn ensure_unlocked(existing_session: Option<&str>) -> Result<Option<String>> {
    let mut cmd = Command::new("bw");
    cmd.arg("status");
    if let Some(session) = existing_session {
        cmd.arg("--session").arg(session);
    }

    let output = cmd
        .output()
        .context("Failed to execute 'bw status'. Is Bitwarden CLI installed and in your PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("'bw status' failed: {}", stderr.trim());
    }

    let status_str = String::from_utf8_lossy(&output.stdout);
    let bw_status: BwStatus = serde_json::from_str(&status_str)
        .with_context(|| format!("Failed to parse 'bw status' output: {}", status_str))?;

    match bw_status.status.as_str() {
        "unlocked" => {
            // Already unlocked
            Ok(existing_session.map(|s| s.to_string()))
        }
        "unauthenticated" => {
            bail!("Bitwarden is not logged in. Please run 'bw login' first.");
        }
        "locked" => {
            println!("{}", "🔑 Bitwarden vault is locked.".yellow());
            let password = rpassword::prompt_password("Enter Bitwarden master password: ")
                .context("Failed to read master password")?;

            let mut unlock_cmd = Command::new("bw");
            unlock_cmd.args(["unlock", &password, "--raw"]);

            let unlock_output = unlock_cmd
                .output()
                .context("Failed to execute 'bw unlock'")?;

            if !unlock_output.status.success() {
                let stderr = String::from_utf8_lossy(&unlock_output.stderr);
                bail!("Failed to unlock Bitwarden vault: {}", stderr.trim());
            }

            let session = String::from_utf8_lossy(&unlock_output.stdout)
                .trim()
                .to_string();
            println!("{}", "✓ Bitwarden vault unlocked successfully.".green());
            Ok(Some(session))
        }
        other => {
            bail!("Unexpected Bitwarden status: '{}'", other);
        }
    }
}

/// Fetch item JSON from Bitwarden.
fn get_item(item_id: &str, session: Option<&str>) -> Result<Value> {
    let mut cmd = Command::new("bw");
    cmd.args(["get", "item", item_id]);
    if let Some(s) = session {
        cmd.arg("--session").arg(s);
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to fetch Bitwarden item '{}'", item_id))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Failed to get item '{}': {}", item_id, stderr.trim());
    }

    let stdout_str = String::from_utf8_lossy(&output.stdout);
    let item: Value = serde_json::from_str(&stdout_str)
        .with_context(|| format!("Failed to parse item JSON for '{}'", item_id))?;

    Ok(item)
}

/// Save previous note content to a timestamped backup file in state dir.
fn create_backup(item_id: &str, old_notes: &str) -> Result<PathBuf> {
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

/// Update item notes in Bitwarden.
fn update_item_notes(
    item_id: &str,
    mut item: Value,
    new_notes: &str,
    session: Option<&str>,
) -> Result<()> {
    item["notes"] = Value::String(new_notes.to_string());
    let json_str = serde_json::to_string(&item).context("Failed to serialize modified item JSON")?;
    let encoded = BASE64_STANDARD.encode(json_str.as_bytes());

    let mut cmd = Command::new("bw");
    cmd.args(["edit", "item", item_id]);
    if let Some(s) = session {
        cmd.arg("--session").arg(s);
    }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .with_context(|| format!("Failed to spawn 'bw edit item {}'", item_id))?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(encoded.as_bytes())
            .context("Failed to write encoded JSON to bw stdin")?;
    }

    let output = child
        .wait_with_output()
        .context("Failed to wait for 'bw edit' process")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("'bw edit item' failed: {}", stderr.trim());
    }

    Ok(())
}

/// Print formatted unified diff between old and new contents.
fn print_diff(old_text: &str, new_text: &str, file_label: &str) {
    let diff = TextDiff::from_lines(old_text, new_text);

    println!(
        "{} {}",
        "--- Bitwarden Remote Notes:".bold().red(),
        file_label
    );
    println!(
        "{} {}",
        "+++ Content to Sync:       ".bold().green(),
        file_label
    );

    for change in diff.iter_all_changes() {
        let sign = match change.tag() {
            ChangeTag::Delete => "-".red(),
            ChangeTag::Insert => "+".green(),
            ChangeTag::Equal => " ".normal(),
        };
        print!("{}{}", sign, change);
    }
}

fn handle_sync(
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

    println!(
        "🔍 Fetching Bitwarden item {}...",
        item_id.cyan().bold()
    );
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
            "⚡ [Dry Run] Changes not applied to Bitwarden.".yellow().bold()
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Sync {
            file,
            id,
            diff,
            dry_run,
            no_backup,
            keep_header,
        } => handle_sync(&file, id, diff, dry_run, no_backup, keep_header),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_and_strip_hash_comment() {
        let content = "# bw_id: e71b158e-c584-4de3-8ae1-b003011c67d4\nHost foo\n  HostName foo.com\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, Some("e71b158e-c584-4de3-8ae1-b003011c67d4".to_string()));
        assert_eq!(stripped, "Host foo\n  HostName foo.com\n");
    }

    #[test]
    fn test_extract_and_strip_slash_comment() {
        let content = "// bw_id: 418f2f48-2d8e-495b-bc44-b17500e9aa1b\n{\n  \"key\": \"value\"\n}\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, Some("418f2f48-2d8e-495b-bc44-b17500e9aa1b".to_string()));
        assert_eq!(stripped, "{\n  \"key\": \"value\"\n}\n");
    }

    #[test]
    fn test_extract_and_strip_dash_comment() {
        let content = "-- bw_id: 7540fa95-a318-49a3-b140-b17500e9b87e\nreturn { theme = 'tokyonight' }\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, Some("7540fa95-a318-49a3-b140-b17500e9b87e".to_string()));
        assert_eq!(stripped, "return { theme = 'tokyonight' }\n");
    }

    #[test]
    fn test_extract_and_strip_chezmoi_template() {
        let content = "{{ if lookPath \"bw\" }}\n{{ (bitwarden \"item\" \"e71b158e-c584-4de3-8ae1-b003011c67d4\").notes }}\n{{ end }}\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, Some("e71b158e-c584-4de3-8ae1-b003011c67d4".to_string()));
        assert_eq!(stripped, "{{ if lookPath \"bw\" }}\n{{ end }}\n");
    }

    #[test]
    fn test_extract_and_strip_duplicated_headers() {
        let content = "# bw_id: e71b158e-c584-4de3-8ae1-b003011c67d4\n# bw_id: e71b158e-c584-4de3-8ae1-b003011c67d4\nHost foo\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, Some("e71b158e-c584-4de3-8ae1-b003011c67d4".to_string()));
        assert_eq!(stripped, "Host foo\n");
    }

    #[test]
    fn test_extract_and_strip_none() {
        let content = "Host foo\n  HostName foo.com\n  User test\n";
        let (id, stripped) = extract_and_strip_header(content);
        assert_eq!(id, None);
        assert_eq!(stripped, content);
    }
}
