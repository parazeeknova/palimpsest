use eframe::egui;
use egui_phosphor::regular::{
    ARROW_DOWN, FILE, FILE_PLUS, FOLDER, GIT_BRANCH, GIT_COMMIT, LIST_CHECKS, PLUS, TRASH, WARNING,
    X,
};

use crate::git::GitRepo;
use crate::state::{AppState, CachedFileChangeKind, CachedFileStatus, CommitAction};

const PANEL_WIDTH: f32 = 360.0;
const PANEL_HEIGHT: f32 = 420.0;
const PANEL_MARGIN: f32 = 18.0;
const FILE_ROW_HEIGHT: f32 = 22.0;
const MAX_VISIBLE_FILES: usize = 10;
const SCROLL_THRESHOLD: usize = 12;
const MAX_TITLE_LEN: usize = 150;

const HEADER_H: f32 = 32.0;
const MSG_BOX_H: f32 = 72.0;
const FOOTER_H: f32 = 36.0;
const CONTENT_PAD: f32 = 10.0;
const SECTION_GAP: f32 = 6.0;

fn clamped_list_height(ui: &egui::Ui, visible_count: usize) -> f32 {
    (visible_count as f32 * FILE_ROW_HEIGHT).min((ui.available_height() - 4.0).max(0.0))
}

fn section_divider(ui: &mut egui::Ui) {
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.left_center(), rect.right_center()],
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(68, 68, 68)),
    );
}

#[derive(Default)]
pub struct State {
    pub title: String,
    pub description: String,
    pub amend: bool,
    pub sign_off: bool,
    pub pending_action: Option<CommitAction>,
    pub show_discard_confirm: bool,
}

impl State {
    fn queue_action(&mut self, action: CommitAction) {
        self.pending_action = Some(action);
    }
}

pub fn show(
    ui: &mut egui::Ui,
    body_rect: egui::Rect,
    state: &mut State,
    git_repo: Option<&GitRepo>,
) {
    let status = git_repo.and_then(|r| r.status().ok());
    let header_text = git_repo
        .and_then(|r| r.head_branch().ok())
        .unwrap_or_else(|| "HEAD".to_string());

    let panel_rect = calc_panel_rect(body_rect);

    render_panel(ui, panel_rect, state, header_text.as_str(), &status);
}

pub fn show_cached(
    ui: &mut egui::Ui,
    body_rect: egui::Rect,
    state: &mut State,
    app_state: &AppState,
) {
    let header_text = app_state
        .cached_status
        .as_ref()
        .map(|s| s.branch.clone())
        .unwrap_or_else(|| "HEAD".to_string());

    let panel_rect = calc_panel_rect(body_rect);

    render_panel_cached(ui, panel_rect, state, &header_text, app_state);
}

fn panel_width(body_width: f32) -> f32 {
    let available = (body_width - PANEL_MARGIN * 2.0).max(0.0);
    PANEL_WIDTH.min(available).max(0.0).min(available)
}

fn calc_panel_rect(body_rect: egui::Rect) -> egui::Rect {
    let available_height = (body_rect.height() - PANEL_MARGIN * 2.0).max(0.0);
    let height = PANEL_HEIGHT
        .min(available_height)
        .max(0.0)
        .min(available_height);
    let width = panel_width(body_rect.width());

    egui::Rect::from_min_size(
        egui::pos2(
            body_rect.right() - width - PANEL_MARGIN,
            body_rect.bottom() - height - PANEL_MARGIN,
        ),
        egui::vec2(width, height),
    )
}

