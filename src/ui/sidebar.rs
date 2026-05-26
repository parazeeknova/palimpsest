use eframe::egui;
use egui_phosphor::regular::{
    BOOKMARK, CARET_DOWN, CARET_RIGHT, CHECK, CHECK_CIRCLE, EYE, FILE_TEXT, FOLDER, FUNNEL,
    GEAR_SIX, GIT_BRANCH, GIT_PULL_REQUEST, GITHUB_LOGO, LIST, MAGNIFYING_GLASS, PACKAGE,
    PLAY_CIRCLE, STACK, TREE_VIEW, WARNING_CIRCLE,
};

use crate::state::AppState;

pub const SIDEBAR_WIDTH: f32 = 236.0;
const HEADER_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 24.0;
const FILTER_HEIGHT: f32 = 26.0;
use crate::ui::colors::get_branch_color;

pub struct SidebarState {
    pub branches_expanded: bool,
    pub remotes_expanded: bool,
    pub tags_expanded: bool,
    pub stashes_expanded: bool,
    pub prs_expanded: bool,
    pub runs_expanded: bool,
    pub releases_expanded: bool,
    pub packages_expanded: bool,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            branches_expanded: true,
            remotes_expanded: false,
            tags_expanded: false,
            stashes_expanded: false,
            prs_expanded: false,
            runs_expanded: false,
            releases_expanded: false,
            packages_expanded: false,
        }
    }
}

pub enum SidebarAction {
    CheckoutBranch(String),
    DeleteBranch(String),
    StashApply(usize),
    StashPop(usize),
    StashDrop(usize),
    OpenUrl(String),
}

