use eframe::egui;
use egui_phosphor::regular::{
    ARROW_SQUARE_OUT, BOOKMARK, CARET_DOWN, CARET_RIGHT, FOLDER, GIT_BRANCH, GIT_COMMIT,
    GITHUB_LOGO,
};

use crate::state::{AppState, ManagerRepoDetails};

pub struct State {
    branches_expanded: bool,
    tags_expanded: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            branches_expanded: true,
            tags_expanded: true,
        }
    }
}

const ROW_HEIGHT: f32 = 28.0;
const SECTION_GAP: f32 = 12.0;
const SECTION_HEADER_HEIGHT: f32 = 32.0;

pub fn show(ui: &mut egui::Ui, state: &mut State, app_state: &AppState) -> Option<String> {
    let rect = ui.available_rect_before_wrap();
    let (rect, _) = ui.allocate_exact_size(rect.size(), egui::Sense::hover());

    let bg = egui::Color32::from_rgb(31, 31, 31);
    ui.painter().rect_filled(rect, 0.0, bg);

    let Some(details) = &app_state.manager_details else {
        paint_no_selection(ui, rect);
        return None;
    };

    let mut y = rect.top();

    if let Some(open_path) = paint_top_bar(ui, rect, y, details) {
        return Some(open_path);
    }
    y += 56.0;
    y += SECTION_GAP;

    y = paint_stats_panel(ui, rect, y, details);
    y += SECTION_GAP;

    y = paint_collapsible_section(
        ui,
        rect,
        y,
        "Branches",
        GIT_BRANCH,
        &mut state.branches_expanded,
        details.branches.len(),
        |ui, rect, y| paint_branches_content(ui, rect, y, details),
    );
    y += SECTION_GAP;

    if !details.tags.is_empty() {
        y = paint_collapsible_section(
            ui,
            rect,
            y,
            "Tags",
            BOOKMARK,
            &mut state.tags_expanded,
            details.tags.len(),
            |ui, rect, y| paint_tags_content(ui, rect, y, details),
        );
        y += SECTION_GAP;
    }

    paint_commits_section(ui, rect, y, details);

    None
}

fn paint_no_selection(ui: &egui::Ui, rect: egui::Rect) {
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        "Select a repository to view details",
        egui::FontId::proportional(13.0),
        egui::Color32::from_rgb(140, 140, 140),
    );
}

fn paint_top_bar(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    details: &ManagerRepoDetails,
) -> Option<String> {
    let row_height = 56.0;
    let row = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 16.0, y),
        egui::vec2(rect.width() - 32.0, row_height),
    );

    ui.painter().text(
        egui::pos2(row.left(), row.top() + 16.0),
        egui::Align2::LEFT_TOP,
        &details.repo_name,
        egui::FontId::proportional(18.0),
        ui.visuals().text_color(),
    );

    let path_y = row.top() + 36.0;
    ui.painter().text(
        egui::pos2(row.left(), path_y),
        egui::Align2::LEFT_CENTER,
        &details.repo_path,
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(140, 140, 140),
    );

    let explorer_btn_rect = egui::Rect::from_min_size(
        egui::pos2(
            row.left() + details.repo_path.len() as f32 * 5.5 + 8.0,
            path_y - 8.0,
        ),
        egui::vec2(20.0, 16.0),
    );
    ui.painter().text(
        explorer_btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        FOLDER,
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgb(140, 140, 140),
    );
    let explorer_resp = ui.interact(
        explorer_btn_rect,
        ui.make_persistent_id("manager_explorer_btn"),
        egui::Sense::click(),
    );
    if explorer_resp.clicked() {
        let _ = std::process::Command::new(if cfg!(target_os = "windows") {
            "explorer"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        })
        .arg(&details.repo_path)
        .spawn();
    }

    let open_btn_rect = egui::Rect::from_min_size(
        egui::pos2(row.right() - 80.0, row.top() + 14.0),
        egui::vec2(70.0, 28.0),
    );
    ui.painter()
        .rect_filled(open_btn_rect, 4.0, egui::Color32::from_rgb(62, 62, 62));
    ui.painter().text(
        egui::pos2(open_btn_rect.left() + 14.0, open_btn_rect.center().y),
        egui::Align2::CENTER_CENTER,
        ARROW_SQUARE_OUT,
        egui::FontId::proportional(12.0),
        ui.visuals().text_color(),
    );
    ui.painter().text(
        egui::pos2(open_btn_rect.left() + 32.0, open_btn_rect.center().y),
        egui::Align2::CENTER_CENTER,
        "Open",
        egui::FontId::proportional(12.0),
        ui.visuals().text_color(),
    );

    let response = ui.interact(
        open_btn_rect,
        ui.make_persistent_id("manager_open_btn"),
        egui::Sense::click(),
    );
    if response.clicked() {
        return Some(details.repo_path.clone());
    }

    None
}