fn render_panel(
    ui: &mut egui::Ui,
    panel_rect: egui::Rect,
    state: &mut State,
    header_text: &str,
    status: &Option<crate::git::models::RepoStatus>,
) {
    let fill = egui::Color32::from_rgb(36, 36, 36);
    let header_fill = egui::Color32::from_rgb(44, 44, 44);
    let footer_fill = egui::Color32::from_rgb(40, 40, 40);
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(78, 78, 78));
    let muted = egui::Color32::from_rgb(172, 172, 172);

    ui.painter().rect_filled(
        panel_rect.translate(egui::vec2(3.0, 3.0)),
        6,
        egui::Color32::from_black_alpha(80),
    );
    ui.painter().rect_filled(panel_rect, 6, fill);
    ui.painter()
        .rect_stroke(panel_rect, 6, stroke, egui::StrokeKind::Inside);

    let header_rect = egui::Rect::from_min_size(
        panel_rect.left_top(),
        egui::vec2(panel_rect.width(), HEADER_H),
    );
    ui.painter().rect_filled(
        header_rect,
        egui::CornerRadius {
            nw: 6,
            ne: 6,
            sw: 0,
            se: 0,
        },
        header_fill,
    );
    ui.painter().line_segment(
        [header_rect.left_bottom(), header_rect.right_bottom()],
        stroke,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.left() + 12.0, header_rect.center().y),
        GIT_COMMIT,
        15.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.left() + 34.0, header_rect.center().y),
        &format!("Commit to {}", header_text),
        12.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );
    if let Some(s) = status {
        header_stats(ui, header_rect, s.additions, s.deletions, s.files_changed);
    }

    let footer_rect = egui::Rect::from_min_size(
        egui::pos2(panel_rect.left(), panel_rect.bottom() - FOOTER_H),
        egui::vec2(panel_rect.width(), FOOTER_H),
    );
    ui.painter().rect_filled(
        footer_rect,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: 6,
            se: 6,
        },
        footer_fill,
    );
    ui.painter()
        .line_segment([footer_rect.left_top(), footer_rect.right_top()], stroke);

    let msg_box_rect = egui::Rect::from_min_max(
        egui::pos2(
            panel_rect.left() + CONTENT_PAD,
            footer_rect.top() - MSG_BOX_H - SECTION_GAP,
        ),
        egui::pos2(
            panel_rect.right() - CONTENT_PAD,
            footer_rect.top() - SECTION_GAP,
        ),
    );

    let content_left = panel_rect.left() + CONTENT_PAD;
    let content_right = panel_rect.right() - CONTENT_PAD;

    let unstaged_bottom = msg_box_rect.top() - SECTION_GAP;
    let unstaged_top = header_rect.bottom() + SECTION_GAP;
    let unstaged_rect = egui::Rect::from_min_max(
        egui::pos2(content_left, unstaged_top),
        egui::pos2(content_right, unstaged_bottom),
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("floating_commit_unstaged")
            .max_rect(unstaged_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            if let Some(s) = status {
                top_strip(ui, s, muted);
            } else {
                top_strip_empty(ui, muted);
            }
            ui.add_space(4.0);
            if let Some(s) = status {
                unstaged_files_list(ui, &s.unstaged_files, muted, state);
            }
            ui.add_space(4.0);
            section_divider(ui);
            ui.add_space(4.0);
            if let Some(s) = status {
                staged_files_list(ui, &s.staged_files, muted, state);
            }
        },
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("floating_commit_msg")
            .max_rect(msg_box_rect),
        |ui| {
            message_box(ui, state);
        },
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("floating_commit_footer")
            .max_rect(footer_rect.shrink2(egui::vec2(CONTENT_PAD, 4.0)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            actions(ui, state);
        },
    );

    if state.show_discard_confirm {
        show_discard_confirm(ui, panel_rect, state);
    }
}

