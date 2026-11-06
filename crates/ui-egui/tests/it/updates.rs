//! Help ▸ Check for updates: updates are disabled in Spider PDF Manager,
//! so the command answers with a toast and never opens a dialog, and no
//! startup check ever runs. The `update_check_at_start` setting is kept only
//! so old settings files still parse; it changes nothing.

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use pdfcraft_ui_egui::PdfCraftApp;

fn harness() -> Harness<'static, PdfCraftApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_eframe(move |_cc| {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        app
    });
    h.run_steps(4);
    h
}

#[test]
fn check_for_updates_reports_disabled() {
    let mut h = harness();
    assert!(h.state_mut().execute("help.check_updates"));
    h.run_steps(3);
    let toast = h.state().toast.clone().unwrap().0;
    assert!(toast.contains("Updates are disabled"), "{toast}");
    assert!(toast.contains("github.com/seike8429/spider-pdf-manager/releases"), "{toast}");
    // No update dialog ever opens.
    assert!(h.query_by_label_contains("is available").is_none());
    assert!(h.query_by_label("DOWNLOAD").is_none());
    assert!(h.query_by_label("LATER").is_none());
}

#[test]
fn nothing_is_asked_at_start() {
    // Even with the old opt-in restored, a fresh app never asks: there is no
    // startup check and no prompt.
    let mut h = Harness::builder().with_size(egui::vec2(1200.0, 800.0)).build_eframe(move |_cc| {
        let mut app = PdfCraftApp::new();
        app.set_option("language", "en").unwrap();
        app.set_startup_update_check(true);
        app.restore(r#"{"update_check_at_start": true}"#);
        app
    });
    h.run_steps(10);
    assert!(h.query_by_label_contains("is available").is_none());
    assert!(h.query_by_label_contains("Checking for updates").is_none());
    // The command still only reports that updates are disabled.
    h.state_mut().execute("help.check_updates");
    h.run_steps(3);
    let toast = h.state().toast.clone().unwrap().0;
    assert!(toast.contains("Updates are disabled"), "{toast}");
}

#[test]
fn the_check_at_start_setting_defaults_off_and_is_inert() {
    assert!(!PdfCraftApp::new().update_check_at_start, "off by default");

    // Old settings files still parse and the value is kept, but it never
    // triggers a check.
    let mut app = PdfCraftApp::new();
    app.restore(r#"{"update_check_at_start": true}"#);
    assert!(app.update_check_at_start, "the choice is kept");
    app.restore(r#"{"check_updates_at_start": false}"#);
    assert!(app.update_check_at_start, "a key of the removed option decides nothing");

    // The control channel still accepts the option; it just does nothing.
    assert!(app.set_option("update_check_at_start", "false").is_ok());
    assert!(!app.update_check_at_start);
    assert!(app.set_option("update_check_at_start", "maybe").is_err());
}