#[allow(unused_assignments)]
pub fn show_cached(
    ui: &mut egui::Ui,
    sidebar_state: &mut SidebarState,
    repo_name: Option<&str>,
    app_state: &AppState,
) -> Option<SidebarAction> {
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

    if app_state.current_repo.is_none() {
        let logo = egui::Image::new(egui::include_image!("../assets/logo.svg"))
            .tint(egui::Color32::from_white_alpha(25))
            .fit_to_exact_size(egui::vec2(120.0, 120.0));
        let logo_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(120.0, 120.0));
        ui.put(logo_rect, logo);
        return None;
    }

    let mut y = rect.top();
    paint_header(ui, rect, y, text, stroke, repo_name);
    y += HEADER_HEIGHT;

    paint_nav_row(ui, rect, y, FILE_TEXT, "Changes", false, text, selected);
    y += ROW_HEIGHT;
    paint_nav_row(ui, rect, y, LIST, "All Commits", true, text, selected);
    y += ROW_HEIGHT;

    paint_mode_bar(ui, rect, y, blue, muted, stroke);
    y += 34.0;

    y += 8.0;
    paint_filter(ui, rect, y, muted, stroke);
    y += FILTER_HEIGHT + 12.0;

    let local: Vec<_> = app_state
        .cached_branches
        .iter()
        .filter(|b| !b.is_remote)
        .collect();
    let remote: Vec<_> = app_state
        .cached_branches
        .iter()
        .filter(|b| b.is_remote && !b.name.ends_with("/HEAD"))
        .collect();

    let mut action = None;

    let section_height = |expanded: bool, count: usize| {
        ROW_HEIGHT
            + if expanded {
                count as f32 * ROW_HEIGHT
            } else {
                0.0
            }
    };

    let content_height = section_height(
        sidebar_state.branches_expanded && !local.is_empty(),
        local.len(),
    ) + section_height(
        sidebar_state.remotes_expanded && !remote.is_empty(),
        remote.len(),
    ) + if !app_state.cached_remotes.is_empty() {
        4.0
    } else {
        0.0
    } + section_height(
        sidebar_state.tags_expanded && !app_state.cached_tags.is_empty(),
        app_state.cached_tags.len(),
    ) + if !app_state.cached_stashes.is_empty() {
        4.0 + section_height(
            sidebar_state.stashes_expanded,
            app_state.cached_stashes.len(),
        )
    } else {
        0.0
    } + if !app_state.github_pull_requests.is_empty() {
        4.0 + section_height(
            sidebar_state.prs_expanded,
            app_state.github_pull_requests.len(),
        )
    } else {
        0.0
    } + if !app_state.github_action_runs.is_empty() {
        4.0 + section_height(
            sidebar_state.runs_expanded,
            app_state.github_action_runs.len(),
        )
    } else {
        0.0
    } + if !app_state.github_releases.is_empty() {
        4.0 + section_height(
            sidebar_state.releases_expanded,
            app_state.github_releases.len(),
        )
    } else {
        0.0
    } + if !app_state.github_packages.is_empty() {
        4.0 + section_height(
            sidebar_state.packages_expanded,
            app_state.github_packages.len(),
        )
    } else {
        0.0
    };

    let scroll_rect = egui::Rect::from_min_max(egui::pos2(rect.left(), y), rect.right_bottom());

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("app_sidebar_scroll_host")
            .max_rect(scroll_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            egui::ScrollArea::vertical()
                .id_salt("app_sidebar_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let (content_rect, _) = ui.allocate_exact_size(
                        egui::vec2(ui.available_width(), content_height),
                        egui::Sense::hover(),
                    );
                    let mut local_y = content_rect.top();

                    if !local.is_empty() {
                        paint_section(
                            ui,
                            content_rect,
                            local_y,
                            "Branches",
                            &mut sidebar_state.branches_expanded,
                            text,
                        );
                        local_y += ROW_HEIGHT;
                        if sidebar_state.branches_expanded {
                            for branch in &local {
                                let icon = if branch.is_current { CHECK } else { FOLDER };
                                let response = paint_tree_row(
                                    ui,
                                    content_rect,
                                    local_y,
                                    1,
                                    icon,
                                    &branch.name,
                                    branch.is_current,
                                    text,
                                    muted,
                                    None,
                                    &format!("local_{}", branch.name),
                                    Some(get_branch_color(
                                        &branch.name,
                                        &app_state.cached_branches,
                                    )),
                                );

                                if response.double_clicked() {
                                    action =
                                        Some(SidebarAction::CheckoutBranch(branch.name.clone()));
                                }

                                let branch_name = branch.name.clone();
                                let is_current = branch.is_current;
                                response.context_menu(|ui| {
                                    let btn = ui.add_enabled(
                                        !is_current,
                                        egui::Button::new("Delete Branch"),
                                    );
                                    if btn.clicked() {
                                        action =
                                            Some(SidebarAction::DeleteBranch(branch_name.clone()));
                                        ui.close();
                                    }
                                });

                                local_y += ROW_HEIGHT;
                            }
                        }
                    }

                    if !remote.is_empty() {
                        paint_section(
                            ui,
                            content_rect,
                            local_y,
                            "Remotes",
                            &mut sidebar_state.remotes_expanded,
                            text,
                        );
                        local_y += ROW_HEIGHT;
                        if sidebar_state.remotes_expanded {
                            for branch in &remote {
                                paint_tree_row(
                                    ui,
                                    content_rect,
                                    local_y,
                                    1,
                                    GITHUB_LOGO,
                                    &branch.name,
                                    false,
                                    text,
                                    muted,
                                    None,
                                    &format!("remote_{}", branch.name),
                                    Some(get_branch_color(
                                        &branch.name,
                                        &app_state.cached_branches,
                                    )),
                                );
                                local_y += ROW_HEIGHT;
                            }
                        }
                    }

                    if !app_state.cached_remotes.is_empty() {
                        local_y += 4.0;
                    }

                    if !app_state.cached_tags.is_empty() {
                        paint_section(
                            ui,
                            content_rect,
                            local_y,
                            "Tags",
                            &mut sidebar_state.tags_expanded,
                            text,
                        );
                        local_y += ROW_HEIGHT;
                        if sidebar_state.tags_expanded {
                            for tag in &app_state.cached_tags {
                                paint_tree_row(
                                    ui,
                                    content_rect,
                                    local_y,
                                    1,
                                    FUNNEL,
                                    &tag.name,
                                    false,
                                    text,
                                    muted,
                                    None,
                                    &format!("tag_{}", tag.name),
                                    None,
                                );
                                local_y += ROW_HEIGHT;
                            }
                        }
                    }

                    if !app_state.cached_stashes.is_empty() {
                        local_y += 4.0;
                        paint_section(
                            ui,
                            content_rect,
                            local_y,
                            "Stashes",
                            &mut sidebar_state.stashes_expanded,
                            text,
                        );
                        local_y += ROW_HEIGHT;
                        if sidebar_state.stashes_expanded {
                            for (idx, stash) in app_state.cached_stashes.iter().enumerate() {
                                let label = format!("stash@{{{}}}: {}", idx, stash.message);
                                let response = paint_tree_row(
                                    ui,
                                    content_rect,
                                    local_y,
                                    1,
                                    STACK,
                                    &label,
                                    false,
                                    text,
                                    muted,
                                    Some((&stash.hash, muted)),
                                    &format!("stash_{}", stash.hash),
                                    None,
                                );

                                response.context_menu(|ui| {
                                    if ui.button("Apply Stash").clicked() {
                                        action = Some(SidebarAction::StashApply(idx));
                                        ui.close();
                                    }
                                    if ui.button("Pop Stash").clicked() {
                                        action = Some(SidebarAction::StashPop(idx));
                                        ui.close();
                                    }
                                    if ui.button("Drop Stash").clicked() {
                                        action = Some(SidebarAction::StashDrop(idx));
                                        ui.close();
                                    }
                                });

                                local_y += ROW_HEIGHT;
                            }
                        }
                    }

                    // Pull Requests
                    if !app_state.github_pull_requests.is_empty() {
                        local_y += 4.0;
                        paint_section(
                            ui,
                            content_rect,
                            local_y,
                            "Pull Requests",
                            &mut sidebar_state.prs_expanded,
                            text,
                        );
                        local_y += ROW_HEIGHT;
                        if sidebar_state.prs_expanded {
                            for pr in &app_state.github_pull_requests {
                                let label = format!("#{} {}", pr.number, pr.title);
                                let response = paint_tree_row(
                                    ui,
                                    content_rect,
                                    local_y,
                                    1,
                                    GIT_PULL_REQUEST,
                                    &label,
                                    false,
                                    text,
                                    muted,
                                    None,
                                    &format!("pr_{}", pr.number),
                                    None,
                                );
                                if response.clicked() {
                                    action = Some(SidebarAction::OpenUrl(pr.html_url.clone()));
                                }
                                local_y += ROW_HEIGHT;
                            }
                        }
                    }

                    // GitHub Actions Runs
                    if !app_state.github_action_runs.is_empty() {
                        local_y += 4.0;
                        paint_section(
                            ui,
                            content_rect,
                            local_y,
                            "Actions",
                            &mut sidebar_state.runs_expanded,
                            text,
                        );
                        local_y += ROW_HEIGHT;
                        if sidebar_state.runs_expanded {
                            for run in &app_state.github_action_runs {
                                let icon = match run.conclusion.as_deref() {
                                    Some("success") => CHECK_CIRCLE,
                                    Some("failure") => WARNING_CIRCLE,
                                    _ => PLAY_CIRCLE,
                                };
                                let response = paint_tree_row(
                                    ui,
                                    content_rect,
                                    local_y,
                                    1,
                                    icon,
                                    &run.name,
                                    false,
                                    text,
                                    muted,
                                    Some((&run.head_branch, muted)),
                                    &format!("run_{}", run.id),
                                    None,
                                );
                                if response.clicked() {
                                    action = Some(SidebarAction::OpenUrl(run.html_url.clone()));
                                }
                                local_y += ROW_HEIGHT;
                            }
                        }
                    }

                    // Releases
                    if !app_state.github_releases.is_empty() {
                        local_y += 4.0;
                        paint_section(
                            ui,
                            content_rect,
                            local_y,
                            "Releases",
                            &mut sidebar_state.releases_expanded,
                            text,
                        );
                        local_y += ROW_HEIGHT;
                        if sidebar_state.releases_expanded {
                            for release in &app_state.github_releases {
                                let label = release.name.as_ref().unwrap_or(&release.tag_name);
                                let response = paint_tree_row(
                                    ui,
                                    content_rect,
                                    local_y,
                                    1,
                                    BOOKMARK,
                                    label,
                                    false,
                                    text,
                                    muted,
                                    None,
                                    &format!("release_{}", release.tag_name),
                                    None,
                                );
                                if response.clicked() {
                                    action = Some(SidebarAction::OpenUrl(release.html_url.clone()));
                                }
                                local_y += ROW_HEIGHT;
                            }
                        }
                    }

                    // Packages
                    if !app_state.github_packages.is_empty() {
                        local_y += 4.0;
                        paint_section(
                            ui,
                            content_rect,
                            local_y,
                            "Packages",
                            &mut sidebar_state.packages_expanded,
                            text,
                        );
                        local_y += ROW_HEIGHT;
                        if sidebar_state.packages_expanded {
                            for pkg in &app_state.github_packages {
                                let response = paint_tree_row(
                                    ui,
                                    content_rect,
                                    local_y,
                                    1,
                                    PACKAGE,
                                    &pkg.name,
                                    false,
                                    text,
                                    muted,
                                    Some((&pkg.package_type, muted)),
                                    &format!("package_{}", pkg.name),
                                    None,
                                );
                                if response.clicked() {
                                    action = Some(SidebarAction::OpenUrl(pkg.html_url.clone()));
                                }
                                local_y += ROW_HEIGHT;
                            }
                        }
                    }
                });
        },
    );

    action
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
        TREE_VIEW,
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

