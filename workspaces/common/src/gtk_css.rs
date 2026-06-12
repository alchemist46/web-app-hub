//! Manages a small block in the user's GTK config so that web apps launched in
//! Chromium `--app` mode get a title bar that matches the native GTK header bar
//! height.
//!
//! Chromium draws its own client-side title bar for `--app` windows. With a GTK
//! theme applied (see `--gtk-version=4` in the desktop file templates) it looks
//! native, but the height is shorter than a libadwaita `HeaderBar` unless an
//! explicit `headerbar`/`.titlebar` `min-height` is present in the user's
//! `gtk.css`. Setting it to the native header bar height leaves real GTK apps
//! unchanged (they already compute that height) while bringing Chromium's app
//! frame up to match.

use crate::app_dirs::AppDirs;
use anyhow::{Context, Result};
use std::fs::{self};
use tracing::{debug, info};

/// Native libadwaita `HeaderBar` height. Real GTK apps already render at this
/// height, so using it as a floor is visually neutral for them.
const HEADER_BAR_MIN_HEIGHT_PX: u32 = 46;

const BLOCK_BEGIN: &str = "/* === web-app-hub (BEGIN) — managed block, do not edit === */";
const BLOCK_END: &str = "/* === web-app-hub (END) === */";

/// GTK config sub-directories that hold a `gtk.css`. Chromium reads the version
/// matching its `--gtk-version` flag; we cover both so the override applies
/// regardless of which version a browser ends up using.
const GTK_CONFIG_DIRS: [&str; 2] = ["gtk-4.0", "gtk-3.0"];

/// Write (or refresh) the managed header bar override in the user's `gtk.css`
/// files. Existing user content is preserved; only the marked block is touched.
pub fn apply_header_bar_override(app_dirs: &AppDirs) -> Result<()> {
    let block = managed_block();

    for dir in GTK_CONFIG_DIRS {
        let gtk_dir = app_dirs.user_config.join(dir);
        let gtk_css = gtk_dir.join("gtk.css");

        let existing = fs::read_to_string(&gtk_css).unwrap_or_default();
        let updated = merge_block(&existing, &block);

        if updated == existing {
            debug!(path = %gtk_css.display(), "Header bar override already up to date");
            continue;
        }

        fs::create_dir_all(&gtk_dir)
            .with_context(|| format!("Failed to create GTK config dir: {}", gtk_dir.display()))?;
        fs::write(&gtk_css, &updated)
            .with_context(|| format!("Failed to write GTK config: {}", gtk_css.display()))?;

        info!(path = %gtk_css.display(), "Updated header bar override in GTK config");
    }

    Ok(())
}

fn managed_block() -> String {
    format!(
        "{BLOCK_BEGIN}\nheaderbar, .titlebar {{ min-height: {HEADER_BAR_MIN_HEIGHT_PX}px; }}\n{BLOCK_END}"
    )
}

/// Replace an existing managed block in `existing`, or append a new one. Content
/// outside the markers is left untouched.
fn merge_block(existing: &str, block: &str) -> String {
    if let (Some(start), Some(end_marker)) = (existing.find(BLOCK_BEGIN), existing.find(BLOCK_END))
    {
        let end = end_marker + BLOCK_END.len();
        if start < end {
            let mut result = String::with_capacity(existing.len());
            result.push_str(&existing[..start]);
            result.push_str(block);
            result.push_str(&existing[end..]);
            return result;
        }
    }

    if existing.is_empty() {
        format!("{block}\n")
    } else if existing.ends_with('\n') {
        format!("{existing}{block}\n")
    } else {
        format!("{existing}\n{block}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_to_empty() {
        let out = merge_block("", &managed_block());
        assert!(out.contains(BLOCK_BEGIN));
        assert!(out.contains("min-height: 46px"));
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn appends_after_user_content_with_newline() {
        let user = "window { background: red; }\n";
        let out = merge_block(user, &managed_block());
        assert!(out.starts_with(user));
        assert!(out.contains(BLOCK_BEGIN));
    }

    #[test]
    fn appends_after_user_content_without_trailing_newline() {
        let user = "window { background: red; }";
        let out = merge_block(user, &managed_block());
        assert!(out.starts_with("window { background: red; }\n"));
        assert!(out.contains(BLOCK_BEGIN));
    }

    #[test]
    fn replaces_existing_block_and_keeps_surrounding_content() {
        let user = format!(
            "header {{ a: b; }}\n{BLOCK_BEGIN}\nheaderbar {{ min-height: 99px; }}\n{BLOCK_END}\nfooter {{ c: d; }}\n"
        );
        let out = merge_block(&user, &managed_block());
        assert!(out.contains("header { a: b; }"));
        assert!(out.contains("footer { c: d; }"));
        assert!(out.contains("min-height: 46px"));
        assert!(!out.contains("min-height: 99px"));
        // Only one managed block remains.
        assert_eq!(out.matches(BLOCK_BEGIN).count(), 1);
    }

    #[test]
    fn is_idempotent() {
        let once = merge_block("", &managed_block());
        let twice = merge_block(&once, &managed_block());
        assert_eq!(once, twice);
    }
}
