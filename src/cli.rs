use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "bw_edit",
    version,
    about = "Sync local files directly to Bitwarden item notes"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
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