fn render_panel_cached(
    ui: &mut egui::Ui,
    panel_rect: egui::Rect,
    state: &mut State,
    header_text: &str,
    app_state: &AppState,
) {
    let fill = egui::Color32::from_rgb(36, 36, 36);
    let header_fill = egui::Color32::from_rgb(44, 44, 44);
    let footer_fill = egui::Color32::from_rgb(40, 40, 40);
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(78, 78, 78));
    let muted = egui::Color32::from_rgb(172, 172, 172);

    ui.painter().rect_filled(
        panel_rect.translate(egui::vec2(3.0, 3.0)),
        6,
        egui::Color32::from_black_alpha(80),
    );
    ui.painter().rect_filled(panel_rect, 6, fill);
    ui.painter()
        .rect_stroke(panel_rect, 6, stroke, egui::StrokeKind::Inside);

    let header_rect = egui::Rect::from_min_size(
        panel_rect.left_top(),
        egui::vec2(panel_rect.width(), HEADER_H),
    );
    ui.painter().rect_filled(
        header_rect,
        egui::CornerRadius {
            nw: 6,
            ne: 6,
            sw: 0,
            se: 0,
        },
        header_fill,
    );
    ui.painter().line_segment(
        [header_rect.left_bottom(), header_rect.right_bottom()],
        stroke,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.left() + 12.0, header_rect.center().y),
        GIT_COMMIT,
        15.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.left() + 34.0, header_rect.center().y),
        &format!("Commit to {}", header_text),
        12.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );
    if let Some(s) = &app_state.cached_status {
        header_stats(ui, header_rect, s.additions, s.deletions, s.files_changed);
    }

    let footer_rect = egui::Rect::from_min_size(
        egui::pos2(panel_rect.left(), panel_rect.bottom() - FOOTER_H),
        egui::vec2(panel_rect.width(), FOOTER_H),
    );
    ui.painter().rect_filled(
        footer_rect,
        egui::CornerRadius {
            nw: 0,
            ne: 0,
            sw: 6,
            se: 6,
        },
        footer_fill,
    );
    ui.painter()
        .line_segment([footer_rect.left_top(), footer_rect.right_top()], stroke);

    let msg_box_rect = egui::Rect::from_min_max(
        egui::pos2(
            panel_rect.left() + CONTENT_PAD,
            footer_rect.top() - MSG_BOX_H - SECTION_GAP,
        ),
        egui::pos2(
            panel_rect.right() - CONTENT_PAD,
            footer_rect.top() - SECTION_GAP,
        ),
    );

    let content_left = panel_rect.left() + CONTENT_PAD;
    let content_right = panel_rect.right() - CONTENT_PAD;

    let unstaged_bottom = msg_box_rect.top() - SECTION_GAP;
    let unstaged_top = header_rect.bottom() + SECTION_GAP;
    let unstaged_rect = egui::Rect::from_min_max(
        egui::pos2(content_left, unstaged_top),
        egui::pos2(content_right, unstaged_bottom),
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("floating_commit_unstaged")
            .max_rect(unstaged_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            if let Some(s) = &app_state.cached_status {
                top_strip_cached(ui, s, muted);
            } else {
                top_strip_empty(ui, muted);
            }
            ui.add_space(4.0);
            if let Some(s) = &app_state.cached_status {
                unstaged_files_list_cached(ui, &s.unstaged_files, muted, state);
            }
            ui.add_space(4.0);
            section_divider(ui);
            ui.add_space(4.0);
            if let Some(s) = &app_state.cached_status {
                staged_files_list_cached(ui, &s.staged_files, muted, state);
            }
        },
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("floating_commit_msg")
            .max_rect(msg_box_rect),
        |ui| {
            message_box(ui, state);
        },
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("floating_commit_footer")
            .max_rect(footer_rect.shrink2(egui::vec2(CONTENT_PAD, 4.0)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            actions_cached(ui, state);
        },
    );

    if state.show_discard_confirm {
        show_discard_confirm(ui, panel_rect, state);
    }
}

fn top_strip(ui: &mut egui::Ui, status: &crate::git::models::RepoStatus, muted: egui::Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
        icon_label(ui, GIT_BRANCH, &status.branch, "Current branch", muted);
        separator(ui);
        icon_label(
            ui,
            LIST_CHECKS,
            &format!("{}", status.staged_count),
            "Staged",
            muted,
        );
        separator(ui);
        icon_label(
            ui,
            WARNING,
            &format!("{}", status.unstaged_count),
            "Unstaged",
            muted,
        );
    });
}

fn top_strip_cached(
    ui: &mut egui::Ui,
    status: &crate::state::CachedRepoStatus,
    muted: egui::Color32,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
        icon_label(ui, GIT_BRANCH, &status.branch, "Current branch", muted);
        separator(ui);
        icon_label(
            ui,
            LIST_CHECKS,
            &format!("{}", status.staged_count),
            "Staged",
            muted,
        );
        separator(ui);
        icon_label(
            ui,
            WARNING,
            &format!("{}", status.unstaged_count),
            "Unstaged",
            muted,
        );
    });
}

fn top_strip_empty(ui: &mut egui::Ui, muted: egui::Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(7.0, 0.0);
        icon_label(ui, GIT_BRANCH, "no repo", "No repository open", muted);
    });
}

fn header_stats(
    ui: &egui::Ui,
    header_rect: egui::Rect,
    additions: usize,
    deletions: usize,
    files: usize,
) {
    let y = header_rect.center().y;
    painter_text(
        ui,
        egui::pos2(header_rect.right() - 110.0, y),
        &format!("+{}", additions),
        11.0,
        egui::Color32::from_rgb(78, 190, 116),
        egui::Align2::LEFT_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.right() - 74.0, y),
        &format!("-{}", deletions),
        11.0,
        egui::Color32::from_rgb(230, 92, 92),
        egui::Align2::LEFT_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.right() - 40.0, y),
        &format!("{}", files),
        11.0,
        egui::Color32::from_rgb(172, 172, 172),
        egui::Align2::LEFT_CENTER,
    );
}

