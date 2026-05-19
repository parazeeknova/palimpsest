use eframe::egui;
use egui_phosphor::regular::{
    CARET_DOWN, CARET_RIGHT, CHECK, EYE, FILE_TEXT, FOLDER, FUNNEL, GEAR_SIX, GIT_BRANCH,
    GITHUB_LOGO, LIST, MAGNIFYING_GLASS,
};

use crate::git::GitRepo;
use crate::state::AppState;

pub const SIDEBAR_WIDTH: f32 = 236.0;
const HEADER_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 24.0;
const FILTER_HEIGHT: f32 = 26.0;

#[allow(unused_assignments)]
pub fn show(ui: &mut egui::Ui, repo_name: Option<&str>, git_repo: Option<&GitRepo>) {
    let height = ui.available_height();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(SIDEBAR_WIDTH, height), egui::Sense::hover());

    let bg = egui::Color32::from_rgb(39, 39, 39);
    let selected = egui::Color32::from_rgb(66, 66, 66);
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 72, 72));
    let text = ui.visuals().text_color();
    let muted = egui::Color32::from_rgb(165, 165, 165);
    let blue = egui::Color32::from_rgb(28, 145, 220);

    ui.painter().rect_filled(rect, 0.0, bg);
    ui.painter()
        .line_segment([rect.right_top(), rect.right_bottom()], stroke);

    let mut y = rect.top();
    paint_header(ui, rect, y, text, stroke, repo_name);
    y += HEADER_HEIGHT;

    paint_nav_row(ui, rect, y, FILE_TEXT, "Changes", false, text, selected);
    y += ROW_HEIGHT;
    paint_nav_row(ui, rect, y, LIST, "All Commits", true, text, selected);
    y += ROW_HEIGHT;

    paint_mode_bar(ui, rect, y, blue, muted, stroke);
    y += 34.0;

    paint_filter(ui, rect, y, muted, stroke);
    y += FILTER_HEIGHT + 12.0;

    if let Some(repo) = git_repo {
        if let Ok(branches) = repo.branches() {
            let local: Vec<_> = branches.iter().filter(|b| !b.is_remote).collect();
            let remote: Vec<_> = branches.iter().filter(|b| b.is_remote).collect();

            if !local.is_empty() {
                paint_section(ui, rect, y, "Branches", text);
                y += ROW_HEIGHT;
                for branch in &local {
                    let icon = if branch.is_current { CHECK } else { FOLDER };
                    paint_tree_row(
                        ui,
                        rect,
                        y,
                        1,
                        icon,
                        &branch.name,
                        branch.is_current,
                        text,
                        muted,
                        None,
                    );
                    y += ROW_HEIGHT;
                }
            }

            if !remote.is_empty() {
                paint_section(ui, rect, y, "Remotes", text);
                y += ROW_HEIGHT;
                for branch in &remote {
                    paint_tree_row(
                        ui,
                        rect,
                        y,
                        1,
                        GITHUB_LOGO,
                        &branch.name,
                        false,
                        text,
                        muted,
                        None,
                    );
                    y += ROW_HEIGHT;
                }
            }
        }

        if let Ok(remotes) = repo.remotes() {
            if !remotes.is_empty() {
                y += 4.0;
            }
        }

        if let Ok(tags) = repo.tags() {
            if !tags.is_empty() {
                paint_collapsed_section(ui, rect, y, "Tags", text);
                y += ROW_HEIGHT;
            }
        }
    } else {
        for title in ["Branches", "Remotes", "Tags", "Stashes", "Submodules"] {
            paint_collapsed_section(ui, rect, y, title, text);
            y += ROW_HEIGHT;
        }
    }
}

