//! Inline comment editor for circuit-block labels. A single-line text field
//! sits over the clicked label; Enter or focus loss commits, Escape cancels.
//! As with the angle editor, only `Key::Enter` events commit, and the web
//! runner drops the Enter that confirms an IME conversion before egui sees
//! it.

use eframe::egui;

use super::circuit_blocks::{
    circuit_block_label_center, circuit_block_label_font, circuit_block_label_width,
    circuit_block_rect, circuit_block_vertical_label_rect, paint_circuit_block_underline,
    CIRCUIT_BLOCK_PLACEHOLDER,
};
use crate::app::{CircuitBlockEditor, CircuitBlockLabelSlot, QniApp};
use crate::colors::Colors;
use crate::constants::CIRCUIT_BLOCK_LABEL_LINE_HEIGHT;
use crate::layout::{CircuitProjection, LayoutMetrics};

/// Longest comment the editor accepts, in characters. URLs may carry longer
/// labels; only typing is limited.
pub(crate) const CIRCUIT_BLOCK_LABEL_MAX_CHARS: usize = 32;

impl QniApp {
    pub(crate) fn open_circuit_block_editor(
        &mut self,
        index: usize,
        slot: CircuitBlockLabelSlot,
        ctx: &egui::Context,
    ) {
        if !self.circuit_block_editing_allowed() {
            return;
        }
        let Some(block) = self.circuit_blocks.get(index) else {
            return;
        };
        self.circuit_block_editor = Some(CircuitBlockEditor {
            index,
            slot,
            draft: block.label().to_owned(),
            select_all_pending: true,
            commit_after_frame: false,
        });
        self.angle_editor = None;
        self.angle_affordance = None;
        self.picker.close();
        ctx.request_repaint();
    }

    /// Store the edited comment. An unchanged draft keeps the label as it
    /// is, even with surrounding spaces from a hand-written URL; otherwise
    /// the draft is trimmed, and an empty result changes nothing because
    /// labels can never be empty.
    pub(crate) fn commit_circuit_block_editor(&mut self, ctx: &egui::Context) {
        let Some(editor) = self.circuit_block_editor.take() else {
            return;
        };
        ctx.request_repaint();
        if !self.circuit_block_editing_allowed() {
            return;
        }
        let Some(label) = self
            .circuit_blocks
            .get(editor.index)
            .map(|block| block.label())
        else {
            return;
        };
        let trimmed = editor.draft.trim();
        if editor.draft == label || trimmed.is_empty() || trimmed == label {
            return;
        }
        self.begin_circuit_commit();
        let _ = self
            .circuit_blocks
            .set_label(editor.index, trimmed.to_owned());
        self.commit_current_circuit(ctx);
    }