fn paint_section(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    label: &str,
    expanded: &mut bool,
    text: egui::Color32,
) {
    let row = row_rect(rect, y, ROW_HEIGHT);
    let caret = if *expanded { CARET_DOWN } else { CARET_RIGHT };

    let response = ui.interact(
        row,
        ui.make_persistent_id(("app_sidebar_section", label)),
        egui::Sense::click(),
    );
    if response.clicked() {
        *expanded = !*expanded;
    }

    painter_text(
        ui,
        egui::pos2(row.left() + 14.0, row.center().y),
        caret,
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
    id_salt: &str,
    bg_gradient_color: Option<egui::Color32>,
) -> egui::Response {
    let row = row_rect(rect, y, ROW_HEIGHT);
    let response = ui.interact(
        row,
        ui.make_persistent_id(("app_sidebar_row", id_salt, label)),
        egui::Sense::click(),
    );

    if let Some(base_color) = bg_gradient_color {
        let alpha = if response.hovered() { 50 } else { 20 };
        let left_color = egui::Color32::from_rgba_unmultiplied(
            base_color.r(),
            base_color.g(),
            base_color.b(),
            alpha,
        );
        let right_color = egui::Color32::from_rgba_unmultiplied(
            base_color.r(),
            base_color.g(),
            base_color.b(),
            0,
        );
        paint_gradient_rect(ui, row, left_color, right_color);
    } else if response.hovered() {
        ui.painter().rect_filled(
            row,
            0.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 10),
        );
    }

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

    response
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

fn paint_gradient_rect(
    ui: &egui::Ui,
    rect: egui::Rect,
    left_color: egui::Color32,
    right_color: egui::Color32,
) {
    let mut mesh = egui::epaint::Mesh::default();
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_top(),
        uv: egui::Pos2::ZERO,
        color: left_color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_top(),
        uv: egui::Pos2::ZERO,
        color: right_color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.right_bottom(),
        uv: egui::Pos2::ZERO,
        color: right_color,
    });
    mesh.vertices.push(egui::epaint::Vertex {
        pos: rect.left_bottom(),
        uv: egui::Pos2::ZERO,
        color: left_color,
    });
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(mesh);
}
