use eframe::egui;
use egui_phosphor::regular::{BOOKMARK, GIT_BRANCH, GIT_COMMIT, GITHUB_LOGO};

use crate::state::{AppState, ManagerRepoDetails};

pub struct State {
    active_tab: ManagerTab,
}

#[derive(Clone, PartialEq)]
enum ManagerTab {
    Summary,
    Statistics,
}

impl Default for State {
    fn default() -> Self {
        Self {
            active_tab: ManagerTab::Summary,
        }
    }
}

const ROW_HEIGHT: f32 = 28.0;
const SECTION_GAP: f32 = 12.0;

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
    y += 44.0;
    y += SECTION_GAP;

    y = paint_stats_panel(ui, rect, y, details);
    y += SECTION_GAP;

    y = paint_tab_bar(ui, rect, y, &mut state.active_tab);
    y += SECTION_GAP;

    if state.active_tab == ManagerTab::Summary {
        paint_summary(ui, rect, y, details);
    }

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
    let row_height = 44.0;
    let row = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 16.0, y),
        egui::vec2(rect.width() - 32.0, row_height),
    );

    ui.painter().text(
        egui::pos2(row.left(), row.center().y),
        egui::Align2::LEFT_CENTER,
        &details.repo_name,
        egui::FontId::proportional(15.0),
        ui.visuals().text_color(),
    );

    ui.painter().text(
        egui::pos2(row.left(), row.center().y + 16.0),
        egui::Align2::LEFT_CENTER,
        &details.repo_path,
        egui::FontId::proportional(11.0),
        egui::Color32::from_rgb(140, 140, 140),
    );

    let open_btn_rect = egui::Rect::from_min_size(
        egui::pos2(row.right() - 70.0, row.top() + 5.0),
        egui::vec2(60.0, 28.0),
    );
    ui.painter()
        .rect_filled(open_btn_rect, 4.0, egui::Color32::from_rgb(76, 167, 255));
    ui.painter().text(
        open_btn_rect.center(),
        egui::Align2::CENTER_CENTER,
        "Open",
        egui::FontId::proportional(12.0),
        egui::Color32::WHITE,
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
            let link_x = right_x + 16.0 + remote.name.len() as f32 * 6.0 + 8.0;
            ui.painter().text(
                egui::pos2(link_x, ry),
                egui::Align2::LEFT_CENTER,
                "GitHub",
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(76, 167, 255),
            );
            ui.painter().text(
                egui::pos2(link_x + 42.0, ry),
                egui::Align2::LEFT_CENTER,
                "Issues",
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(76, 167, 255),
            );
            ui.painter().text(
                egui::pos2(link_x + 72.0, ry),
                egui::Align2::LEFT_CENTER,
                "Pull Requests",
                egui::FontId::proportional(11.0),
                egui::Color32::from_rgb(76, 167, 255),
            );
        }
    }

    panel.bottom()
}

fn paint_tab_bar(ui: &egui::Ui, rect: egui::Rect, y: f32, active_tab: &mut ManagerTab) -> f32 {
    let tab_height = 28.0;
    let tab_x = rect.left() + 16.0;

    let summary_rect =
        egui::Rect::from_min_size(egui::pos2(tab_x, y), egui::vec2(70.0, tab_height));
    let stats_rect =
        egui::Rect::from_min_size(egui::pos2(tab_x + 74.0, y), egui::vec2(80.0, tab_height));

    let is_summary = *active_tab == ManagerTab::Summary;
    let summary_bg = if is_summary {
        egui::Color32::from_rgb(62, 62, 62)
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(summary_rect, 4.0, summary_bg);
    ui.painter().text(
        summary_rect.center(),
        egui::Align2::CENTER_CENTER,
        "Summary",
        egui::FontId::proportional(12.0),
        if is_summary {
            ui.visuals().text_color()
        } else {
            egui::Color32::from_rgb(140, 140, 140)
        },
    );

    let summary_resp = ui.interact(
        summary_rect,
        ui.make_persistent_id("manager_tab_summary"),
        egui::Sense::click(),
    );
    if summary_resp.clicked() {
        *active_tab = ManagerTab::Summary;
    }

    let is_stats = *active_tab == ManagerTab::Statistics;
    let stats_bg = if is_stats {
        egui::Color32::from_rgb(62, 62, 62)
    } else {
        egui::Color32::TRANSPARENT
    };
    ui.painter().rect_filled(stats_rect, 4.0, stats_bg);
    ui.painter().text(
        stats_rect.center(),
        egui::Align2::CENTER_CENTER,
        "Statistics",
        egui::FontId::proportional(12.0),
        if is_stats {
            ui.visuals().text_color()
        } else {
            egui::Color32::from_rgb(140, 140, 140)
        },
    );

    let stats_resp = ui.interact(
        stats_rect,
        ui.make_persistent_id("manager_tab_stats"),
        egui::Sense::click(),
    );
    if stats_resp.clicked() {
        *active_tab = ManagerTab::Statistics;
    }

    y + tab_height
}

fn paint_summary(ui: &mut egui::Ui, rect: egui::Rect, y: f32, details: &ManagerRepoDetails) {
    let content_rect = egui::Rect::from_min_max(egui::pos2(rect.left(), y), rect.right_bottom());

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("manager_summary_scroll")
            .max_rect(content_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let mut y = 0.0;

                    y = paint_section(ui, rect, y, "Branches", GIT_BRANCH);
                    for branch in &details.branches {
                        y = paint_branch_row(ui, rect, y, branch);
                    }

                    y += SECTION_GAP;
                    y = paint_section(ui, rect, y, "Tags", BOOKMARK);
                    for tag in &details.tags {
                        y = paint_tag_row(ui, rect, y, tag);
                    }

                    y += SECTION_GAP;
                    y = paint_section(ui, rect, y, "Commits", GIT_COMMIT);
                    for commit in &details.commits {
                        y = paint_commit_row(ui, rect, y, commit);
                    }

                    let content_size = egui::vec2(rect.width(), y);
                    let (_, _) = ui.allocate_exact_size(content_size, egui::Sense::hover());
                });
        },
    );
}

fn paint_section(ui: &egui::Ui, rect: egui::Rect, y: f32, label: &str, icon: &str) -> f32 {
    let row = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 16.0, y),
        egui::vec2(rect.width() - 32.0, ROW_HEIGHT),
    );

    let card_bg = egui::Color32::from_rgb(37, 37, 37);
    ui.painter().rect_filled(row, 4.0, card_bg);

    ui.painter().text(
        egui::pos2(row.left() + 12.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        icon,
        egui::FontId::proportional(14.0),
        egui::Color32::from_rgb(140, 140, 140),
    );
    ui.painter().text(
        egui::pos2(row.left() + 30.0, row.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        ui.visuals().text_color(),
    );

    row.bottom()
}

fn paint_branch_row(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    branch: &crate::state::ManagerBranch,
) -> f32 {
    let row = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 16.0, y),
        egui::vec2(rect.width() - 32.0, ROW_HEIGHT),
    );

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

fn paint_tag_row(ui: &egui::Ui, rect: egui::Rect, y: f32, tag: &crate::state::ManagerTag) -> f32 {
    let row = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 16.0, y),
        egui::vec2(rect.width() - 32.0, ROW_HEIGHT),
    );

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

fn paint_commit_row(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    commit: &crate::state::ManagerCommit,
) -> f32 {
    let row = egui::Rect::from_min_size(
        egui::pos2(rect.left() + 16.0, y),
        egui::vec2(rect.width() - 32.0, ROW_HEIGHT),
    );

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