fn message_box(ui: &mut egui::Ui, state: &mut State) {
    let section_rect = ui.max_rect();
    let section_fill = egui::Color32::from_rgb(40, 40, 40);
    let section_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 72, 72));
    let editor_fill = egui::Color32::from_rgb(49, 49, 49);

    ui.painter().rect_filled(section_rect, 6, section_fill);
    ui.painter()
        .rect_stroke(section_rect, 6, section_stroke, egui::StrokeKind::Inside);

    let inner_rect = section_rect.shrink2(egui::vec2(6.0, 6.0));
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(inner_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);

            let title_len = state.title.chars().count();
            let remaining = MAX_TITLE_LEN as i64 - title_len as i64;

            let title_color = if remaining < 0 {
                egui::Color32::from_rgb(230, 92, 92)
            } else if remaining <= 10 {
                egui::Color32::from_rgb(252, 197, 34)
            } else {
                egui::Color32::from_rgb(140, 140, 140)
            };

            ui.horizontal(|ui| {
                let title_edit = egui::TextEdit::singleline(&mut state.title)
                    .hint_text("Commit title")
                    .frame(egui::Frame::NONE)
                    .background_color(editor_fill)
                    .desired_width((ui.available_width() - 40.0).max(0.0));
                ui.add(title_edit);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("{}", remaining))
                            .size(9.0)
                            .color(title_color),
                    );
                });
            });

            let desc_edit = egui::TextEdit::multiline(&mut state.description)
                .hint_text("Description (optional)")
                .frame(egui::Frame::NONE)
                .background_color(editor_fill)
                .desired_rows(2);
            ui.add_sized([ui.available_width(), 36.0], desc_edit);
        },
    );
}

fn unstaged_files_list(
    ui: &mut egui::Ui,
    files: &[crate::git::models::FileStatus],
    muted: egui::Color32,
    state: &mut State,
) {
    if files.is_empty() {
        ui.label(
            egui::RichText::new("No unstaged changes")
                .size(10.0)
                .color(muted),
        );
        return;
    }

    let needs_scroll = files.len() > SCROLL_THRESHOLD;
    let visible_count = if needs_scroll {
        MAX_VISIBLE_FILES
    } else {
        files.len().min(MAX_VISIBLE_FILES)
    };
    let list_height = clamped_list_height(ui, visible_count);

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("unstaged_files_scroll")
            .max_rect(egui::Rect::from_min_size(
                ui.available_rect_before_wrap().left_top(),
                egui::vec2(ui.available_width(), list_height),
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt("unstaged_files")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    section_header(ui, "Unstaged", files.len(), muted);
                    for file in files {
                        file_row_unstaged(ui, file, muted, state);
                    }
                });
        },
    );
}

fn unstaged_files_list_cached(
    ui: &mut egui::Ui,
    files: &[CachedFileStatus],
    muted: egui::Color32,
    state: &mut State,
) {
    if files.is_empty() {
        ui.label(
            egui::RichText::new("No unstaged changes")
                .size(10.0)
                .color(muted),
        );
        return;
    }

    let needs_scroll = files.len() > SCROLL_THRESHOLD;
    let visible_count = if needs_scroll {
        MAX_VISIBLE_FILES
    } else {
        files.len().min(MAX_VISIBLE_FILES)
    };
    let list_height = clamped_list_height(ui, visible_count);

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("unstaged_files_scroll")
            .max_rect(egui::Rect::from_min_size(
                ui.available_rect_before_wrap().left_top(),
                egui::vec2(ui.available_width(), list_height),
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt("unstaged_files")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    section_header(ui, "Unstaged", files.len(), muted);
                    for file in files {
                        file_row_unstaged_cached(ui, file, muted, state);
                    }
                });
        },
    );
}

fn staged_files_list(
    ui: &mut egui::Ui,
    files: &[crate::git::models::FileStatus],
    muted: egui::Color32,
    state: &mut State,
) {
    if files.is_empty() {
        return;
    }

    let needs_scroll = files.len() > SCROLL_THRESHOLD;
    let visible_count = if needs_scroll {
        MAX_VISIBLE_FILES
    } else {
        files.len().min(MAX_VISIBLE_FILES)
    };
    let list_height = clamped_list_height(ui, visible_count);

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("staged_files_scroll")
            .max_rect(egui::Rect::from_min_size(
                ui.available_rect_before_wrap().left_top(),
                egui::vec2(ui.available_width(), list_height),
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt("staged_files")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    section_header(ui, "Staged", files.len(), muted);
                    for file in files {
                        file_row_staged(ui, file, muted, state);
                    }
                });
        },
    );
}

