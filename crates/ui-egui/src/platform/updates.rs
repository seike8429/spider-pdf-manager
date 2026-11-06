//! Help ▸ Check for updates, disabled.
//!
//! Spider PDF Manager ships no update check (v1.0 decision): it never contacts any server
//! for releases, never prompts at start, and never self-updates. Help ▸ Check for updates
//! answers with a toast saying updates are disabled, and points at the GitHub releases page
//! in that message instead of opening it. The `craft-update` dependency stays in the tree,
//! unused by this module, so a later decision can re-enable checks without re-adding it.
//!
//! `set_startup_update_check` and `set_update_source` remain as no-ops, and `UpdateSource`
//! keeps its type, so existing callers (including the desktop app's start-up path and the
//! update tests) keep compiling. `Updates::open` stays too: the app reads it while deciding
//! whether a modal dialog is open.

#[cfg(not(target_arch = "wasm32"))]
use craft_update::updater::CheckSource;

use crate::PdfCraftApp;

/// Stands in for the network in tests. Kept so `set_update_source` keeps its signature; the
/// source is never used.
#[cfg(not(target_arch = "wasm32"))]
pub type UpdateSource = CheckSource;

#[derive(Default)]
pub(crate) struct Updates {
    /// The Updates dialog is showing. Never set: there is no dialog any more.
    pub(crate) open: bool,
}

impl PdfCraftApp {
    /// Whether the app asks for a newer release on its first frame. No-op: update checking is
    /// disabled, so nothing ever asks.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_startup_update_check(&mut self, _on: bool) {}

    /// Replaces the network with `source` (tests). No-op: the source is never used.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn set_update_source(&mut self, _source: UpdateSource) {}

    /// Help ▸ Check for updates: report that updates are disabled.
    pub fn check_for_updates(&mut self) {
        self.notify_tr(
            "Updates are disabled in Spider PDF Manager. New releases are listed at https://github.com/seike8429/spider-pdf-manager/releases.",
        );
    }

    /// Starts the checks and picks up their answers (each frame). No-op: there are no checks.
    pub(crate) fn poll_updates(&mut self) {}
}

/// The startup prompt, and the Updates dialog that answers Help ▸ Check for updates.
/// No-op: there is no prompt and no dialog.
pub(crate) fn dialog(app: &mut PdfCraftApp, ctx: &egui::Context) {
    let _ = ctx;
    // Belt and braces: nothing ever opens it, but never leave it open.
    app.updates.open = false;
}