    pub(crate) fn show_circuit_block_editor(
        &mut self,
        ui: &mut egui::Ui,
        rect: egui::Rect,
        metrics: &LayoutMetrics,
        colors: &Colors,
        scroll_time: f32,
    ) {
        let projection = self.circuit_projection(rect, scroll_time);
        publish_circuit_block_geometry_json(self, ui.ctx(), metrics, projection);
        if !self.circuit_block_editing_allowed() {
            self.circuit_block_editor = None;
            return;
        }
        let Some(mut editor) = self.circuit_block_editor.take() else {
            return;
        };
        let Some(body) = self
            .circuit_blocks
            .get(editor.index)
            .and_then(|block| circuit_block_rect(block, metrics))
            .map(|local| projection.to_screen_rect(local))
        else {
            return;
        };
        let opening = editor.select_all_pending;
        let commit_after_frame = editor.commit_after_frame;
        editor.select_all_pending = false;
        editor.commit_after_frame = false;

        let shown = if editor.draft.is_empty() {
            CIRCUIT_BLOCK_PLACEHOLDER
        } else {
            editor.draft.as_str()
        };
        let text_width = circuit_block_label_width(ui.ctx(), shown);
        let edit_rect = circuit_block_editor_rect(body, editor.slot, text_width, projection);
        let mut edit_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(("circuit-block-editor-child", editor.index))
                .max_rect(edit_rect)
                .layout(egui::Layout::left_to_right(if projection.is_vertical() {
                    egui::Align::Min
                } else {
                    egui::Align::Center
                })),
        );
        edit_ui.set_min_size(edit_rect.size());
        edit_ui.visuals_mut().selection.stroke = egui::Stroke::new(0.0_f32, colors.text_strong);
        let output = egui::TextEdit::singleline(&mut editor.draft)
            .id(ui.make_persistent_id(("circuit-block-editor", editor.index)))
            .font(circuit_block_label_font())
            .text_color(colors.circuit_block_label)
            .hint_text(
                egui::RichText::new(CIRCUIT_BLOCK_PLACEHOLDER)
                    .color(colors.circuit_block_placeholder),
            )
            .char_limit(CIRCUIT_BLOCK_LABEL_MAX_CHARS)
            .horizontal_align(if projection.is_vertical() {
                egui::Align::Max
            } else {
                egui::Align::Center
            })
            .frame(egui::Frame::NONE)
            .margin(egui::Margin::ZERO)
            .desired_width(edit_rect.width())
            .show(&mut edit_ui);
        if opening {
            output.response.request_focus();
            let mut state = output.state;
            state
                .cursor
                .set_char_range(Some(egui::text::CCursorRange::two(
                    egui::text::CCursor::default(),
                    egui::text::CCursor::new(editor.draft.chars().count()),
                )));
            state.store(ui.ctx(), output.response.id);
        }
        paint_circuit_block_underline(
            ui.painter(),
            if projection.is_vertical() {
                egui::Rect::from_min_size(output.galley_pos, output.galley.size())
            } else {
                egui::Rect::from_center_size(
                    output.text_clip_rect.center(),
                    egui::vec2(text_width, output.galley.size().y),
                )
            },
            colors,
        );

        let (enter, escape) = ui.input(|input| {
            input
                .events
                .iter()
                .fold((false, false), |(enter, escape), event| match event {
                    egui::Event::Key {
                        key, pressed: true, ..
                    } => (
                        enter || *key == egui::Key::Enter,
                        escape || *key == egui::Key::Escape,
                    ),
                    _ => (enter, escape),
                })
        });
        if output.response.changed() {
            ui.ctx().request_repaint();
        }
        let lost_focus = output.response.lost_focus() && !opening;
        if escape {
            ui.ctx().request_repaint();
        } else if enter || commit_after_frame || lost_focus {
            self.circuit_block_editor = Some(editor);
            self.commit_circuit_block_editor(ui.ctx());
        } else {
            self.circuit_block_editor = Some(editor);
        }
    }
}

/// The editor stays upright in both orientations. In portrait it is
/// right-aligned with the single label to the left of the projected body.
fn circuit_block_editor_rect(
    body: egui::Rect,
    slot: CircuitBlockLabelSlot,
    text_width: f32,
    projection: CircuitProjection,
) -> egui::Rect {
    let padded_width = text_width + crate::tailwind::SPACING_4;
    if projection.is_vertical() {
        circuit_block_vertical_label_rect(body, padded_width)
    } else {
        egui::Rect::from_center_size(
            circuit_block_label_center(body, slot),
            egui::vec2(
                body.width().max(padded_width),
                CIRCUIT_BLOCK_LABEL_LINE_HEIGHT,
            ),
        )
    }
}