fn staged_files_list_cached(
    ui: &mut egui::Ui,
    files: &[CachedFileStatus],
    muted: egui::Color32,
    state: &mut State,
) {
    if files.is_empty() {
        return;
    }

    let needs_scroll = files.len() > SCROLL_THRESHOLD;
    let visible_count = if needs_scroll {
        MAX_VISIBLE_FILES
    } else {
        files.len().min(MAX_VISIBLE_FILES)
    };
    let list_height = clamped_list_height(ui, visible_count);

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("staged_files_scroll")
            .max_rect(egui::Rect::from_min_size(
                ui.available_rect_before_wrap().left_top(),
                egui::vec2(ui.available_width(), list_height),
            ))
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt("staged_files")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    section_header(ui, "Staged", files.len(), muted);
                    for file in files {
                        file_row_staged_cached(ui, file, muted, state);
                    }
                });
        },
    );
}

fn section_header(ui: &mut egui::Ui, label: &str, count: usize, muted: egui::Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
        ui.label(egui::RichText::new(label).size(9.0).color(muted).strong());
        ui.label(
            egui::RichText::new(format!("({})", count))
                .size(9.0)
                .color(egui::Color32::from_rgb(120, 120, 120)),
        );
    });
}

fn file_row_unstaged(
    ui: &mut egui::Ui,
    file: &crate::git::models::FileStatus,
    _muted: egui::Color32,
    state: &mut State,
) {
    let (icon, icon_color) = file_icon_for_kind(&file.kind);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), FILE_ROW_HEIGHT),
        egui::Sense::click(),
    );

    let hovered = rect.contains(
        ui.input(|i| i.pointer.hover_pos())
            .unwrap_or(egui::Pos2::ZERO),
    );
    if hovered {
        ui.painter()
            .rect_filled(rect, 3.0, egui::Color32::from_rgb(48, 48, 48));
    }

    let icon_x = rect.left() + 4.0;
    let icon_y = rect.center().y;
    painter_text(
        ui,
        egui::pos2(icon_x, icon_y),
        icon,
        11.0,
        icon_color,
        egui::Align2::CENTER_CENTER,
    );

    let path_x = icon_x + 16.0;
    let path_width = rect.width() - 80.0 - (path_x - rect.left());
    let display = truncate_path(&file.path, path_width, 10.0);
    painter_text(
        ui,
        egui::pos2(path_x, rect.center().y),
        &display,
        10.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );

    let stats_x = rect.right() - 72.0;
    if file.additions > 0 || file.deletions > 0 {
        painter_text(
            ui,
            egui::pos2(stats_x, rect.center().y),
            &format!("+{}", file.additions),
            9.0,
            egui::Color32::from_rgb(78, 190, 116),
            egui::Align2::LEFT_CENTER,
        );
        painter_text(
            ui,
            egui::pos2(stats_x + 32.0, rect.center().y),
            &format!("-{}", file.deletions),
            9.0,
            egui::Color32::from_rgb(230, 92, 92),
            egui::Align2::LEFT_CENTER,
        );
    }

    if hovered {
        let btn_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 14.0, rect.center().y),
            egui::vec2(18.0, 18.0),
        );
        let btn_resp = ui.scope_builder(egui::UiBuilder::new().max_rect(btn_rect), |ui| {
            ui.button(egui::RichText::new(PLUS.to_string()).size(10.0))
        });
        if btn_resp.inner.clicked() {
            state.queue_action(CommitAction::StageFile(file.path.clone()));
        }
    }
}

