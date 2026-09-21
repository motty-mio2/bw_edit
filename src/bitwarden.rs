use anyhow::{bail, Context, Result};
use base64::prelude::*;
use colored::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::Write;
use std::process::{Command, Stdio};

#[derive(Deserialize, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BwStatus {
    pub server_url: Option<String>,
    pub user_email: Option<String>,
    pub user_id: Option<String>,
    pub status: String,
}

/// Check Bitwarden vault status and unlock interactively if needed, returning an active session key if unlocked.
pub fn ensure_unlocked(existing_session: Option<&str>) -> Result<Option<String>> {
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
pub fn get_item(item_id: &str, session: Option<&str>) -> Result<Value> {
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

/// Update item notes in Bitwarden.
pub fn update_item_notes(
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
