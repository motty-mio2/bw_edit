use colored::*;
use similar::{ChangeTag, TextDiff};

/// Print formatted unified diff between old and new contents.
pub fn print_diff(old_text: &str, new_text: &str, file_label: &str) {
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