fn file_row_unstaged_cached(
    ui: &mut egui::Ui,
    file: &CachedFileStatus,
    _muted: egui::Color32,
    state: &mut State,
) {
    let (icon, icon_color) = cached_file_icon_for_kind(&file.kind);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), FILE_ROW_HEIGHT),
        egui::Sense::click(),
    );

    let hovered = rect.contains(
        ui.input(|i| i.pointer.hover_pos())
            .unwrap_or(egui::Pos2::ZERO),
    );
    if hovered {
        ui.painter()
            .rect_filled(rect, 3.0, egui::Color32::from_rgb(48, 48, 48));
    }

    let icon_x = rect.left() + 4.0;
    let icon_y = rect.center().y;
    painter_text(
        ui,
        egui::pos2(icon_x, icon_y),
        icon,
        11.0,
        icon_color,
        egui::Align2::CENTER_CENTER,
    );

    let path_x = icon_x + 16.0;
    let path_width = rect.width() - 80.0 - (path_x - rect.left());
    let display = truncate_path(&file.path, path_width, 10.0);
    painter_text(
        ui,
        egui::pos2(path_x, rect.center().y),
        &display,
        10.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );

    let stats_x = rect.right() - 72.0;
    if file.additions > 0 || file.deletions > 0 {
        painter_text(
            ui,
            egui::pos2(stats_x, rect.center().y),
            &format!("+{}", file.additions),
            9.0,
            egui::Color32::from_rgb(78, 190, 116),
            egui::Align2::LEFT_CENTER,
        );
        painter_text(
            ui,
            egui::pos2(stats_x + 32.0, rect.center().y),
            &format!("-{}", file.deletions),
            9.0,
            egui::Color32::from_rgb(230, 92, 92),
            egui::Align2::LEFT_CENTER,
        );
    }

    if hovered {
        let btn_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 14.0, rect.center().y),
            egui::vec2(18.0, 18.0),
        );
        let btn_resp = ui.scope_builder(egui::UiBuilder::new().max_rect(btn_rect), |ui| {
            ui.button(egui::RichText::new(PLUS.to_string()).size(10.0))
        });
        if btn_resp.inner.clicked() {
            state.queue_action(CommitAction::StageFile(file.path.clone()));
        }
    }
}

fn file_row_staged(
    ui: &mut egui::Ui,
    file: &crate::git::models::FileStatus,
    _muted: egui::Color32,
    state: &mut State,
) {
    let (icon, icon_color) = file_icon_for_kind(&file.kind);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), FILE_ROW_HEIGHT),
        egui::Sense::click(),
    );

    let hovered = rect.contains(
        ui.input(|i| i.pointer.hover_pos())
            .unwrap_or(egui::Pos2::ZERO),
    );
    if hovered {
        ui.painter()
            .rect_filled(rect, 3.0, egui::Color32::from_rgb(48, 48, 48));
    }

    let icon_x = rect.left() + 4.0;
    let icon_y = rect.center().y;
    painter_text(
        ui,
        egui::pos2(icon_x, icon_y),
        icon,
        11.0,
        icon_color,
        egui::Align2::CENTER_CENTER,
    );

    let path_x = icon_x + 16.0;
    let path_width = rect.width() - 80.0 - (path_x - rect.left());
    let display = truncate_path(&file.path, path_width, 10.0);
    painter_text(
        ui,
        egui::pos2(path_x, rect.center().y),
        &display,
        10.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );

    let stats_x = rect.right() - 72.0;
    if file.additions > 0 || file.deletions > 0 {
        painter_text(
            ui,
            egui::pos2(stats_x, rect.center().y),
            &format!("+{}", file.additions),
            9.0,
            egui::Color32::from_rgb(78, 190, 116),
            egui::Align2::LEFT_CENTER,
        );
        painter_text(
            ui,
            egui::pos2(stats_x + 32.0, rect.center().y),
            &format!("-{}", file.deletions),
            9.0,
            egui::Color32::from_rgb(230, 92, 92),
            egui::Align2::LEFT_CENTER,
        );
    }

    if hovered {
        let unstage_btn_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 14.0, rect.center().y),
            egui::vec2(18.0, 18.0),
        );
        let unstage_resp = ui
            .scope_builder(egui::UiBuilder::new().max_rect(unstage_btn_rect), |ui| {
                ui.button(egui::RichText::new(X.to_string()).size(10.0))
            });
        if unstage_resp.inner.clicked() {
            state.queue_action(CommitAction::UnstageFile(file.path.clone()));
        }
    }
}