#[allow(unused_assignments)]
pub fn show_cached(ui: &mut egui::Ui, repo_name: Option<&str>, app_state: &AppState) {
    let height = ui.available_height();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(SIDEBAR_WIDTH, height), egui::Sense::hover());

    let bg = egui::Color32::from_rgb(39, 39, 39);
    let selected = egui::Color32::from_rgb(66, 66, 66);
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 72, 72));
    let text = ui.visuals().text_color();
    let muted = egui::Color32::from_rgb(165, 165, 165);
    let blue = egui::Color32::from_rgb(28, 145, 220);

    ui.painter().rect_filled(rect, 0.0, bg);
    ui.painter()
        .line_segment([rect.right_top(), rect.right_bottom()], stroke);

    let mut y = rect.top();
    paint_header(ui, rect, y, text, stroke, repo_name);
    y += HEADER_HEIGHT;

    paint_nav_row(ui, rect, y, FILE_TEXT, "Changes", false, text, selected);
    y += ROW_HEIGHT;
    paint_nav_row(ui, rect, y, LIST, "All Commits", true, text, selected);
    y += ROW_HEIGHT;

    paint_mode_bar(ui, rect, y, blue, muted, stroke);
    y += 34.0;

    paint_filter(ui, rect, y, muted, stroke);
    y += FILTER_HEIGHT + 12.0;

    if app_state.current_repo.is_some() {
        let local: Vec<_> = app_state
            .cached_branches
            .iter()
            .filter(|b| !b.is_remote)
            .collect();
        let remote: Vec<_> = app_state
            .cached_branches
            .iter()
            .filter(|b| b.is_remote)
            .collect();

        if !local.is_empty() {
            paint_section(ui, rect, y, "Branches", text);
            y += ROW_HEIGHT;
            for branch in &local {
                let icon = if branch.is_current { CHECK } else { FOLDER };
                paint_tree_row(
                    ui,
                    rect,
                    y,
                    1,
                    icon,
                    &branch.name,
                    branch.is_current,
                    text,
                    muted,
                    None,
                );
                y += ROW_HEIGHT;
            }
        }

        if !remote.is_empty() {
            paint_section(ui, rect, y, "Remotes", text);
            y += ROW_HEIGHT;
            for branch in &remote {
                paint_tree_row(
                    ui,
                    rect,
                    y,
                    1,
                    GITHUB_LOGO,
                    &branch.name,
                    false,
                    text,
                    muted,
                    None,
                );
                y += ROW_HEIGHT;
            }
        }

        if !app_state.cached_remotes.is_empty() {
            y += 4.0;
        }

        if !app_state.cached_tags.is_empty() {
            paint_collapsed_section(ui, rect, y, "Tags", text);
            y += ROW_HEIGHT;
        }
    } else {
        for title in ["Branches", "Remotes", "Tags", "Stashes", "Submodules"] {
            paint_collapsed_section(ui, rect, y, title, text);
            y += ROW_HEIGHT;
        }
    }
}

