use anyhow::Result;
use clap::Parser;

fn main() -> Result<()> {
    let cli = bw_edit::cli::Cli::parse();

    match cli.command {
        bw_edit::cli::Commands::Sync {
            file,
            id,
            diff,
            dry_run,
            no_backup,
            keep_header,
        } => bw_edit::sync::handle_sync(&file, id, diff, dry_run, no_backup, keep_header),
    }
}