#[cfg(all(target_arch = "wasm32", debug_assertions))]
fn publish_circuit_block_geometry_json(
    app: &QniApp,
    ctx: &egui::Context,
    metrics: &LayoutMetrics,
    projection: CircuitProjection,
) {
    use super::circuit_blocks::{
        circuit_block_handle_rect, circuit_block_label_rect, CIRCUIT_BLOCK_LABEL_SLOTS,
    };
    use crate::app::CircuitBlockEdge;

    let rect_json = |rect: egui::Rect| {
        format!(
            "{{\"left\":{:.3},\"top\":{:.3},\"right\":{:.3},\"bottom\":{:.3}}}",
            rect.left(),
            rect.top(),
            rect.right(),
            rect.bottom()
        )
    };
    let blocks = app
        .circuit_blocks
        .iter()
        .filter_map(|block| {
            let local = circuit_block_rect(block, metrics)?;
            let body = projection.to_screen_rect(local);
            let width = circuit_block_label_width(ctx, block.label());
            let labels = if projection.is_vertical() {
                rect_json(circuit_block_vertical_label_rect(body, width))
            } else {
                CIRCUIT_BLOCK_LABEL_SLOTS
                    .map(|slot| rect_json(circuit_block_label_rect(body, slot, width)))
                    .join(",")
            };
            let handles = [CircuitBlockEdge::Left, CircuitBlockEdge::Right]
                .map(|edge| rect_json(projection.to_screen_rect(circuit_block_handle_rect(local, edge))))
                .join(",");
            Some(format!(
                "{{\"label\":\"{}\",\"start\":{},\"end\":{},\"body\":{},\"labels\":[{labels}],\"handles\":[{handles}]}}",
                block.label().replace('\\', "\\\\").replace('"', "\\\""),
                block.start().as_usize(),
                block.end().as_usize(),
                rect_json(body),
            ))
        })
        .collect::<Vec<_>>()
        .join(",");
    let editor = app.circuit_block_editor.as_ref().map_or_else(
        || "null".to_owned(),
        |editor| {
            format!(
                "{{\"index\":{},\"draft\":\"{}\"}}",
                editor.index,
                editor.draft.replace('\\', "\\\\").replace('"', "\\\"")
            )
        },
    );
    let hovered = app
        .hovered_circuit_block
        .map_or_else(|| "null".to_owned(), |hover| hover.index.to_string());
    let json = format!("{{\"blocks\":[{blocks}],\"editor\":{editor},\"hovered\":{hovered}}}");
    crate::test_hooks::set_window_value(
        crate::test_hooks::QNI_CIRCUIT_BLOCK_GEOMETRY_JSON,
        &wasm_bindgen::JsValue::from_str(&json),
    );
}

