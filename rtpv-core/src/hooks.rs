//! Hooks — executable scripts run on store events.
//!
//! Drop scripts in `~/.config/rtpv/hooks/`:
//!   post-add.sh
//!   post-edit.sh
//!   post-rotate.sh
//!   post-sync.sh
//!   post-delete.sh
//!
//! Scripts receive the entry path as $1.
//! They run synchronously — keep them fast.

use std::path::Path;

/// Run all hooks for an event, passing `entry_path` as the first argument.
/// Failures are silently swallowed — hooks should not break normal operations.
pub fn run(hooks_dir: &Path, event: &str, entry_path: &str) {
    for ext in &["sh", "bash"] {
        let hook = hooks_dir.join(format!("{}.{}", event, ext));
        if hook.exists() {
            let _ = std::process::Command::new("bash")
                .arg(&hook)
                .arg(entry_path)
                .status();
        }
    }
}

/// Run hook with no arguments (e.g. post-sync).
pub fn run_bare(hooks_dir: &Path, event: &str) {
    run(hooks_dir, event, "");
}

/// List all installed hook scripts.
pub fn list(hooks_dir: &Path) -> Vec<String> {
    if !hooks_dir.exists() { return vec![]; }
    walkdir::WalkDir::new(hooks_dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect()
}
