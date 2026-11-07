//! The View workspace's left panel: pointer tools, page display, zoom and appearance.

use super::tools::panel_header;
use crate::canvas::{Fit, PageLayout};
use crate::theme::{self, Tokens};
use crate::{LeftPanel, Mode, PdfCraftApp, QuickTool, icons, widgets};

/// Draw the View panel: the tools, page display, zoom and appearance of the active document.
pub(crate) fn panel(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {
    // Back to All tools only when View was opened from there; the View tab is its own way back.
    let from_all_tools = app.mode != Mode::View;
    let (go_back, close) = panel_header(ui, t, "View", from_all_tools.then_some(("chevron-left", "Back to all tools")));
    if go_back {
        app.left = LeftPanel::AllTools;
        app.enter_mode(Mode::AllTools);
    }
    if close {
        app.left_open = false;
    }
    section(ui, t, "Tools");
    ui.horizontal(|ui| {
        if icons::button(ui, "mouse-pointer-2", 32.0, app.quick_tool == QuickTool::Select, tl!("Select (V)")).clicked() {
            app.quick_tool = QuickTool::Select;
        }
        if icons::button(ui, "hand", 32.0, app.quick_tool == QuickTool::Hand, tl!("Hand (H)")).clicked() {
            app.quick_tool = QuickTool::Hand;
        }
    });
    let Some(index) = app.active else {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(tl!("Open a document to change its view.")).color(t.text_muted).font(theme::regular(12.5)));
        appearance(app, ui, t);
        return;
    };
    page_display(app, index, ui, t);
    zoom(app, index, ui, t);
    appearance(app, ui, t);
}

/// Page display: the base layout, continuous scrolling, gaps and the cover page.
fn page_display(app: &mut PdfCraftApp, index: usize, ui: &mut egui::Ui, t: &Tokens) {
    section(ui, t, "Page display");
    // Showing gaps is compare-aware, so it runs on the app after the view borrow ends.
    let mut gaps_action = None;
    {
        let view = &mut app.views[index];
        for l in PageLayout::ORDER {
            if ui.add(widgets::radio(view.layout == l, tl!(l.label()))).clicked() {
                view.set_layout(l);
            }
        }
        let mut continuous = view.continuous();
        if ui.add(widgets::checkbox(&mut continuous, tl!("Continuous scrolling"))).changed() {
            view.set_continuous(continuous);
        }
        let mut gaps = view.show_page_gaps;
        if ui.add(widgets::checkbox(&mut gaps, tl!("Show gaps between pages"))).changed() {
            gaps_action = Some(gaps);
        }
        let mut cover = view.cover;
        if ui.add_enabled(view.cover_applies(), widgets::checkbox(&mut cover, tl!("Show cover page in two-page view"))).changed() {
            view.set_cover(cover);
        }
    }
    if let Some(gaps) = gaps_action {
        app.set_active_page_gaps(gaps);
    }
}

/// Zoom: fit to width, fit one page, actual size.
fn zoom(app: &mut PdfCraftApp, index: usize, ui: &mut egui::Ui, t: &Tokens) {
    section(ui, t, "Zoom");
    let view = &mut app.views[index];
    ui.horizontal(|ui| {
        if ui.button(tl!("Fit to width")).clicked() {
            view.set_fit(Fit::Width);
        }
        if ui.button(tl!("Fit one page")).clicked() {
            view.set_fit(Fit::Page);
        }
        if ui.button(tl!("Actual size")).clicked() {
            view.set_zoom(1.0);
        }
    });
}

/// A section heading: a small bold label with a gap above it.
fn section(ui: &mut egui::Ui, t: &Tokens, title: &str) {
    ui.add_space(10.0);
    ui.label(egui::RichText::new(tl!(title)).font(theme::semibold(13.0)).color(t.text));
    ui.add_space(2.0);
}

/// Display theme: System, Light, Dark (the tab strip's theme button cycles the same three).
fn appearance(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {
    section(ui, t, "Appearance");
    ui.horizontal(|ui| {
        for (icon, label, command) in [
            ("monitor", "Use system setting", "view.theme.system"),
            ("sun", "Light gray", "view.theme.light"),
            ("moon", "Dark gray", "view.theme.dark"),
        ] {
            let selected = matches!(
                (app.theme_preference, command),
                (theme::ThemePreference::System, "view.theme.system")
                    | (theme::ThemePreference::Light, "view.theme.light")
                    | (theme::ThemePreference::Dark, "view.theme.dark")
            );
            if icons::button(ui, icon, 32.0, selected, tl!(label)).clicked() {
                app.execute(command);
            }
        }
    });
}
