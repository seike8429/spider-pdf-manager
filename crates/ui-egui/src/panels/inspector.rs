//! The Inspector: properties of the current selection — a comment, a form field, selected
//! text or selected pages. It opens itself when something is selected and stays closed for the
//! session once the user closes it ([`crate::PdfCraftApp::inspector_dismissed`]).

use egui::Stroke;
use pdfcraft_engine::{Edit, NewAnnotation, Shape};

use super::tools::panel_header;
use crate::PdfCraftApp;
use crate::comments::{CommentTool, quick_bar_colour};
use crate::theme::{self, Tokens};

/// What the Inspector shows, decided from the current selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum InspectorKind {
    None,
    Annotation,
    Field,
    Text,
    Page,
}

/// Which kind wins when several selections exist at once: the annotation under the pointer
/// first, then the form field being prepared, then selected text, then selected pages.
pub(crate) fn decide(has_annotation: bool, has_field: bool, has_text: bool, has_pages: bool) -> InspectorKind {
    if has_annotation {
        InspectorKind::Annotation
    } else if has_field {
        InspectorKind::Field
    } else if has_text {
        InspectorKind::Text
    } else if has_pages {
        InspectorKind::Page
    } else {
        InspectorKind::None
    }
}

/// The kind for the app's current selection, if any.
pub(crate) fn kind(app: &PdfCraftApp) -> InspectorKind {
    let Some(index) = app.active else { return InspectorKind::None };
    let view = &app.views[index];
    decide(
        view.comments.selected.is_some(),
        app.is_preparing() && view.prepare.selected.is_some(),
        view.selected_text().is_some(),
        view.organize && !view.selected.is_empty(),
    )
}

/// Draw the Inspector pane: the selection's properties, or a placeholder when nothing is
/// selected (the pane only opens with a selection, so this is the command's doing).
pub(crate) fn panel(app: &mut PdfCraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    egui::Frame::NONE.fill(t.panel).inner_margin(egui::Margin { left: 14, right: 12, top: 12, bottom: 8 }).stroke(Stroke::new(1.0, t.divider)).show(
        ui,
        |ui| {
            let (_, close) = panel_header(ui, &t, "Inspector", None);
            if close {
                app.inspector_dismissed = true;
                return;
            }
            match kind(app) {
                InspectorKind::None => {
                    ui.label(egui::RichText::new(tl!("Nothing selected.")).color(t.text_muted).font(theme::regular(12.5)));
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(tl!("Select text, a comment, a form field or pages to see its properties here."))
                            .color(t.text_faint)
                            .font(theme::regular(12.5)),
                    );
                }
                InspectorKind::Annotation => annotation(app, ui, &t),
                InspectorKind::Field => field(app, ui, &t),
                InspectorKind::Text => text(app, ui, &t),
                InspectorKind::Page => pages(app, ui, &t),
            }
        },
    );
}

/// A label/value row: the label faint, the value plain.
fn row(ui: &mut egui::Ui, t: &Tokens, label: &str, value: &str) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tl!(label)).color(t.text_faint).font(theme::regular(12.5)));
        ui.label(egui::RichText::new(value).color(t.text).font(theme::regular(12.5)));
    });
}

/// The selected comment: its type, author, colour and text, with Properties and Delete.
fn annotation(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let Some((index, id)) = app.active_ids() else { return };
    let Some((page, selected)) = app.views[index].comments.selected else { return };
    let Some(doc) = app.session.get(id) else { return };
    let Some(a) = doc.info.annotations.iter().find(|a| a.page == page && a.index == selected) else { return };
    let subtype = a.subtype.clone();
    let author = a.author.clone().unwrap_or_default();
    let contents = a.contents.clone().unwrap_or_default();
    let color = a.color.map(|c| c.map(f64::from)).unwrap_or([1.0, 1.0, 0.0]);
    let can_modify = doc.allows_annotation();
    row(ui, t, "Type", &subtype);
    if !author.is_empty() {
        row(ui, t, "Author", &author);
    }
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(tl!("Colour")).color(t.text_faint).font(theme::regular(12.5)));
        if let Some(c) = quick_bar_colour(ui, color) {
            app.colour_selected_comments(c);
        }
    });
    if !contents.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(contents.chars().take(400).collect::<String>()).color(t.text).font(theme::regular(12.5)));
    }
    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.add_enabled(can_modify, egui::Button::new(tl!("Properties…"))).clicked() {
            app.open_comment_props(page, selected);
        }
        if ui.add_enabled(can_modify, egui::Button::new(tl!("Delete"))).clicked() {
            let view = &mut app.views[index];
            if let Some(edit) = crate::comments::delete_selected(&mut view.comments) {
                view.pending_edit = Some(edit);
            }
        }
    });
}