fn file_row_staged_cached(
    ui: &mut egui::Ui,
    file: &CachedFileStatus,
    _muted: egui::Color32,
    state: &mut State,
) {
    let (icon, icon_color) = cached_file_icon_for_kind(&file.kind);
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), FILE_ROW_HEIGHT),
        egui::Sense::click(),
    );

    let hovered = rect.contains(
        ui.input(|i| i.pointer.hover_pos())
            .unwrap_or(egui::Pos2::ZERO),
    );
    if hovered {
        ui.painter()
            .rect_filled(rect, 3.0, egui::Color32::from_rgb(48, 48, 48));
    }

    let icon_x = rect.left() + 4.0;
    let icon_y = rect.center().y;
    painter_text(
        ui,
        egui::pos2(icon_x, icon_y),
        icon,
        11.0,
        icon_color,
        egui::Align2::CENTER_CENTER,
    );

    let path_x = icon_x + 16.0;
    let path_width = rect.width() - 80.0 - (path_x - rect.left());
    let display = truncate_path(&file.path, path_width, 10.0);
    painter_text(
        ui,
        egui::pos2(path_x, rect.center().y),
        &display,
        10.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );

    let stats_x = rect.right() - 72.0;
    if file.additions > 0 || file.deletions > 0 {
        painter_text(
            ui,
            egui::pos2(stats_x, rect.center().y),
            &format!("+{}", file.additions),
            9.0,
            egui::Color32::from_rgb(78, 190, 116),
            egui::Align2::LEFT_CENTER,
        );
        painter_text(
            ui,
            egui::pos2(stats_x + 32.0, rect.center().y),
            &format!("-{}", file.deletions),
            9.0,
            egui::Color32::from_rgb(230, 92, 92),
            egui::Align2::LEFT_CENTER,
        );
    }

    if hovered {
        let unstage_btn_rect = egui::Rect::from_center_size(
            egui::pos2(rect.right() - 14.0, rect.center().y),
            egui::vec2(18.0, 18.0),
        );
        let unstage_resp = ui
            .scope_builder(egui::UiBuilder::new().max_rect(unstage_btn_rect), |ui| {
                ui.button(egui::RichText::new(X.to_string()).size(10.0))
            });
        if unstage_resp.inner.clicked() {
            state.queue_action(CommitAction::UnstageFile(file.path.clone()));
        }
    }
}

fn file_icon_for_kind(kind: &crate::git::models::FileChangeKind) -> (&'static str, egui::Color32) {
    use crate::git::models::FileChangeKind;
    match kind {
        FileChangeKind::Added => (FILE_PLUS, egui::Color32::from_rgb(78, 190, 116)),
        FileChangeKind::Modified => (FILE, egui::Color32::from_rgb(252, 197, 34)),
        FileChangeKind::Deleted => (TRASH, egui::Color32::from_rgb(230, 92, 92)),
        FileChangeKind::Renamed => (ARROW_DOWN, egui::Color32::from_rgb(151, 113, 255)),
        FileChangeKind::TypeChanged => (FOLDER, egui::Color32::from_rgb(172, 172, 172)),
    }
}

fn cached_file_icon_for_kind(kind: &CachedFileChangeKind) -> (&'static str, egui::Color32) {
    match kind {
        CachedFileChangeKind::Added => (FILE_PLUS, egui::Color32::from_rgb(78, 190, 116)),
        CachedFileChangeKind::Modified => (FILE, egui::Color32::from_rgb(252, 197, 34)),
        CachedFileChangeKind::Deleted => (TRASH, egui::Color32::from_rgb(230, 92, 92)),
        CachedFileChangeKind::Renamed => (ARROW_DOWN, egui::Color32::from_rgb(151, 113, 255)),
        CachedFileChangeKind::TypeChanged => (FOLDER, egui::Color32::from_rgb(172, 172, 172)),
    }
}

fn truncate_path(path: &str, max_width: f32, font_size: f32) -> String {
    let char_count = path.chars().count();
    if char_count as f32 * font_size * 0.55 < max_width {
        return path.to_string();
    }

    let parts: Vec<&str> = path.split('/').collect();
    if parts.len() <= 2 {
        let max_chars = (max_width / (font_size * 0.55)) as usize;
        if char_count > max_chars {
            let keep = max_chars.saturating_sub(3);
            let suffix: String = path.chars().skip(char_count.saturating_sub(keep)).collect();
            return format!("...{}", suffix);
        }
        return path.to_string();
    }

    let file_name = parts.last().unwrap();
    let file_char_count = file_name.chars().count();
    let max_chars = (max_width / (font_size * 0.55)) as usize - 4;
    if file_char_count + 4 < max_chars {
        return format!("…/{}", file_name);
    }

    let keep = max_chars.saturating_sub(4);
    let truncated: String = file_name
        .chars()
        .skip(file_char_count.saturating_sub(keep))
        .collect();
    format!("…/…{}", truncated)
}

