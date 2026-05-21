use eframe::egui;
use egui_phosphor::regular::{CARET_DOWN, CARET_RIGHT, FOLDER};

use crate::state::AppState;
use crate::ui::repo_manager::format_relative_time;

pub const SIDEBAR_WIDTH: f32 = 236.0;
const HEADER_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 24.0;

pub struct SidebarState {
    pub recents_expanded: bool,
    pub repos_expanded: bool,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            recents_expanded: true,
            repos_expanded: false,
        }
    }
}

pub enum ManagerSidebarAction {
    SelectRepo(String),
}

pub fn show(
    ui: &mut egui::Ui,
    sidebar_state: &mut SidebarState,
    app_state: &AppState,
) -> Option<ManagerSidebarAction> {
    let height = ui.available_height();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(SIDEBAR_WIDTH, height), egui::Sense::hover());

    let bg = egui::Color32::from_rgb(39, 39, 39);
    let selected = egui::Color32::from_rgb(66, 66, 66);
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 72, 72));
    let text = ui.visuals().text_color();
    let muted = egui::Color32::from_rgb(140, 140, 140);

    ui.painter().rect_filled(rect, 0.0, bg);
    ui.painter()
        .line_segment([rect.right_top(), rect.right_bottom()], stroke);

    let mut y = rect.top();

    paint_title(ui, rect, y, text, stroke);
    y += HEADER_HEIGHT;

    let mut action = None;

    y = paint_section_header(
        ui,
        rect,
        y,
        "Recent",
        &mut sidebar_state.recents_expanded,
        text,
    );
    if sidebar_state.recents_expanded {
        for repo in &app_state.recent_repos {
            let name = repo_name(&repo.path);
            let is_selected = app_state.manager_selected_repo.as_deref() == Some(&repo.path);
            let time_ago = format_relative_time(repo.last_opened as i64);
            let clicked = paint_repo_row_with_time(
                ui,
                rect,
                y,
                name,
                &time_ago,
                is_selected,
                text,
                selected,
                muted,
            );
            if clicked {
                action = Some(ManagerSidebarAction::SelectRepo(repo.path.clone()));
            }
            y += ROW_HEIGHT;
        }
    }

    y += 8.0;
    y = paint_section_header(
        ui,
        rect,
        y,
        "Repositories",
        &mut sidebar_state.repos_expanded,
        text,
    );
    if sidebar_state.repos_expanded {
        for repo in &app_state.recent_repos {
            let name = repo_name(&repo.path);
            let is_selected = app_state.manager_selected_repo.as_deref() == Some(&repo.path);
            let clicked = paint_repo_row(ui, rect, y, name, is_selected, text, selected, muted);
            if clicked {
                action = Some(ManagerSidebarAction::SelectRepo(repo.path.clone()));
            }
            y += ROW_HEIGHT;
        }
    }

    action
}

fn paint_title(ui: &egui::Ui, rect: egui::Rect, y: f32, text: egui::Color32, stroke: egui::Stroke) {
    let row = row_rect(rect, y, HEADER_HEIGHT);
    ui.painter()
        .line_segment([row.left_bottom(), row.right_bottom()], stroke);
    ui.painter().text(
        egui::pos2(row.left() + 12.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        "Repository Manager",
        egui::FontId::proportional(15.0),
        text,
    );
}

fn paint_section_header(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    label: &str,
    expanded: &mut bool,
    text: egui::Color32,
) -> f32 {
    let row = row_rect(rect, y, ROW_HEIGHT);
    let caret = if *expanded { CARET_DOWN } else { CARET_RIGHT };

    let response = ui.interact(
        row,
        ui.make_persistent_id(("manager_section", label)),
        egui::Sense::click(),
    );
    if response.clicked() {
        *expanded = !*expanded;
    }

    ui.painter().text(
        egui::pos2(row.left() + 10.0, row.center().y),
        egui::Align2::CENTER_CENTER,
        caret,
        egui::FontId::proportional(12.0),
        text,
    );
    ui.painter().text(
        egui::pos2(row.left() + 24.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        text,
    );
    row.bottom()
}

#[allow(clippy::too_many_arguments)]
fn paint_repo_row(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    name: &str,
    is_selected: bool,
    text: egui::Color32,
    selected: egui::Color32,
    muted: egui::Color32,
) -> bool {
    let row = row_rect(rect, y, ROW_HEIGHT);
    if is_selected {
        ui.painter().rect_filled(row, 0.0, selected);
    }

    let response = ui.interact(
        row,
        ui.make_persistent_id(("manager_repo", name)),
        egui::Sense::click(),
    );

    ui.painter().text(
        egui::pos2(row.left() + 24.0, row.center().y),
        egui::Align2::CENTER_CENTER,
        FOLDER,
        egui::FontId::proportional(14.0),
        if is_selected { text } else { muted },
    );
    ui.painter().text(
        egui::pos2(row.left() + 44.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(13.0),
        text,
    );

    response.clicked()
}

#[allow(clippy::too_many_arguments)]
fn paint_repo_row_with_time(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    name: &str,
    time_ago: &str,
    is_selected: bool,
    text: egui::Color32,
    selected: egui::Color32,
    muted: egui::Color32,
) -> bool {
    let row = row_rect(rect, y, ROW_HEIGHT);
    if is_selected {
        ui.painter().rect_filled(row, 0.0, selected);
    }

    let response = ui.interact(
        row,
        ui.make_persistent_id(("manager_repo_recent", name)),
        egui::Sense::click(),
    );

    ui.painter().text(
        egui::pos2(row.left() + 24.0, row.center().y),
        egui::Align2::CENTER_CENTER,
        FOLDER,
        egui::FontId::proportional(14.0),
        if is_selected { text } else { muted },
    );
    ui.painter().text(
        egui::pos2(row.left() + 44.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        name,
        egui::FontId::proportional(13.0),
        text,
    );
    ui.painter().text(
        egui::pos2(row.right() - 12.0, row.center().y),
        egui::Align2::RIGHT_CENTER,
        time_ago,
        egui::FontId::proportional(11.0),
        muted,
    );

    response.clicked()
}

fn repo_name(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}

fn row_rect(rect: egui::Rect, y: f32, height: f32) -> egui::Rect {
    egui::Rect::from_min_size(egui::pos2(rect.left(), y), egui::vec2(rect.width(), height))
}