#[cfg(any(not(target_arch = "wasm32"), not(debug_assertions)))]
fn publish_circuit_block_geometry_json(
    _app: &QniApp,
    _ctx: &egui::Context,
    _metrics: &LayoutMetrics,
    _projection: CircuitProjection,
) {
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::app::circuit_library;
    use crate::layout::layout_metrics;

    const BLOCK: &str = r#"{"cols":[["{a"],["H"],["}"],["X"]]}"#;

    fn screen() -> egui::Rect {
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1280.0, 800.0))
    }

    fn editable(json: &str) -> (QniApp, egui::Context) {
        let ctx = egui::Context::default();
        let mut app = QniApp::new(&eframe::CreationContext::_new_kittest(ctx.clone()));
        app.library = circuit_library::for_startup(json.to_owned(), true).0;
        app.apply_url_payload(json.to_owned(), &ctx);
        (app, ctx)
    }

    fn frame(app: &mut QniApp, ctx: &egui::Context, events: Vec<egui::Event>) {
        let colors = app.colors();
        let _ = ctx.run_ui(
            egui::RawInput {
                events,
                screen_rect: Some(screen()),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    let metrics = layout_metrics(1280.0, 2, 6);
                    app.show_circuit_block_editor(ui, screen(), &metrics, &colors, 0.0);
                });
            },
        );
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        }
    }

    fn text(value: &str) -> egui::Event {
        egui::Event::Text(value.to_owned())
    }

    /// Open the editor on block 0 and let it take focus with the label
    /// selected, as after a click on the label.
    fn open(json: &str) -> (QniApp, egui::Context) {
        let (mut app, ctx) = editable(json);
        frame(&mut app, &ctx, Vec::new());
        app.open_circuit_block_editor(0, CircuitBlockLabelSlot::Top, &ctx);
        frame(&mut app, &ctx, Vec::new());
        (app, ctx)
    }

    fn label(app: &QniApp) -> Option<&str> {
        app.circuit_blocks.get(0).map(|block| block.label())
    }

    struct PortraitEditorGeometry {
        expected: egui::Rect,
        visible_label: egui::Rect,
        body: egui::Rect,
        unscrolled: egui::Rect,
        response: egui::Rect,
        editor_open: bool,
    }

    fn portrait_editor_geometry() -> PortraitEditorGeometry {
        let (mut app, ctx) = editable(BLOCK);
        app.orientation = crate::layout::CircuitOrientation::Vertical;
        app.circuit_scroll_time = 37.0;
        app.open_circuit_block_editor(0, CircuitBlockLabelSlot::Top, &ctx);
        let screen = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(390.0, 844.0));
        let rect = egui::Rect::from_min_max(egui::pos2(0.0, 200.0), egui::pos2(390.0, 700.0));
        let metrics = app.circuit_layout_metrics(rect);
        let projection = app.circuit_projection(rect, app.circuit_scroll_time);
        let local = circuit_block_rect(app.circuit_blocks.get(0).unwrap(), &metrics).unwrap();
        let body = projection.to_screen_rect(local);
        let colors = app.colors();
        let mut observed = None;
        let _ = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    let width = circuit_block_label_width(ui.ctx(), "a");
                    let visible_label = circuit_block_vertical_label_rect(body, width);
                    let expected = circuit_block_editor_rect(
                        body,
                        CircuitBlockLabelSlot::Top,
                        width,
                        projection,
                    );
                    let unscrolled = app.circuit_projection(rect, 0.0).to_screen_rect(local);
                    let editor_id = ui.make_persistent_id(("circuit-block-editor", 0_usize));
                    app.show_circuit_block_editor(
                        ui,
                        rect,
                        &metrics,
                        &colors,
                        app.circuit_scroll_time,
                    );
                    let response = ui
                        .ctx()
                        .read_response(editor_id)
                        .expect("portrait text field rendered");
                    observed = Some((expected, visible_label, unscrolled, response.rect));
                });
            },
        );
        let (expected, visible_label, unscrolled, response) = observed.unwrap();
        PortraitEditorGeometry {
            expected,
            visible_label,
            body,
            unscrolled,
            response,
            editor_open: app.circuit_block_editor.is_some(),
        }
    }

    #[test]
    fn portrait_editor_matches_the_projected_label_top() {
        let geometry = portrait_editor_geometry();
        assert_eq!(
            geometry.expected.right_top(),
            geometry.visible_label.right_top()
        );
    }

    #[test]
    fn portrait_editor_sits_left_of_the_block_body() {
        let geometry = portrait_editor_geometry();
        assert!(geometry.expected.right() < geometry.body.left());
    }

    #[test]
    fn portrait_block_body_tracks_scroll_projection() {
        let geometry = portrait_editor_geometry();
        assert_eq!(geometry.body.top(), geometry.unscrolled.top() - 37.0);
    }

    #[test]
    fn portrait_text_field_starts_at_the_label_top() {
        let geometry = portrait_editor_geometry();
        assert_eq!(geometry.response.right_top(), geometry.expected.right_top());
    }

    #[test]
    fn portrait_text_field_matches_editor_width() {
        let geometry = portrait_editor_geometry();
        assert_eq!(geometry.response.width(), geometry.expected.width());
    }

    #[test]
    fn portrait_text_field_fits_editor_height() {
        let geometry = portrait_editor_geometry();
        assert!(geometry.response.height() <= geometry.expected.height());
    }

    #[test]
    fn portrait_editor_remains_open() {
        let geometry = portrait_editor_geometry();
        assert!(geometry.editor_open);
    }

    #[test]
    fn opening_selects_the_whole_label_so_typing_replaces_it() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![text("量子もつれ")]);

        assert_eq!(
            app.circuit_block_editor
                .as_ref()
                .map(|editor| editor.draft.as_str()),
            Some("量子もつれ")
        );
    }

    #[test]
    fn enter_commits_the_comment() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![text("量子もつれ")]);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);

        assert_eq!(label(&app), Some("量子もつれ"));
    }

    #[test]
    fn enter_closes_the_editor() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![text("b")]);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);

        assert!(app.circuit_block_editor.is_none());
    }

    #[test]
    fn escape_keeps_the_original_label() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![text("b")]);
        frame(&mut app, &ctx, vec![key(egui::Key::Escape)]);

        assert_eq!(label(&app), Some("a"));
    }

    #[test]
    fn escape_closes_the_editor() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![key(egui::Key::Escape)]);

        assert!(app.circuit_block_editor.is_none());
    }

    #[test]
    fn commit_trims_surrounding_spaces() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![text("  b  ")]);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);

        assert_eq!(label(&app), Some("b"));
    }

    #[test]
    fn blank_comment_keeps_the_label() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![text("   ")]);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);

        assert_eq!(label(&app), Some("a"));
    }

    #[test]
    fn unchanged_label_with_spaces_from_the_url_stays_as_is() {
        let (mut app, ctx) = open(r#"{"cols":[["{ a "],["H"],["}"]]}"#);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);

        assert_eq!(label(&app), Some(" a "));
    }

    #[test]
    fn typing_stops_at_thirty_two_characters() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![text(&"あ".repeat(40))]);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);

        assert_eq!(label(&app).map(|label| label.chars().count()), Some(32));
    }

    #[test]
    fn pasted_newlines_do_not_enter_the_label() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![egui::Event::Paste("b\nc".to_owned())]);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);

        assert_eq!(label(&app), Some("b c"));
    }

    #[test]
    fn a_comment_change_is_one_undo_step() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![text("b")]);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);
        app.undo_circuit(&ctx);

        assert_eq!(label(&app), Some("a"));
    }

    #[test]
    fn a_committed_comment_reaches_the_circuit_json() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![text("量子もつれ")]);
        frame(&mut app, &ctx, vec![key(egui::Key::Enter)]);

        assert_eq!(
            app.library.active().circuit_json,
            r#"{"cols":[["{量子もつれ"],["H"],["}"],["X"]]}"#
        );
    }

    #[test]
    fn pressing_elsewhere_commits_the_comment() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![text("量子もつれ")]);
        if let Some(editor) = app.circuit_block_editor.as_mut() {
            editor.commit_after_frame = true;
        }
        frame(&mut app, &ctx, Vec::new());

        assert_eq!(label(&app), Some("量子もつれ"));
    }

    #[test]
    fn locked_circuit_does_not_open_the_editor() {
        let (mut app, ctx) = editable(BLOCK);
        app.library.toggle_active_lock();
        app.open_circuit_block_editor(0, CircuitBlockLabelSlot::Top, &ctx);

        assert!(app.circuit_block_editor.is_none());
    }

    #[test]
    fn palette_without_the_comment_block_does_not_open_the_editor() {
        let (mut app, ctx) = editable(BLOCK);
        app.palette = crate::layout::Palette::restricted(&["H"]).unwrap();
        app.open_circuit_block_editor(0, CircuitBlockLabelSlot::Top, &ctx);

        assert!(app.circuit_block_editor.is_none());
    }

    #[test]
    fn empty_draft_shows_the_placeholder_editor_snapshot() {
        let (mut app, ctx) = open(BLOCK);
        frame(&mut app, &ctx, vec![key(egui::Key::Backspace)]);
        let colors = app.colors();
        let output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(screen()),
                ..Default::default()
            },
            |root_ui| {
                egui::CentralPanel::default().show(root_ui, |ui| {
                    let metrics = layout_metrics(1280.0, 2, 6);
                    let projection = app.circuit_projection(screen(), 0.0);
                    app.draw_circuit_blocks(ui.painter(), &metrics, &colors, projection);
                    app.show_circuit_block_editor(ui, screen(), &metrics, &colors, 0.0);
                });
            },
        );
        insta::assert_debug_snapshot!(output.shapes);
    }
}