fn paint_header(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    text: egui::Color32,
    stroke: egui::Stroke,
    repo_name: Option<&str>,
) {
    let row = row_rect(rect, y, HEADER_HEIGHT);
    ui.painter()
        .line_segment([row.left_bottom(), row.right_bottom()], stroke);
    let label = repo_name.unwrap_or("Open a repository");
    painter_text(
        ui,
        egui::pos2(row.left() + 18.0, row.center().y),
        label,
        15.0,
        text,
        egui::Align2::LEFT_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(row.right() - 16.0, row.center().y),
        GEAR_SIX,
        16.0,
        text,
        egui::Align2::CENTER_CENTER,
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_nav_row(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    icon: &str,
    label: &str,
    is_selected: bool,
    text: egui::Color32,
    selected: egui::Color32,
) {
    let row = row_rect(rect, y, ROW_HEIGHT);
    if is_selected {
        ui.painter().rect_filled(row, 0.0, selected);
    }
    painter_text(
        ui,
        egui::pos2(row.left() + 24.0, row.center().y),
        icon,
        16.0,
        text,
        egui::Align2::CENTER_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(row.left() + 48.0, row.center().y),
        label,
        14.0,
        text,
        egui::Align2::LEFT_CENTER,
    );
}

fn paint_mode_bar(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    active: egui::Color32,
    muted: egui::Color32,
    stroke: egui::Stroke,
) {
    let row = row_rect(rect, y, 34.0);
    ui.painter()
        .line_segment([row.left_top(), row.right_top()], stroke);
    ui.painter()
        .line_segment([row.left_bottom(), row.right_bottom()], stroke);
    let third = row.width() / 3.0;
    painter_text(
        ui,
        egui::pos2(row.left() + third * 0.5, row.center().y),
        GIT_BRANCH,
        18.0,
        active,
        egui::Align2::CENTER_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(row.left() + third * 1.5, row.center().y),
        MAGNIFYING_GLASS,
        18.0,
        muted,
        egui::Align2::CENTER_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(row.left() + third * 2.5, row.center().y),
        GITHUB_LOGO,
        18.0,
        muted,
        egui::Align2::CENTER_CENTER,
    );
}

fn paint_filter(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    muted: egui::Color32,
    stroke: egui::Stroke,
) {
    let filter = row_rect(rect, y, FILTER_HEIGHT).shrink2(egui::vec2(10.0, 2.0));
    ui.painter()
        .rect_stroke(filter, 0.0, stroke, egui::StrokeKind::Inside);
    painter_text(
        ui,
        egui::pos2(filter.left() + 16.0, filter.center().y),
        MAGNIFYING_GLASS,
        14.0,
        muted,
        egui::Align2::CENTER_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(filter.left() + 32.0, filter.center().y),
        "Filter",
        13.0,
        muted,
        egui::Align2::LEFT_CENTER,
    );
}

fn paint_section(ui: &egui::Ui, rect: egui::Rect, y: f32, label: &str, text: egui::Color32) {
    let row = row_rect(rect, y, ROW_HEIGHT);
    painter_text(
        ui,
        egui::pos2(row.left() + 14.0, row.center().y),
        CARET_DOWN,
        12.0,
        text,
        egui::Align2::CENTER_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(row.left() + 28.0, row.center().y),
        label,
        15.0,
        text,
        egui::Align2::LEFT_CENTER,
    );
}

fn paint_collapsed_section(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    label: &str,
    text: egui::Color32,
) {
    let row = row_rect(rect, y, ROW_HEIGHT);
    painter_text(
        ui,
        egui::pos2(row.left() + 14.0, row.center().y),
        CARET_RIGHT,
        12.0,
        text,
        egui::Align2::CENTER_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(row.left() + 28.0, row.center().y),
        label,
        15.0,
        text,
        egui::Align2::LEFT_CENTER,
    );
}

#[allow(clippy::too_many_arguments)]
fn paint_tree_row(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    indent: usize,
    icon: &str,
    label: &str,
    strong: bool,
    text: egui::Color32,
    muted: egui::Color32,
    trailing: Option<(&str, egui::Color32)>,
) {
    let row = row_rect(rect, y, ROW_HEIGHT);
    let left = row.left() + 18.0 + indent as f32 * 16.0;
    painter_text(
        ui,
        egui::pos2(left - 9.0, row.center().y),
        CARET_RIGHT,
        10.0,
        muted,
        egui::Align2::CENTER_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(left + 8.0, row.center().y),
        icon,
        16.0,
        if strong { text } else { muted },
        egui::Align2::CENTER_CENTER,
    );
    painter_text(
        ui,
        egui::pos2(left + 28.0, row.center().y),
        label,
        if strong { 14.5 } else { 14.0 },
        text,
        egui::Align2::LEFT_CENTER,
    );

    if let Some((trailing_icon, color)) = trailing {
        painter_text(
            ui,
            egui::pos2(row.right() - 54.0, row.center().y),
            trailing_icon,
            16.0,
            color,
            egui::Align2::CENTER_CENTER,
        );
        painter_text(
            ui,
            egui::pos2(row.right() - 34.0, row.center().y),
            FUNNEL,
            15.0,
            muted,
            egui::Align2::CENTER_CENTER,
        );
        painter_text(
            ui,
            egui::pos2(row.right() - 15.0, row.center().y),
            EYE,
            15.0,
            muted,
            egui::Align2::CENTER_CENTER,
        );
    }
}

fn row_rect(rect: egui::Rect, y: f32, height: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(rect.width(), height))
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