fn paint_stats_panel(ui: &egui::Ui, rect: egui::Rect, y: f32, details: &ManagerRepoDetails) -> f32 {
    let panel_height = 80.0;
    let panel = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 16.0, y),
        egui::vec2(rect.width() - 32.0, panel_height),
    );

    let card_bg = egui::Color32::from_rgb(37, 37, 37);
    ui.painter().rect_filled(panel, 6.0, card_bg);

    let left_x = panel.left() + 16.0;
    let right_x = panel.left() + panel.width() * 0.5;
    let stat_y = panel.top() + 14.0;
    let stat_gap = 18.0;

    let muted = egui::Color32::from_rgb(140, 140, 140);
    let text = ui.visuals().text_color();

    ui.painter().text(
        egui::pos2(left_x, stat_y),
        egui::Align2::LEFT_CENTER,
        format!("Uncommitted Files: {}", details.uncommitted_files),
        egui::FontId::proportional(12.0),
        text,
    );
    ui.painter().text(
        egui::pos2(left_x, stat_y + stat_gap),
        egui::Align2::LEFT_CENTER,
        format!("Commits: {}", details.total_commits),
        egui::FontId::proportional(12.0),
        text,
    );
    ui.painter().text(
        egui::pos2(left_x, stat_y + stat_gap * 2.0),
        egui::Align2::LEFT_CENTER,
        format!("Initial Commit: {}", details.initial_commit_date),
        egui::FontId::proportional(12.0),
        text,
    );
    ui.painter().text(
        egui::pos2(left_x, stat_y + stat_gap * 3.0),
        egui::Align2::LEFT_CENTER,
        format!("Last Commit: {}", details.last_commit_date),
        egui::FontId::proportional(12.0),
        text,
    );

    let remote_y = panel.top() + 14.0;
    ui.painter().text(
        egui::pos2(right_x, remote_y),
        egui::Align2::LEFT_CENTER,
        "Remotes:",
        egui::FontId::proportional(12.0),
        text,
    );

    for (i, remote) in details.remotes.iter().enumerate() {
        let ry = remote_y + 18.0 + i as f32 * 16.0;
        let icon = if remote.is_github {
            GITHUB_LOGO
        } else {
            GIT_BRANCH
        };
        ui.painter().text(
            egui::pos2(right_x, ry),
            egui::Align2::LEFT_CENTER,
            icon,
            egui::FontId::proportional(12.0),
            muted,
        );
        ui.painter().text(
            egui::pos2(right_x + 16.0, ry),
            egui::Align2::LEFT_CENTER,
            &remote.name,
            egui::FontId::proportional(12.0),
            text,
        );

        if remote.is_github {
            let mut link_x = right_x + 16.0 + remote.name.len() as f32 * 6.0 + 12.0;
            let link_color = egui::Color32::from_rgb(160, 160, 160);
            let link_size = 11.0;
            let gap = 14.0;

            for label in ["GitHub", "Issues", "Pull Requests", "Actions"] {
                let text_width = ui
                    .painter()
                    .layout_no_wrap(
                        label.to_string(),
                        egui::FontId::proportional(link_size),
                        link_color,
                    )
                    .rect
                    .width();
                ui.painter().text(
                    egui::pos2(link_x, ry),
                    egui::Align2::LEFT_CENTER,
                    label,
                    egui::FontId::proportional(link_size),
                    link_color,
                );
                link_x += text_width + gap;
            }
        }
    }

    panel.bottom()
}

