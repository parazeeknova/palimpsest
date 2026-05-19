use eframe::egui;
use egui_phosphor::regular::{
    ARROW_CLOCKWISE, CHECK, FILE, GIT_BRANCH, GIT_COMMIT, LIST_CHECKS, PAPER_PLANE, PLUS, WARNING,
};

use crate::git::GitRepo;
use crate::state::AppState;

const PANEL_HEIGHT: f32 = 188.0;
const PANEL_MARGIN: f32 = 18.0;

#[derive(Default)]
pub struct State {
    pub message: String,
    pub amend: bool,
    pub sign_off: bool,
}

pub fn show(
    ui: &mut egui::Ui,
    body_rect: egui::Rect,
    state: &mut State,
    git_repo: Option<&GitRepo>,
) {
    let width = (body_rect.width() * 0.56)
        .clamp(620.0, body_rect.width() - PANEL_MARGIN * 2.0)
        .max(320.0);
    let height = PANEL_HEIGHT.min((body_rect.height() - PANEL_MARGIN * 2.0).max(128.0));
    let panel_rect = egui::Rect::from_min_size(
        egui::pos2(
            body_rect.center().x - width * 0.5,
            body_rect.bottom() - height - PANEL_MARGIN,
        ),
        egui::vec2(width, height),
    );

    let fill = egui::Color32::from_rgb(36, 36, 36);
    let header_fill = egui::Color32::from_rgb(44, 44, 44);
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(78, 78, 78));
    let muted = egui::Color32::from_rgb(172, 172, 172);

    ui.painter().rect_filled(
        panel_rect.translate(egui::vec2(3.0, 3.0)),
        4.0,
        egui::Color32::from_black_alpha(80),
    );
    ui.painter().rect_filled(panel_rect, 4.0, fill);
    ui.painter()
        .rect_stroke(panel_rect, 4.0, stroke, egui::StrokeKind::Inside);

    let header_text = if let Some(repo) = git_repo {
        repo.head_branch().unwrap_or_else(|_| "HEAD".to_string())
    } else {
        "no repository".to_string()
    };

    let header_rect =
        egui::Rect::from_min_size(panel_rect.left_top(), egui::vec2(panel_rect.width(), 30.0));
    ui.painter().rect_filled(header_rect, 4.0, header_fill);
    ui.painter().line_segment(
        [header_rect.left_bottom(), header_rect.right_bottom()],
        stroke,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.left() + 14.0, header_rect.center().y),
        GIT_COMMIT,
        17.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.left() + 38.0, header_rect.center().y),
        &format!("Commit to {}", header_text),
        14.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );

    if let Some(repo) = git_repo {
        if let Ok(status) = repo.status() {
            header_stats(
                ui,
                header_rect,
                status.additions,
                status.deletions,
                status.files_changed,
            );
        }
    }

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("floating_commit_panel")
            .max_rect(panel_rect.shrink2(egui::vec2(12.0, 8.0)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.add_space(28.0);
            if let Some(repo) = git_repo {
                if let Ok(status) = repo.status() {
                    top_strip(ui, &status, muted);
                }
            } else {
                top_strip_empty(ui, muted);
            }
            ui.add_space(7.0);
            middle_row(ui, state, muted, git_repo);
            ui.add_space(8.0);
            actions(ui, state);
        },
    );
}

pub fn show_cached(
    ui: &mut egui::Ui,
    body_rect: egui::Rect,
    state: &mut State,
    app_state: &AppState,
) {
    let width = (body_rect.width() * 0.56)
        .clamp(620.0, body_rect.width() - PANEL_MARGIN * 2.0)
        .max(320.0);
    let height = PANEL_HEIGHT.min((body_rect.height() - PANEL_MARGIN * 2.0).max(128.0));
    let panel_rect = egui::Rect::from_min_size(
        egui::pos2(
            body_rect.center().x - width * 0.5,
            body_rect.bottom() - height - PANEL_MARGIN,
        ),
        egui::vec2(width, height),
    );

    let fill = egui::Color32::from_rgb(36, 36, 36);
    let header_fill = egui::Color32::from_rgb(44, 44, 44);
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(78, 78, 78));
    let muted = egui::Color32::from_rgb(172, 172, 172);

    ui.painter().rect_filled(
        panel_rect.translate(egui::vec2(3.0, 3.0)),
        4.0,
        egui::Color32::from_black_alpha(80),
    );
    ui.painter().rect_filled(panel_rect, 4.0, fill);
    ui.painter()
        .rect_stroke(panel_rect, 4.0, stroke, egui::StrokeKind::Inside);

    let header_text = app_state
        .cached_status
        .as_ref()
        .map(|s| s.branch.clone())
        .unwrap_or_else(|| "HEAD".to_string());

    let header_rect =
        egui::Rect::from_min_size(panel_rect.left_top(), egui::vec2(panel_rect.width(), 30.0));
    ui.painter().rect_filled(header_rect, 4.0, header_fill);
    ui.painter().line_segment(
        [header_rect.left_bottom(), header_rect.right_bottom()],
        stroke,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.left() + 14.0, header_rect.center().y),
        GIT_COMMIT,
        17.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.left() + 38.0, header_rect.center().y),
        &format!("Commit to {}", header_text),
        14.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );

    if let Some(status) = &app_state.cached_status {
        header_stats(
            ui,
            header_rect,
            status.additions,
            status.deletions,
            status.files_changed,
        );
    }

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("floating_commit_panel")
            .max_rect(panel_rect.shrink2(egui::vec2(12.0, 8.0)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            ui.add_space(28.0);
            if let Some(status) = &app_state.cached_status {
                top_strip_cached(ui, status, muted);
            } else {
                top_strip_empty(ui, muted);
            }
            ui.add_space(7.0);
            middle_row_cached(ui, state, muted, app_state);
            ui.add_space(8.0);
            actions(ui, state);
        },
    );
}