fn actions(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
        ui.spacing_mut().interact_size = egui::vec2(0.0, 22.0);
        ui.checkbox(&mut state.amend, "Amend");
        ui.checkbox(&mut state.sign_off, "Sign-off");
        separator(ui);
        if ui
            .button(egui::RichText::new(format!("{PLUS} All")).size(10.0))
            .clicked()
        {
            state.queue_action(CommitAction::StageAll);
        }
        if ui
            .button(egui::RichText::new(format!("{TRASH} All")).size(10.0))
            .clicked()
        {
            state.show_discard_confirm = true;
        }
    });
}

fn actions_cached(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
        ui.spacing_mut().interact_size = egui::vec2(0.0, 22.0);
        ui.checkbox(&mut state.amend, "Amend");
        ui.checkbox(&mut state.sign_off, "Sign-off");
        separator(ui);
        if ui
            .button(egui::RichText::new(format!("{PLUS} All")).size(10.0))
            .clicked()
        {
            state.queue_action(CommitAction::StageAll);
        }
        if ui
            .button(egui::RichText::new(format!("{TRASH} All")).size(10.0))
            .clicked()
        {
            state.show_discard_confirm = true;
        }
    });
}

fn show_discard_confirm(ui: &mut egui::Ui, panel_rect: egui::Rect, state: &mut State) {
    let confirm_rect = panel_rect.shrink2(egui::vec2(20.0, 40.0));
    ui.painter()
        .rect_filled(confirm_rect, 6, egui::Color32::from_rgb(50, 50, 50));
    ui.painter().rect_stroke(
        confirm_rect,
        6,
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(100, 100, 100)),
        egui::StrokeKind::Inside,
    );

    let msg_y = confirm_rect.top() + 20.0;
    painter_text(
        ui,
        egui::pos2(confirm_rect.center().x, msg_y),
        "Discard all changes?",
        12.0,
        ui.visuals().text_color(),
        egui::Align2::CENTER_CENTER,
    );

    let btn_y = confirm_rect.bottom() - 18.0;
    let cancel_rect = egui::Rect::from_center_size(
        egui::pos2(confirm_rect.center().x - 40.0, btn_y),
        egui::vec2(60.0, 22.0),
    );
    let confirm_btn_rect = egui::Rect::from_center_size(
        egui::pos2(confirm_rect.center().x + 40.0, btn_y),
        egui::vec2(60.0, 22.0),
    );

    let cancel_resp = ui.interact(
        cancel_rect,
        ui.make_persistent_id("discard_cancel"),
        egui::Sense::click(),
    );
    ui.painter()
        .rect_filled(cancel_rect, 3.0, egui::Color32::from_rgb(60, 60, 60));
    painter_text(
        ui,
        cancel_rect.center(),
        "Cancel",
        10.0,
        ui.visuals().text_color(),
        egui::Align2::CENTER_CENTER,
    );
    if cancel_resp.clicked() {
        state.show_discard_confirm = false;
    }

    let confirm_resp = ui.interact(
        confirm_btn_rect,
        ui.make_persistent_id("discard_confirm"),
        egui::Sense::click(),
    );
    ui.painter()
        .rect_filled(confirm_btn_rect, 3.0, egui::Color32::from_rgb(180, 60, 60));
    painter_text(
        ui,
        confirm_btn_rect.center(),
        "Discard",
        10.0,
        egui::Color32::WHITE,
        egui::Align2::CENTER_CENTER,
    );
    if confirm_resp.clicked() {
        state.show_discard_confirm = false;
        state.queue_action(CommitAction::DiscardAll);
    }
}

fn icon_label(ui: &mut egui::Ui, icon: &str, value: &str, tooltip: &str, muted: egui::Color32) {
    ui.label(egui::RichText::new(icon).size(12.0).color(muted))
        .on_hover_text(tooltip);
    ui.label(egui::RichText::new(value).size(10.0));
}

fn separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 12.0), egui::Sense::hover());
    ui.painter().line_segment(
        [rect.center_top(), rect.center_bottom()],
        egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 72, 72)),
    );
}

fn painter_text(
    ui: &egui::Ui,
    pos: egui::Pos2,
    text: &str,
    size: f32,
    color: egui::Color32,
    align: egui::Align2,
) {
    ui.painter()
        .text(pos, align, text, egui::FontId::proportional(size), color);
}