#[allow(clippy::too_many_arguments)]
fn paint_collapsible_section(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    label: &str,
    icon: &str,
    expanded: &mut bool,
    item_count: usize,
    content_fn: impl FnOnce(&egui::Ui, egui::Rect, f32) -> f32,
) -> f32 {
    let header_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 16.0, y),
        egui::vec2(rect.width() - 32.0, SECTION_HEADER_HEIGHT),
    );

    let card_bg = egui::Color32::from_rgb(37, 37, 37);

    let response = ui.interact(
        header_rect,
        ui.make_persistent_id(("manager_section", label)),
        egui::Sense::click(),
    );
    if response.clicked() {
        *expanded = !*expanded;
    }

    if *expanded {
        let content_height = (item_count.max(1) as f32) * ROW_HEIGHT;
        let content_start_y = header_rect.bottom();
        let full_card = egui::Rect::from_min_max(
            header_rect.left_top(),
            egui::pos2(header_rect.right(), content_start_y + content_height),
        );
        ui.painter().rect_filled(full_card, 6.0, card_bg);
    } else {
        ui.painter().rect_filled(header_rect, 6.0, card_bg);
    }

    let caret = if *expanded { CARET_DOWN } else { CARET_RIGHT };
    ui.painter().text(
        egui::pos2(header_rect.left() + 10.0, header_rect.center().y),
        egui::Align2::CENTER_CENTER,
        caret,
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgb(140, 140, 140),
    );
    ui.painter().text(
        egui::pos2(header_rect.left() + 26.0, header_rect.center().y),
        egui::Align2::LEFT_CENTER,
        icon,
        egui::FontId::proportional(14.0),
        egui::Color32::from_rgb(140, 140, 140),
    );
    ui.painter().text(
        egui::pos2(header_rect.left() + 44.0, header_rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        ui.visuals().text_color(),
    );

    if !*expanded {
        return header_rect.bottom();
    }

    let content_start_y = header_rect.bottom();
    let content_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 16.0, content_start_y),
        egui::vec2(rect.width() - 32.0, 1.0),
    );

    content_fn(ui, content_rect, content_start_y)
}

fn paint_commits_section(ui: &egui::Ui, rect: egui::Rect, y: f32, details: &ManagerRepoDetails) {
    let header_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 16.0, y),
        egui::vec2(rect.width() - 32.0, SECTION_HEADER_HEIGHT),
    );

    let card_bg = egui::Color32::from_rgb(37, 37, 37);
    let content_height = (details.commits.len().max(1) as f32) * ROW_HEIGHT;
    let content_start_y = header_rect.bottom();

    let full_card = egui::Rect::from_min_max(
        header_rect.left_top(),
        egui::pos2(header_rect.right(), content_start_y + content_height),
    );
    ui.painter().rect_filled(full_card, 6.0, card_bg);

    ui.painter().text(
        egui::pos2(header_rect.left() + 10.0, header_rect.center().y),
        egui::Align2::LEFT_CENTER,
        GIT_COMMIT,
        egui::FontId::proportional(14.0),
        egui::Color32::from_rgb(140, 140, 140),
    );
    ui.painter().text(
        egui::pos2(header_rect.left() + 28.0, header_rect.center().y),
        egui::Align2::LEFT_CENTER,
        "Commits",
        egui::FontId::proportional(13.0),
        ui.visuals().text_color(),
    );

    let content_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 16.0, content_start_y),
        egui::vec2(rect.width() - 32.0, content_height),
    );

    let mut current_y = content_start_y;
    for commit in &details.commits {
        let row = egui::Rect::from_min_size(
            egui::pos2(content_rect.left(), current_y),
            egui::vec2(content_rect.width(), ROW_HEIGHT),
        );
        current_y = paint_commit_row(ui, row, commit);
    }
}

fn paint_branches_content(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    details: &ManagerRepoDetails,
) -> f32 {
    let mut current_y = y;
    for branch in &details.branches {
        let row = egui::Rect::from_min_size(
            egui::pos2(rect.left(), current_y),
            egui::vec2(rect.width(), ROW_HEIGHT),
        );
        current_y = paint_branch_row(ui, row, branch);
    }
    if details.branches.is_empty() {
        current_y += ROW_HEIGHT;
    }
    current_y
}

fn paint_tags_content(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    details: &ManagerRepoDetails,
) -> f32 {
    let mut current_y = y;
    for tag in &details.tags {
        let row = egui::Rect::from_min_size(
            egui::pos2(rect.left(), current_y),
            egui::vec2(rect.width(), ROW_HEIGHT),
        );
        current_y = paint_tag_row(ui, row, tag);
    }
    if details.tags.is_empty() {
        current_y += ROW_HEIGHT;
    }
    current_y
}