fn top_strip(ui: &mut egui::Ui, status: &crate::git::models::RepoStatus, muted: egui::Color32) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(7.0, 0.0);
        icon_label(ui, GIT_BRANCH, &status.branch, "Current branch", muted);
        separator(ui);
        icon_label(
            ui,
            LIST_CHECKS,
            &format!("{} staged", status.staged_count),
            "Index",
            muted,
        );
        separator(ui);
        icon_label(
            ui,
            WARNING,
            &format!("{} unstaged", status.unstaged_count),
            "Working tree",
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
        ui.spacing_mut().item_spacing = egui::vec2(7.0, 0.0);
        icon_label(ui, GIT_BRANCH, &status.branch, "Current branch", muted);
        separator(ui);
        icon_label(
            ui,
            LIST_CHECKS,
            &format!("{} staged", status.staged_count),
            "Index",
            muted,
        );
        separator(ui);
        icon_label(
            ui,
            WARNING,
            &format!("{} unstaged", status.unstaged_count),
            "Working tree",
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
        egui::pos2(header_rect.right() - 148.0, y),
        &format!("+{}", additions),
        12.0,
        egui::Color32::from_rgb(78, 190, 116),
        egui::Align2::LEFT_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.right() - 102.0, y),
        &format!("-{}", deletions),
        12.0,
        egui::Color32::from_rgb(230, 92, 92),
        egui::Align2::LEFT_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(header_rect.right() - 58.0, y),
        &format!("{} files", files),
        12.0,
        egui::Color32::from_rgb(172, 172, 172),
        egui::Align2::LEFT_CENTER,
    );
}

fn middle_row(
    ui: &mut egui::Ui,
    state: &mut State,
    muted: egui::Color32,
    git_repo: Option<&GitRepo>,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
        message_box(ui, state);
        if let Some(repo) = git_repo {
            if let Ok(status) = repo.status() {
                staged_files(ui, &status, muted);
            }
        }
    });
}

fn middle_row_cached(
    ui: &mut egui::Ui,
    state: &mut State,
    muted: egui::Color32,
    app_state: &AppState,
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);
        message_box(ui, state);
        if let Some(status) = &app_state.cached_status {
            staged_files_cached(ui, status, muted);
        }
    });
}

fn message_box(ui: &mut egui::Ui, state: &mut State) {
    let edit = egui::TextEdit::multiline(&mut state.message)
        .hint_text("Summarize the change")
        .desired_rows(3);
    ui.add_sized([(ui.available_width() * 0.62).max(320.0), 64.0], edit);
}

fn staged_files(ui: &mut egui::Ui, status: &crate::git::models::RepoStatus, muted: egui::Color32) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
        ui.label(
            egui::RichText::new(format!("{FILE} {} staged", status.staged_files.len()))
                .size(11.0)
                .color(muted),
        );
        for file in &status.staged_files {
            let display = file.chars().take(25).collect::<String>();
            file_chip(ui, &display);
        }
        if status.staged_files.is_empty() {
            ui.label(
                egui::RichText::new("No staged files")
                    .size(11.0)
                    .color(muted),
            );
        }
    });
}

fn staged_files_cached(
    ui: &mut egui::Ui,
    status: &crate::state::CachedRepoStatus,
    muted: egui::Color32,
) {
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);
        ui.label(
            egui::RichText::new(format!("{FILE} {} staged", status.staged_files.len()))
                .size(11.0)
                .color(muted),
        );
        for file in &status.staged_files {
            let display = file.chars().take(25).collect::<String>();
            file_chip(ui, &display);
        }
        if status.staged_files.is_empty() {
            ui.label(
                egui::RichText::new("No staged files")
                    .size(11.0)
                    .color(muted),
            );
        }
    });
}

fn actions(ui: &mut egui::Ui, state: &mut State) {
    ui.horizontal(|ui| {
        ui.checkbox(&mut state.amend, "Amend");
        ui.checkbox(&mut state.sign_off, "Sign-off");
        separator(ui);
        drop(ui.button(egui::RichText::new(format!("{ARROW_CLOCKWISE} Refresh")).size(12.0)));
        drop(ui.button(egui::RichText::new(format!("{PLUS} Stage all")).size(12.0)));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let can_commit = !state.message.trim().is_empty();
            ui.add_enabled(
                can_commit,
                egui::Button::new(egui::RichText::new(format!("{PAPER_PLANE} Commit")).size(12.0)),
            );
            ui.add_enabled(
                can_commit,
                egui::Button::new(egui::RichText::new(format!("{CHECK} Ready")).size(12.0)),
            );
        });
    });
}

fn icon_label(ui: &mut egui::Ui, icon: &str, value: &str, tooltip: &str, muted: egui::Color32) {
    ui.label(egui::RichText::new(icon).size(13.0).color(muted))
        .on_hover_text(tooltip);
    ui.label(egui::RichText::new(value).size(11.0));
}

fn file_chip(ui: &mut egui::Ui, label: &str) {
    let width = (label.len() as f32 * 6.2 + 20.0).clamp(92.0, 170.0);
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, 18.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 3.0, egui::Color32::from_rgb(48, 48, 48));
    painter_text(
        ui,
        egui::pos2(rect.left() + 8.0, rect.center().y),
        label,
        10.5,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );
}

fn separator(ui: &mut egui::Ui) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(1.0, 14.0), egui::Sense::hover());
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