/// The form field being prepared: its name, type, page and value, with Properties.
fn field(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let Some((index, id)) = app.active_ids() else { return };
    let Some((name, widget)) = app.views[index].prepare.selected.clone() else { return };
    let Some(doc) = app.session.get(id) else { return };
    let field = doc.form.iter().find(|f| f.name == name);
    row(ui, t, "Field", &name);
    if let Some(f) = field {
        row(ui, t, "Type", &format!("{:?}", f.kind));
        // The first widget's page: a field may have widgets on several pages.
        if let Some(page) = f.widgets.first().and_then(|w| w.page) {
            row(ui, t, "Page", &(page + 1).to_string());
        }
        if !f.value.is_empty() {
            row(ui, t, "Value", &f.value.join(", ").chars().take(200).collect::<String>());
        }
    }
    ui.add_space(8.0);
    if ui.button(tl!("Properties…")).clicked() {
        app.open_field_props(&name, widget);
    }
}

/// Selected text: the words themselves, with the markup actions from the selection bar.
fn text(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let Some((index, id)) = app.active_ids() else { return };
    let (selected, quads, can_comment) = {
        let Some(doc) = app.session.get(id) else { return };
        let view = &app.views[index];
        (view.selected_text(), view.selection_quads(&doc.info), doc.allows_annotation())
    };
    let Some(selected) = selected else { return };
    let shown: String = selected.chars().take(400).collect();
    ui.label(egui::RichText::new(shown).color(t.text).font(theme::regular(12.5)).italics());
    ui.add_space(8.0);
    let mut markup = None;
    ui.horizontal(|ui| {
        for (label, tool) in
            [(tl!("Highlight"), CommentTool::Highlight), (tl!("Underline"), CommentTool::Underline), (tl!("Strikethrough"), CommentTool::StrikeOut)]
        {
            if ui.add_enabled(can_comment, egui::Button::new(label)).clicked() {
                markup = Some(tool);
            }
        }
    });
    if let Some(tool) = markup
        && let (Some((page, quads)), Some(kind)) = (quads, tool.markup())
    {
        let style = app.comment_prefs.style(tool);
        let author = app.comment_prefs.author.clone();
        let view = &mut app.views[index];
        view.pending_edit =
            Some(Edit::AddAnnotation(NewAnnotation { page, shape: Shape::TextMarkup { kind, quads }, style, contents: String::new(), author }));
        view.clear_selection();
    }
}

/// Selected pages (Organize): how many, and which.
fn pages(app: &mut PdfCraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let Some(index) = app.active else { return };
    let view = &app.views[index];
    let count = view.selected.len();
    row(ui, t, "Pages selected", &count.to_string());
    let mut listed: Vec<String> = view.selected.iter().take(12).map(|p| (p + 1).to_string()).collect();
    if count > listed.len() {
        listed.push("…".to_string());
    }
    if !listed.is_empty() {
        ui.add_space(4.0);
        ui.label(egui::RichText::new(listed.join(", ")).color(t.text).font(theme::regular(12.5)));
    }
    ui.add_space(4.0);
    ui.label(
        egui::RichText::new(tl!("Rotate, delete and reorder these pages in the Organize panel.")).color(t.text_faint).font(theme::regular(12.5)),
    );
}

#[cfg(test)]
mod tests {
    use super::{InspectorKind, decide};

    /// The most specific selection wins: annotation over field over text over pages.
    #[test]
    fn inspector_kind_priority() {
        assert_eq!(decide(false, false, false, false), InspectorKind::None);
        assert_eq!(decide(false, false, false, true), InspectorKind::Page);
        assert_eq!(decide(false, false, true, true), InspectorKind::Text);
        assert_eq!(decide(false, true, true, true), InspectorKind::Field);
        assert_eq!(decide(true, true, true, true), InspectorKind::Annotation);
        assert_eq!(decide(true, false, false, false), InspectorKind::Annotation);
    }
}