fn paint_branch_row(ui: &egui::Ui, row: egui::Rect, branch: &crate::state::ManagerBranch) -> f32 {
    let muted = egui::Color32::from_rgb(140, 140, 140);
    let text = ui.visuals().text_color();

    ui.painter().text(
        egui::pos2(row.left() + 12.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        GIT_BRANCH,
        egui::FontId::proportional(12.0),
        muted,
    );
    ui.painter().text(
        egui::pos2(row.left() + 28.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        &branch.name,
        egui::FontId::proportional(12.0),
        text,
    );

    let msg_x = row.left() + 28.0 + branch.name.len() as f32 * 6.0 + 12.0;
    let max_msg_width = (row.width() - (msg_x - row.left()) - 200.0).max(50.0);
    let display_msg = truncate(&branch.last_message, max_msg_width);
    ui.painter().text(
        egui::pos2(msg_x, row.center().y),
        egui::Align2::LEFT_CENTER,
        &display_msg,
        egui::FontId::proportional(11.0),
        muted,
    );

    let author_x = row.right() - 180.0;
    paint_avatar(ui, egui::pos2(author_x, row.center().y), &branch.author);
    ui.painter().text(
        egui::pos2(author_x + 22.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        &branch.author,
        egui::FontId::proportional(11.0),
        text,
    );

    ui.painter().text(
        egui::pos2(row.right() - 12.0, row.center().y),
        egui::Align2::RIGHT_CENTER,
        &branch.relative_date,
        egui::FontId::proportional(11.0),
        muted,
    );

    row.bottom()
}

fn paint_tag_row(ui: &egui::Ui, row: egui::Rect, tag: &crate::state::ManagerTag) -> f32 {
    let muted = egui::Color32::from_rgb(140, 140, 140);
    let text = ui.visuals().text_color();

    ui.painter().text(
        egui::pos2(row.left() + 12.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        BOOKMARK,
        egui::FontId::proportional(12.0),
        muted,
    );
    ui.painter().text(
        egui::pos2(row.left() + 28.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        &tag.name,
        egui::FontId::proportional(12.0),
        text,
    );

    let author_x = row.right() - 180.0;
    paint_avatar(ui, egui::pos2(author_x, row.center().y), &tag.author);
    ui.painter().text(
        egui::pos2(author_x + 22.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        &tag.author,
        egui::FontId::proportional(11.0),
        text,
    );

    ui.painter().text(
        egui::pos2(row.right() - 12.0, row.center().y),
        egui::Align2::RIGHT_CENTER,
        &tag.relative_date,
        egui::FontId::proportional(11.0),
        muted,
    );

    row.bottom()
}

fn paint_commit_row(ui: &egui::Ui, row: egui::Rect, commit: &crate::state::ManagerCommit) -> f32 {
    let muted = egui::Color32::from_rgb(140, 140, 140);
    let text = ui.visuals().text_color();

    ui.painter().text(
        egui::pos2(row.left() + 12.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        GIT_COMMIT,
        egui::FontId::proportional(12.0),
        muted,
    );

    let max_msg_width = row.width() - 200.0;
    let display_msg = truncate(&commit.message, max_msg_width);
    ui.painter().text(
        egui::pos2(row.left() + 28.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        &display_msg,
        egui::FontId::proportional(12.0),
        text,
    );

    let author_x = row.right() - 180.0;
    paint_avatar(ui, egui::pos2(author_x, row.center().y), &commit.author);
    ui.painter().text(
        egui::pos2(author_x + 22.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        &commit.author,
        egui::FontId::proportional(11.0),
        text,
    );

    ui.painter().text(
        egui::pos2(row.right() - 12.0, row.center().y),
        egui::Align2::RIGHT_CENTER,
        &commit.relative_date,
        egui::FontId::proportional(11.0),
        muted,
    );

    row.bottom()
}

fn paint_avatar(ui: &egui::Ui, center: egui::Pos2, name: &str) {
    let rect = egui::Rect::from_center_size(center, egui::vec2(16.0, 16.0));
    let color = avatar_color(name);
    ui.painter().rect_filled(rect, 2.0, color);

    let initials: String = name
        .split_whitespace()
        .take(2)
        .map(|w| {
            w.chars()
                .next()
                .unwrap_or('?')
                .to_uppercase()
                .next()
                .unwrap()
        })
        .collect();

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        &initials,
        egui::FontId::proportional(7.0),
        egui::Color32::WHITE,
    );
}

fn avatar_color(name: &str) -> egui::Color32 {
    let colors = [
        egui::Color32::from_rgb(255, 165, 16),
        egui::Color32::from_rgb(238, 202, 34),
        egui::Color32::from_rgb(255, 45, 72),
        egui::Color32::from_rgb(151, 113, 73),
        egui::Color32::from_rgb(42, 167, 222),
        egui::Color32::from_rgb(56, 193, 114),
    ];
    let hash: u32 = name.bytes().map(|b| b as u32).sum();
    colors[(hash as usize) % colors.len()]
}

fn truncate(s: &str, max_width: f32) -> String {
    let char_width = 5.5;
    let max_chars = (max_width / char_width) as usize;
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        let keep = max_chars.saturating_sub(3);
        let truncated: String = s.chars().take(keep).collect();
        format!("{}...", truncated)
    }
}
