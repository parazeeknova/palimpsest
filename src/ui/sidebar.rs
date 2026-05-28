use eframe::egui;
use egui_phosphor::regular::{
    ARROW_COUNTER_CLOCKWISE, ARROW_DOWN, ARROW_UP, ARROW_UP_RIGHT, BOOKMARK, CARET_DOWN,
    CARET_RIGHT, CHECK_CIRCLE, CLOCK, CLOUD, DOTS_THREE_VERTICAL, EYE, FILE_TEXT, FUNNEL, GEAR_SIX,
    GIT_BRANCH, GIT_PULL_REQUEST, LAPTOP, LIST, MAGNIFYING_GLASS, PACKAGE, PLAY_CIRCLE, PROHIBIT,
    STACK, TAG, TREE_VIEW, WARNING_CIRCLE,
};

use crate::state::AppState;

pub const SIDEBAR_WIDTH: f32 = 236.0;
const HEADER_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 24.0;
const FILTER_HEIGHT: f32 = 26.0;
use crate::ui::colors::get_branch_color;

pub mod branch_dropdown;
pub mod remote_branch_dropdown;
pub mod remote_dropdown;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SidebarTab {
    Repository,
    Search,
    FileTree,
}

pub struct SidebarState {
    pub current_tab: SidebarTab,
    pub branches_expanded: bool,
    pub remotes_expanded: bool,
    pub tags_expanded: bool,
    pub tags_show_all: bool,
    pub stashes_expanded: bool,
    pub prs_expanded: bool,
    pub runs_expanded: bool,
    pub runs_show_all: bool,
    pub releases_expanded: bool,
    pub releases_show_all: bool,
    pub packages_expanded: bool,
    pub repo_tree_state: crate::ui::core::filetree::TreeState,
    pub search_query: String,
    pub filter_query: String,
    pub cached_head_hash: Option<String>,
    pub cached_tracked_files: Vec<String>,
    pub collapsed_remotes: std::collections::HashSet<String>,
    pub last_loaded_repo_path: Option<String>,
}

impl Default for SidebarState {
    fn default() -> Self {
        Self {
            current_tab: SidebarTab::Repository,
            branches_expanded: true,
            remotes_expanded: false,
            tags_expanded: false,
            tags_show_all: false,
            stashes_expanded: false,
            prs_expanded: false,
            runs_expanded: false,
            runs_show_all: false,
            releases_expanded: false,
            releases_show_all: false,
            packages_expanded: false,
            repo_tree_state: crate::ui::core::filetree::TreeState::default(),
            search_query: String::new(),
            filter_query: String::new(),
            cached_head_hash: None,
            cached_tracked_files: Vec::new(),
            collapsed_remotes: std::collections::HashSet::new(),
            last_loaded_repo_path: None,
        }
    }
}

pub enum SidebarAction {
    CheckoutBranch(String),
    CheckoutRemoteBranch {
        local_name: String,
        remote_name: String,
    },
    DeleteBranch(String),
    StashApply(usize),
    StashPop(usize),
    StashDrop(usize),
    OpenUrl(String),
    Fetch,
    RefreshActions,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SectionKind {
    Local,
    Remotes,
    Tags,
    Stashes,
    PRs,
    Runs,
    Releases,
    Packages,
}

#[allow(unused_assignments)]
pub fn show_cached(
    ui: &mut egui::Ui,
    sidebar_state: &mut SidebarState,
    repo_name: Option<&str>,
    app_state: &AppState,
    git_repo: Option<&crate::git::repo::GitRepo>,
) -> Option<SidebarAction> {
    let height = ui.available_height();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(SIDEBAR_WIDTH, height), egui::Sense::hover());

    let text_edit_id = egui::Id::new("sidebar_filter_input");
    if ui.input(|i| i.modifiers.ctrl && i.modifiers.alt && i.key_pressed(egui::Key::F)) {
        sidebar_state.current_tab = SidebarTab::Repository;
        ui.ctx().memory_mut(|mem| mem.request_focus(text_edit_id));
    }

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

    paint_mode_bar(ui, rect, y, sidebar_state, blue, muted, stroke);
    y += 34.0;

    let mut action = None;

    match sidebar_state.current_tab {
        SidebarTab::Repository => {
            // Changes row — show file count, +adds, -dels from cached_status
            let changes_trailing = if let Some(ref status) = app_state.cached_status {
                let files = status.files_changed;
                let additions = status.additions;
                let deletions = status.deletions;
                NavRowTrailing::DiffStats {
                    files,
                    additions,
                    deletions,
                }
            } else {
                NavRowTrailing::None
            };
            paint_nav_row(
                ui,
                rect,
                y,
                FILE_TEXT,
                "Changes",
                false,
                text,
                selected,
                changes_trailing,
            );
            y += ROW_HEIGHT;

            // All Commits row — show total commit count
            let commits_trailing = NavRowTrailing::Count(app_state.cached_commits.len());
            paint_nav_row(
                ui,
                rect,
                y,
                LIST,
                "All Commits",
                true,
                text,
                selected,
                commits_trailing,
            );
            y += ROW_HEIGHT;

            let query = sidebar_state.filter_query.to_lowercase();

            let local: Vec<_> = app_state
                .cached_branches
                .iter()
                .filter(|b| !b.is_remote)
                .filter(|b| query.is_empty() || b.name.to_lowercase().contains(&query))
                .collect();
            let remote: Vec<_> = app_state
                .cached_branches
                .iter()
                .filter(|b| b.is_remote && !b.name.ends_with("/HEAD"))
                .filter(|b| query.is_empty() || b.name.to_lowercase().contains(&query))
                .collect();

            let tags: Vec<_> = app_state
                .cached_tags
                .iter()
                .filter(|t| query.is_empty() || t.name.to_lowercase().contains(&query))
                .collect();
            let stashes: Vec<_> = app_state
                .cached_stashes
                .iter()
                .enumerate()
                .filter(|(idx, s)| {
                    query.is_empty()
                        || s.message.to_lowercase().contains(&query)
                        || format!("stash@{{{}}}", idx).contains(&query)
                        || s.hash.to_lowercase().contains(&query)
                })
                .collect();
            let prs: Vec<_> = app_state
                .github_pull_requests
                .iter()
                .filter(|pr| {
                    query.is_empty()
                        || pr.title.to_lowercase().contains(&query)
                        || pr.number.to_string().contains(&query)
                })
                .collect();
            let runs: Vec<_> = app_state
                .github_action_runs
                .iter()
                .filter(|run| {
                    query.is_empty()
                        || run.name.to_lowercase().contains(&query)
                        || run.head_branch.to_lowercase().contains(&query)
                        || run.run_number.to_string().contains(&query)
                })
                .collect();
            let releases: Vec<_> = app_state
                .github_releases
                .iter()
                .filter(|rel| {
                    query.is_empty()
                        || rel.tag_name.to_lowercase().contains(&query)
                        || rel
                            .name
                            .as_deref()
                            .unwrap_or("")
                            .to_lowercase()
                            .contains(&query)
                })
                .collect();
            let packages: Vec<_> = app_state
                .github_packages
                .iter()
                .filter(|pkg| query.is_empty() || pkg.name.to_lowercase().contains(&query))
                .collect();

            let total_searchable = local.len()
                + remote.len()
                + tags.len()
                + stashes.len()
                + prs.len()
                + runs.len()
                + releases.len()
                + packages.len();

            y += 8.0;
            paint_filter(ui, rect, y, sidebar_state, text, muted, stroke);

            let count_y = y + FILTER_HEIGHT + 6.0;
            painter_text(
                ui,
                egui::pos2(rect.left() + 10.0, count_y),
                &format!("viewing : {}", total_searchable),
                10.0,
                muted.linear_multiply(0.7),
                egui::Align2::LEFT_CENTER,
            );
            y += FILTER_HEIGHT + 18.0;
            let mut expanded_sections = Vec::new();
            let mut collapsed_sections = Vec::new();
            let is_searching = !query.is_empty();

            if !local.is_empty() {
                if sidebar_state.branches_expanded || is_searching {
                    expanded_sections.push(SectionKind::Local);
                } else {
                    collapsed_sections.push(SectionKind::Local);
                }
            }
            if !remote.is_empty() {
                if sidebar_state.remotes_expanded || is_searching {
                    expanded_sections.push(SectionKind::Remotes);
                } else {
                    collapsed_sections.push(SectionKind::Remotes);
                }
            }
            if !tags.is_empty() {
                if sidebar_state.tags_expanded || is_searching {
                    expanded_sections.push(SectionKind::Tags);
                } else {
                    collapsed_sections.push(SectionKind::Tags);
                }
            }
            if !stashes.is_empty() {
                if sidebar_state.stashes_expanded || is_searching {
                    expanded_sections.push(SectionKind::Stashes);
                } else {
                    collapsed_sections.push(SectionKind::Stashes);
                }
            }
            if !prs.is_empty() {
                if sidebar_state.prs_expanded || is_searching {
                    expanded_sections.push(SectionKind::PRs);
                } else {
                    collapsed_sections.push(SectionKind::PRs);
                }
            }
            if !runs.is_empty() {
                if sidebar_state.runs_expanded || is_searching {
                    expanded_sections.push(SectionKind::Runs);
                } else {
                    collapsed_sections.push(SectionKind::Runs);
                }
            }
            if !releases.is_empty() {
                if sidebar_state.releases_expanded || is_searching {
                    expanded_sections.push(SectionKind::Releases);
                } else {
                    collapsed_sections.push(SectionKind::Releases);
                }
            }
            if !packages.is_empty() {
                if sidebar_state.packages_expanded || is_searching {
                    expanded_sections.push(SectionKind::Packages);
                } else {
                    collapsed_sections.push(SectionKind::Packages);
                }
            }

            let collapsed_height = collapsed_sections.len() as f32 * ROW_HEIGHT;

            let mut top_content_height = 0.0;
            let mut first_expanded = true;
            for section in &expanded_sections {
                if !first_expanded {
                    top_content_height += 4.0;
                }
                first_expanded = false;

                let section_h = match section {
                    SectionKind::Runs => {
                        let total = runs.len();
                        if total > 5 {
                            let show_count = if sidebar_state.runs_show_all {
                                total
                            } else {
                                5
                            };
                            ROW_HEIGHT + show_count as f32 * 40.0 + ROW_HEIGHT
                        } else {
                            ROW_HEIGHT + total as f32 * 40.0
                        }
                    }
                    _ => {
                        let count = match section {
                            SectionKind::Local => local.len(),
                            SectionKind::Remotes => remote.len(),
                            SectionKind::Tags => {
                                let total = tags.len();
                                if total > 5 {
                                    if sidebar_state.tags_show_all {
                                        total + 1
                                    } else {
                                        5 + 1
                                    }
                                } else {
                                    total
                                }
                            }
                            SectionKind::Stashes => stashes.len(),
                            SectionKind::PRs => prs.len(),
                            SectionKind::Releases => {
                                let total = releases.len();
                                if total > 5 {
                                    if sidebar_state.releases_show_all {
                                        total + 1
                                    } else {
                                        5 + 1
                                    }
                                } else {
                                    total
                                }
                            }
                            SectionKind::Packages => packages.len(),
                            SectionKind::Runs => unreachable!(),
                        };
                        ROW_HEIGHT + count as f32 * ROW_HEIGHT
                    }
                };
                top_content_height += section_h;
            }

            let scroll_rect =
                egui::Rect::from_min_max(egui::pos2(rect.left(), y), rect.right_bottom());

            let top_scroll_rect = egui::Rect::from_min_max(
                scroll_rect.min,
                egui::pos2(scroll_rect.max.x, scroll_rect.max.y - collapsed_height),
            );

            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("app_sidebar_scroll_host")
                    .max_rect(top_scroll_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("app_sidebar_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            let (content_rect, _) = ui.allocate_exact_size(
                                egui::vec2(ui.available_width(), top_content_height),
                                egui::Sense::hover(),
                            );
                            let mut local_y = content_rect.top();
                            let mut first_expanded = true;

                            for section in &expanded_sections {
                                if !first_expanded {
                                    local_y += 4.0;
                                }
                                first_expanded = false;

                                match section {
                                    SectionKind::Local => {
                                        let mut is_expanded = sidebar_state.branches_expanded || !query.is_empty();
                                        let old_expanded = is_expanded;
                                        paint_section(
                                            ui,
                                            content_rect,
                                            local_y,
                                            "Local",
                                            LAPTOP,
                                            &mut is_expanded,
                                            text,
                                            &mut action,
                                            false,
                                            Some(local.len()),
                                        );
                                        if is_expanded != old_expanded {
                                            sidebar_state.branches_expanded = !sidebar_state.branches_expanded;
                                        }
                                        local_y += ROW_HEIGHT;
                                        for branch in &local {
                                            let response = paint_tree_row(
                                                ui,
                                                content_rect,
                                                local_y,
                                                1,
                                                GIT_BRANCH,
                                                &branch.name,
                                                branch.is_current,
                                                text,
                                                muted,
                                                None,
                                                TrailingStyle::None,
                                                &format!("local_{}", branch.name),
                                                Some(get_branch_color(
                                                    &branch.name,
                                                    &app_state.cached_branches,
                                                )),
                                                None,
                                            );

                                            let row = row_rect(content_rect, local_y, ROW_HEIGHT);

                                            let mut current_x = row.right() - 26.0;
                                            let gray_color = egui::Color32::from_rgb(135, 135, 135);
                                            if let Some(behind) = branch.behind {
                                                if behind > 0 {
                                                    let label = format!("{} {}", behind, ARROW_DOWN);
                                                    painter_bold_text(
                                                        ui,
                                                        egui::pos2(current_x, row.center().y),
                                                        &label,
                                                        11.0,
                                                        gray_color,
                                                        egui::Align2::RIGHT_CENTER,
                                                    );
                                                    current_x -= (label.chars().count() as f32 * 6.5) + 6.0;
                                                }
                                            }
                                            if let Some(ahead) = branch.ahead {
                                                if ahead > 0 {
                                                    let label = format!("{} {}", ahead, ARROW_UP);
                                                    painter_bold_text(
                                                        ui,
                                                        egui::pos2(current_x, row.center().y),
                                                        &label,
                                                        11.0,
                                                        gray_color,
                                                        egui::Align2::RIGHT_CENTER,
                                                    );
                                                }
                                            }

                                            let dropdown_rect = egui::Rect::from_min_max(
                                                egui::pos2(row.right() - 22.0, row.top() + 2.0),
                                                egui::pos2(row.right() - 4.0, row.bottom() - 2.0),
                                            );

                                            let dropdown_id = ui.make_persistent_id((
                                                "local_branch_dropdown_btn",
                                                &branch.name,
                                            ));
                                            let dropdown_resp = ui.interact(
                                                dropdown_rect,
                                                dropdown_id,
                                                egui::Sense::click(),
                                            );

                                            let popup_id: egui::Id = dropdown_id.with("popup");
                                            let is_open =
                                                egui::Popup::is_id_open(ui.ctx(), popup_id);

                                            if dropdown_resp.hovered() || is_open {
                                                ui.painter().rect_filled(
                                                    dropdown_rect,
                                                    4.0,
                                                    egui::Color32::from_rgba_unmultiplied(
                                                        255, 255, 255, 12,
                                                    ),
                                                );
                                            }

                                            let icon_color = if dropdown_resp.hovered() || is_open {
                                                text
                                            } else {
                                                muted
                                            };

                                            painter_text(
                                                ui,
                                                dropdown_rect.center(),
                                                DOTS_THREE_VERTICAL,
                                                12.0,
                                                icon_color,
                                                egui::Align2::CENTER_CENTER,
                                            );

                                            branch_dropdown::show(ui, branch, &dropdown_resp);

                                            let is_hovering_dropdown = ui.input(|i| {
                                                i.pointer
                                                    .hover_pos()
                                                    .is_some_and(|pos| dropdown_rect.contains(pos))
                                            });

                                            if response.double_clicked()
                                                && !branch.is_current
                                                && !is_hovering_dropdown
                                            {
                                                action = Some(SidebarAction::CheckoutBranch(
                                                    branch.name.clone(),
                                                ));
                                            }

                                            let branch_name = branch.name.clone();
                                            let is_current = branch.is_current;
                                            response.context_menu(|ui| {
                                                let btn = ui.add_enabled(
                                                    !is_current,
                                                    egui::Button::new("Delete Branch"),
                                                );
                                                if btn.clicked() {
                                                    action = Some(SidebarAction::DeleteBranch(
                                                        branch_name.clone(),
                                                    ));
                                                    ui.close();
                                                }
                                            });

                                            local_y += ROW_HEIGHT;
                                        }
                                    }
                                    SectionKind::Remotes => {
                                        let is_fetching =
                                            app_state.repo_error.as_deref() == Some("Fetching...");
                                        let mut is_expanded = sidebar_state.remotes_expanded || !query.is_empty();
                                        let old_expanded = is_expanded;
                                        paint_section(
                                            ui,
                                            content_rect,
                                            local_y,
                                            "Remotes",
                                            CLOUD,
                                            &mut is_expanded,
                                            text,
                                            &mut action,
                                            is_fetching,
                                            Some(remote.len()),
                                        );
                                        if is_expanded != old_expanded {
                                            sidebar_state.remotes_expanded = !sidebar_state.remotes_expanded;
                                        }
                                        local_y += ROW_HEIGHT;

                                        if sidebar_state.remotes_expanded || !query.is_empty() {
                                            let mut remote_groups: std::collections::BTreeMap<String, Vec<&crate::state::CachedBranch>> = std::collections::BTreeMap::new();
                                            for branch in &remote {
                                                let parts: Vec<&str> = branch.name.splitn(2, '/').collect();
                                                let remote_name = if parts.len() == 2 { parts[0].to_string() } else { "other".to_string() };
                                                remote_groups.entry(remote_name).or_default().push(*branch);
                                            }

                                            for (remote_name, branches) in remote_groups {
                                                let is_expanded = !sidebar_state.collapsed_remotes.contains(&remote_name);

                                                // Look up avatar for this remote's GitHub owner
                                                let avatar_url: Option<String> = app_state.cached_remotes.iter()
                                                    .find(|r| r.name == remote_name)
                                                    .and_then(|r| parse_github_owner_from_url(&r.url))
                                                    .and_then(|owner| {
                                                        // If the owner matches logged-in user, use their avatar_url directly
                                                        if let Some(user) = &app_state.github_user {
                                                            if user.login.eq_ignore_ascii_case(&owner) {
                                                                 return Some(user.avatar_url.clone());
                                                            }
                                                        }
                                                        // Fallback: check avatar_cache by owner name
                                                        app_state.avatar_cache.get(&owner).cloned()
                                                    });

                                                let group_res = paint_remote_group_row(
                                                    ui,
                                                    RemoteGroupRowArgs {
                                                        rect: content_rect,
                                                        y: local_y,
                                                        label: &remote_name,
                                                        expanded: is_expanded,
                                                        text,
                                                        muted,
                                                        id_salt: &remote_name,
                                                        avatar_path_or_url: avatar_url.as_deref(),
                                                    },
                                                );

                                                let row = row_rect(content_rect, local_y, ROW_HEIGHT);
                                                let dropdown_rect = egui::Rect::from_min_max(
                                                    egui::pos2(row.right() - 22.0, row.top() + 2.0),
                                                    egui::pos2(row.right() - 4.0, row.bottom() - 2.0),
                                                );

                                                let dropdown_id = ui.make_persistent_id((
                                                    "remote_group_dropdown_btn",
                                                    &remote_name,
                                                ));
                                                let dropdown_resp = ui.interact(
                                                    dropdown_rect,
                                                    dropdown_id,
                                                    egui::Sense::click(),
                                                );

                                                let popup_id: egui::Id = dropdown_id.with("popup");
                                                let is_open =
                                                    egui::Popup::is_id_open(ui.ctx(), popup_id);

                                                if dropdown_resp.hovered() || is_open {
                                                    ui.painter().rect_filled(
                                                        dropdown_rect,
                                                        4.0,
                                                        egui::Color32::from_rgba_unmultiplied(
                                                            255, 255, 255, 12,
                                                        ),
                                                    );
                                                }

                                                let icon_color = if dropdown_resp.hovered() || is_open {
                                                    text
                                                } else {
                                                    muted
                                                };

                                                painter_text(
                                                    ui,
                                                    dropdown_rect.center(),
                                                    DOTS_THREE_VERTICAL,
                                                    12.0,
                                                    icon_color,
                                                    egui::Align2::CENTER_CENTER,
                                                );

                                                let current_branch_name = local.iter()
                                                    .find(|b| b.is_current)
                                                    .map(|b| b.name.as_str())
                                                    .unwrap_or("dev");

                                                remote_dropdown::show(ui, &remote_name, current_branch_name, &dropdown_resp);

                                                let is_hovering_dropdown = ui.input(|i| {
                                                    i.pointer
                                                        .hover_pos()
                                                        .is_some_and(|pos| dropdown_rect.contains(pos))
                                                });

                                                if group_res.clicked() && !is_hovering_dropdown {
                                                    if is_expanded {
                                                        sidebar_state.collapsed_remotes.insert(remote_name.clone());
                                                    } else {
                                                        sidebar_state.collapsed_remotes.remove(&remote_name);
                                                    }
                                                }

                                                local_y += ROW_HEIGHT;

                                                if is_expanded {
                                                    for branch in branches {
                                                        let display_name = if let Some(pos) = branch.name.find('/') {
                                                            &branch.name[pos + 1..]
                                                        } else {
                                                            &branch.name
                                                        };

                                                        let response = paint_tree_row(
                                                            ui,
                                                            content_rect,
                                                            local_y,
                                                            2, // Level 2 indent
                                                            GIT_BRANCH,
                                                            display_name,
                                                            false,
                                                            text,
                                                            muted,
                                                            None,
                                                            TrailingStyle::None,
                                                            &format!("remote_{}", branch.name),
                                                            Some(get_branch_color(
                                                                &branch.name,
                                                                &app_state.cached_branches,
                                                            )),
                                                            None,
                                                        );

                                                        let row = row_rect(content_rect, local_y, ROW_HEIGHT);
                                                        let dropdown_rect = egui::Rect::from_min_max(
                                                            egui::pos2(row.right() - 22.0, row.top() + 2.0),
                                                            egui::pos2(row.right() - 4.0, row.bottom() - 2.0),
                                                        );

                                                        let dropdown_id = ui.make_persistent_id((
                                                            "remote_branch_dropdown_btn",
                                                            &branch.name,
                                                        ));
                                                        let dropdown_resp = ui.interact(
                                                            dropdown_rect,
                                                            dropdown_id,
                                                            egui::Sense::click(),
                                                        );

                                                        let popup_id: egui::Id = dropdown_id.with("popup");
                                                        let is_open =
                                                            egui::Popup::is_id_open(ui.ctx(), popup_id);

                                                        if dropdown_resp.hovered() || is_open {
                                                            ui.painter().rect_filled(
                                                                dropdown_rect,
                                                                4.0,
                                                                egui::Color32::from_rgba_unmultiplied(
                                                                    255, 255, 255, 12,
                                                                ),
                                                            );
                                                        }

                                                        let icon_color = if dropdown_resp.hovered() || is_open {
                                                            text
                                                        } else {
                                                            muted
                                                        };

                                                        painter_text(
                                                            ui,
                                                            dropdown_rect.center(),
                                                            DOTS_THREE_VERTICAL,
                                                            12.0,
                                                            icon_color,
                                                            egui::Align2::CENTER_CENTER,
                                                        );

                                                        remote_branch_dropdown::show(ui, branch, current_branch_name, &dropdown_resp);

                                                        let is_hovering_dropdown = ui.input(|i| {
                                                            i.pointer
                                                                .hover_pos()
                                                                .is_some_and(|pos| dropdown_rect.contains(pos))
                                                        });

                                                        if response.double_clicked() && !is_hovering_dropdown {
                                                            let local_name = display_name;
                                                            let local_exists =
                                                                local.iter().any(|b| b.name == local_name);
                                                            if local_exists {
                                                                action = Some(SidebarAction::CheckoutBranch(
                                                                    local_name.to_string(),
                                                                ));
                                                            } else {
                                                                action =
                                                                    Some(SidebarAction::CheckoutRemoteBranch {
                                                                        local_name: local_name.to_string(),
                                                                        remote_name: branch.name.clone(),
                                                                    });
                                                            }
                                                        }

                                                        local_y += ROW_HEIGHT;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    SectionKind::Tags => {
                                        let mut is_expanded = sidebar_state.tags_expanded || !query.is_empty();
                                        let old_expanded = is_expanded;
                                        paint_section(
                                            ui,
                                            content_rect,
                                            local_y,
                                            "Tags",
                                            TAG,
                                            &mut is_expanded,
                                            text,
                                            &mut action,
                                            false,
                                            Some(tags.len()),
                                        );
                                        if is_expanded != old_expanded {
                                            sidebar_state.tags_expanded = !sidebar_state.tags_expanded;
                                        }
                                        local_y += ROW_HEIGHT;

                                        let total_tags = tags.len();
                                        let tags_to_show =
                                            if total_tags > 5 && !sidebar_state.tags_show_all {
                                                &tags[..5]
                                            } else {
                                                &tags[..]
                                            };

                                        for tag in tags_to_show {
                                            paint_tree_row(
                                                ui,
                                                content_rect,
                                                local_y,
                                                1,
                                                TAG,
                                                &tag.name,
                                                false,
                                                text,
                                                muted,
                                                None,
                                                TrailingStyle::None,
                                                &format!("tag_{}", tag.name),
                                                None,
                                                None,
                                            );
                                            local_y += ROW_HEIGHT;
                                        }

                                        if total_tags > 5 {
                                            let btn_label = if sidebar_state.tags_show_all {
                                                "Show Less"
                                            } else {
                                                "Show More"
                                            };

                                            let btn_res = paint_tree_row(
                                                ui,
                                                content_rect,
                                                local_y,
                                                1,
                                                "",
                                                btn_label,
                                                false,
                                                text,
                                                blue,
                                                None,
                                                TrailingStyle::None,
                                                "tags_toggle",
                                                None,
                                                None,
                                            );
                                            if btn_res.clicked() {
                                                sidebar_state.tags_show_all =
                                                    !sidebar_state.tags_show_all;
                                            }
                                            local_y += ROW_HEIGHT;
                                        }
                                    }
                                    SectionKind::Stashes => {
                                        let mut is_expanded = sidebar_state.stashes_expanded || !query.is_empty();
                                        let old_expanded = is_expanded;
                                        paint_section(
                                            ui,
                                            content_rect,
                                            local_y,
                                            "Stashes",
                                            STACK,
                                            &mut is_expanded,
                                            text,
                                            &mut action,
                                            false,
                                            Some(stashes.len()),
                                        );
                                        if is_expanded != old_expanded {
                                            sidebar_state.stashes_expanded = !sidebar_state.stashes_expanded;
                                        }
                                        local_y += ROW_HEIGHT;
                                        for &(idx, stash) in &stashes {
                                            let label =
                                                format!("stash@{{{}}}: {}", idx, stash.message);
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
                                                Some(stash.hash.as_str()),
                                                TrailingStyle::Stash,
                                                &format!("stash_{}", stash.hash),
                                                None,
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
                                    SectionKind::PRs => {
                                        let mut is_expanded = sidebar_state.prs_expanded || !query.is_empty();
                                        let old_expanded = is_expanded;
                                        paint_section(
                                            ui,
                                            content_rect,
                                            local_y,
                                            "Pull Requests",
                                            GIT_PULL_REQUEST,
                                            &mut is_expanded,
                                            text,
                                            &mut action,
                                            false,
                                            Some(prs.len()),
                                        );
                                        if is_expanded != old_expanded {
                                            sidebar_state.prs_expanded = !sidebar_state.prs_expanded;
                                        }
                                        local_y += ROW_HEIGHT;
                                        for pr in &prs {
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
                                                TrailingStyle::None,
                                                &format!("pr_{}", pr.number),
                                                None,
                                                None,
                                            );
                                            if response.clicked() {
                                                action = Some(SidebarAction::OpenUrl(
                                                    pr.html_url.clone(),
                                                ));
                                            }
                                            local_y += ROW_HEIGHT;
                                        }
                                    }
                                    SectionKind::Runs => {
                                        let mut is_expanded = sidebar_state.runs_expanded || !query.is_empty();
                                        let old_expanded = is_expanded;
                                        paint_section(
                                            ui,
                                            content_rect,
                                            local_y,
                                            "Actions",
                                            PLAY_CIRCLE,
                                            &mut is_expanded,
                                            text,
                                            &mut action,
                                            app_state.github_loading,
                                            Some(runs.len()),
                                        );
                                        if is_expanded != old_expanded {
                                            sidebar_state.runs_expanded = !sidebar_state.runs_expanded;
                                        }
                                        local_y += ROW_HEIGHT;

                                        let total_runs = runs.len();
                                        let runs_to_show =
                                            if total_runs > 5 && !sidebar_state.runs_show_all {
                                                &runs[..5]
                                            } else {
                                                &runs[..]
                                            };

                                        for run in runs_to_show {
                                            let (icon, icon_color) = match run.conclusion.as_deref()
                                            {
                                                Some("success") => (
                                                    CHECK_CIRCLE,
                                                    egui::Color32::from_rgb(39, 174, 96),
                                                ),
                                                Some("skipped") => (
                                                    PROHIBIT,
                                                    egui::Color32::from_rgb(120, 120, 120),
                                                ),
                                                Some("failure") => (
                                                    WARNING_CIRCLE,
                                                    egui::Color32::from_rgb(231, 76, 60),
                                                ),
                                                Some("cancelled")
                                                | Some("timed_out")
                                                | Some("action_required") => (
                                                    WARNING_CIRCLE,
                                                    egui::Color32::from_rgb(231, 76, 60),
                                                ),
                                                _ => (
                                                    PLAY_CIRCLE,
                                                    egui::Color32::from_rgb(241, 196, 15),
                                                ),
                                            };
                                            let response = paint_action_row(
                                                ui,
                                                content_rect,
                                                local_y,
                                                icon,
                                                &run.name,
                                                run.run_number,
                                                &run.status,
                                                &run.actor_login,
                                                &run.created_at,
                                                &run.updated_at,
                                                run.conclusion.as_deref(),
                                                &run.head_branch,
                                                get_branch_color(
                                                    &run.head_branch,
                                                    &app_state.cached_branches,
                                                ),
                                                text,
                                                muted,
                                                &app_state.avatar_cache,
                                                &format!("run_{}", run.id),
                                                icon_color,
                                            );
                                            if response.clicked() {
                                                action = Some(SidebarAction::OpenUrl(
                                                    run.html_url.clone(),
                                                ));
                                            }
                                            local_y += 40.0;
                                        }

                                        if total_runs > 5 {
                                            let btn_label = if sidebar_state.runs_show_all {
                                                "Show Less"
                                            } else {
                                                "Show More"
                                            };

                                            let btn_res = paint_tree_row(
                                                ui,
                                                content_rect,
                                                local_y,
                                                1,
                                                "", // Empty icon to align with label of actions
                                                btn_label,
                                                false,
                                                text,
                                                blue, // Interactive blue color
                                                None,
                                                TrailingStyle::None,
                                                "runs_toggle",
                                                None,
                                                None,
                                            );
                                            if btn_res.clicked() {
                                                sidebar_state.runs_show_all =
                                                    !sidebar_state.runs_show_all;
                                            }
                                            local_y += ROW_HEIGHT;
                                        }
                                    }
                                    SectionKind::Releases => {
                                        let mut is_expanded = sidebar_state.releases_expanded || !query.is_empty();
                                        let old_expanded = is_expanded;
                                        paint_section(
                                            ui,
                                            content_rect,
                                            local_y,
                                            "Releases",
                                            BOOKMARK,
                                            &mut is_expanded,
                                            text,
                                            &mut action,
                                            false,
                                            Some(releases.len()),
                                        );
                                        if is_expanded != old_expanded {
                                            sidebar_state.releases_expanded = !sidebar_state.releases_expanded;
                                        }
                                        local_y += ROW_HEIGHT;

                                        let total_releases = releases.len();
                                        let releases_to_show = if total_releases > 5
                                            && !sidebar_state.releases_show_all
                                        {
                                            &releases[..5]
                                        } else {
                                            &releases[..]
                                        };

                                        for release in releases_to_show {
                                            let label =
                                                release.name.as_ref().unwrap_or(&release.tag_name);
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
                                                TrailingStyle::None,
                                                &format!("release_{}", release.tag_name),
                                                None,
                                                None,
                                            );
                                            if response.clicked() {
                                                action = Some(SidebarAction::OpenUrl(
                                                    release.html_url.clone(),
                                                ));
                                            }
                                            local_y += ROW_HEIGHT;
                                        }

                                        if total_releases > 5 {
                                            let btn_label = if sidebar_state.releases_show_all {
                                                "Show Less"
                                            } else {
                                                "Show More"
                                            };

                                            let btn_res = paint_tree_row(
                                                ui,
                                                content_rect,
                                                local_y,
                                                1,
                                                "", // Empty icon to align with label of releases
                                                btn_label,
                                                false,
                                                text,
                                                blue, // Interactive blue color
                                                None,
                                                TrailingStyle::None,
                                                "releases_toggle",
                                                None,
                                                None,
                                            );
                                            if btn_res.clicked() {
                                                sidebar_state.releases_show_all =
                                                    !sidebar_state.releases_show_all;
                                            }
                                            local_y += ROW_HEIGHT;
                                        }
                                    }
                                    SectionKind::Packages => {
                                        let mut is_expanded = sidebar_state.packages_expanded || !query.is_empty();
                                        let old_expanded = is_expanded;
                                        paint_section(
                                            ui,
                                            content_rect,
                                            local_y,
                                            "Packages",
                                            PACKAGE,
                                            &mut is_expanded,
                                            text,
                                            &mut action,
                                            false,
                                            Some(packages.len()),
                                        );
                                        if is_expanded != old_expanded {
                                            sidebar_state.packages_expanded = !sidebar_state.packages_expanded;
                                        }
                                        local_y += ROW_HEIGHT;
                                        for pkg in &packages {
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
                                                Some(pkg.package_type.as_str()),
                                                TrailingStyle::Package,
                                                &format!("package_{}", pkg.name),
                                                None,
                                                None,
                                            );
                                            if response.clicked() {
                                                action = Some(SidebarAction::OpenUrl(
                                                    pkg.html_url.clone(),
                                                ));
                                            }
                                            local_y += ROW_HEIGHT;
                                        }
                                    }
                                }
                            }
                        });
                },
            );

            // Paint collapsed sections at the bottom
            let mut cur_bottom_y = scroll_rect.bottom() - collapsed_height;
            for section in &collapsed_sections {
                match section {
                    SectionKind::Local => {
                        paint_section(
                            ui,
                            rect,
                            cur_bottom_y,
                            "Local",
                            LAPTOP,
                            &mut sidebar_state.branches_expanded,
                            text,
                            &mut action,
                            false,
                            Some(local.len()),
                        );
                    }
                    SectionKind::Remotes => {
                        let is_fetching = app_state.repo_error.as_deref() == Some("Fetching...");
                        paint_section(
                            ui,
                            rect,
                            cur_bottom_y,
                            "Remotes",
                            CLOUD,
                            &mut sidebar_state.remotes_expanded,
                            text,
                            &mut action,
                            is_fetching,
                            Some(remote.len()),
                        );
                    }
                    SectionKind::Tags => {
                        paint_section(
                            ui,
                            rect,
                            cur_bottom_y,
                            "Tags",
                            TAG,
                            &mut sidebar_state.tags_expanded,
                            text,
                            &mut action,
                            false,
                            Some(tags.len()),
                        );
                    }
                    SectionKind::Stashes => {
                        paint_section(
                            ui,
                            rect,
                            cur_bottom_y,
                            "Stashes",
                            STACK,
                            &mut sidebar_state.stashes_expanded,
                            text,
                            &mut action,
                            false,
                            Some(stashes.len()),
                        );
                    }
                    SectionKind::PRs => {
                        paint_section(
                            ui,
                            rect,
                            cur_bottom_y,
                            "Pull Requests",
                            GIT_PULL_REQUEST,
                            &mut sidebar_state.prs_expanded,
                            text,
                            &mut action,
                            false,
                            Some(prs.len()),
                        );
                    }
                    SectionKind::Runs => {
                        paint_section(
                            ui,
                            rect,
                            cur_bottom_y,
                            "Actions",
                            PLAY_CIRCLE,
                            &mut sidebar_state.runs_expanded,
                            text,
                            &mut action,
                            app_state.github_loading,
                            Some(runs.len()),
                        );
                    }
                    SectionKind::Releases => {
                        paint_section(
                            ui,
                            rect,
                            cur_bottom_y,
                            "Releases",
                            BOOKMARK,
                            &mut sidebar_state.releases_expanded,
                            text,
                            &mut action,
                            false,
                            Some(releases.len()),
                        );
                    }
                    SectionKind::Packages => {
                        paint_section(
                            ui,
                            rect,
                            cur_bottom_y,
                            "Packages",
                            PACKAGE,
                            &mut sidebar_state.packages_expanded,
                            text,
                            &mut action,
                            false,
                            Some(packages.len()),
                        );
                    }
                }
                cur_bottom_y += ROW_HEIGHT;
            }
        }
        SidebarTab::Search => {
            y += 12.0;
            let search_rect = egui::Rect::from_min_size(
                egui::pos2(rect.left() + 10.0, y),
                egui::vec2(rect.width() - 20.0, 26.0),
            );
            let text_edit = egui::TextEdit::singleline(&mut sidebar_state.search_query)
                .hint_text("Search commits...")
                .text_color(text);
            ui.put(search_rect, text_edit);
            y += 26.0 + 12.0;

            let scroll_rect =
                egui::Rect::from_min_max(egui::pos2(rect.left(), y), rect.right_bottom());
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("sidebar_search_scroll_host")
                    .max_rect(scroll_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    egui::ScrollArea::vertical()
                        .id_salt("sidebar_search_scroll")
                        .auto_shrink([false, false])
                        .show(ui, |ui| {
                            ui.add_space(10.0);
                            ui.horizontal(|ui| {
                                ui.add_space(12.0);
                                ui.label(
                                    egui::RichText::new("No searches").size(11.0).color(muted),
                                );
                            });
                        });
                },
            );
        }
        SidebarTab::FileTree => {
            y += 12.0;

            let head_hash = app_state
                .cached_commits
                .first()
                .map(|c| c.hash.clone())
                .unwrap_or_else(|| "empty".to_string());
            let repo_identity = git_repo
                .map(|repo| {
                    repo.workdir_path()
                        .unwrap_or_else(|| repo.git_dir_path())
                        .to_string_lossy()
                        .to_string()
                })
                .unwrap_or_else(|| "no-repo".to_string());
            let cached_head_key = format!("{}::{}", repo_identity, head_hash);

            if sidebar_state.cached_head_hash.as_deref() != Some(&cached_head_key) {
                if let Some(repo) = git_repo {
                    match repo.repo_files() {
                        Ok(files) => {
                            sidebar_state.cached_tracked_files =
                                files.into_iter().map(|f| f.path).collect();
                            sidebar_state.cached_head_hash = Some(cached_head_key);
                        }
                        Err(err) => {
                            tracing::warn!(err = %err, "Failed to load repo files for sidebar file tree");
                        }
                    }
                }
            }

            let mut items_map = std::collections::HashMap::new();
            for path in &sidebar_state.cached_tracked_files {
                items_map.insert(path.clone(), None);
            }

            if let Some(status) = &app_state.cached_status {
                for f in &status.unstaged_files {
                    use crate::state::CachedFileChangeKind;
                    match f.kind {
                        CachedFileChangeKind::Deleted => {
                            items_map.remove(&f.path);
                        }
                        CachedFileChangeKind::Added => {
                            items_map.insert(
                                f.path.clone(),
                                Some(crate::git::models::FileChangeKind::Added),
                            );
                        }
                        CachedFileChangeKind::Modified => {
                            items_map.insert(
                                f.path.clone(),
                                Some(crate::git::models::FileChangeKind::Modified),
                            );
                        }
                        CachedFileChangeKind::Renamed => {
                            if let Some(old_path) = &f.old_path {
                                items_map.remove(old_path);
                            }
                            items_map.insert(
                                f.path.clone(),
                                Some(crate::git::models::FileChangeKind::Renamed),
                            );
                        }
                        CachedFileChangeKind::TypeChanged => {
                            items_map.insert(
                                f.path.clone(),
                                Some(crate::git::models::FileChangeKind::TypeChanged),
                            );
                        }
                    }
                }

                for f in &status.staged_files {
                    use crate::state::CachedFileChangeKind;
                    match f.kind {
                        CachedFileChangeKind::Deleted => {
                            items_map.remove(&f.path);
                        }
                        CachedFileChangeKind::Added => {
                            items_map.insert(
                                f.path.clone(),
                                Some(crate::git::models::FileChangeKind::Added),
                            );
                        }
                        CachedFileChangeKind::Modified => {
                            items_map.insert(
                                f.path.clone(),
                                Some(crate::git::models::FileChangeKind::Modified),
                            );
                        }
                        CachedFileChangeKind::Renamed => {
                            if let Some(old_path) = &f.old_path {
                                items_map.remove(old_path);
                            }
                            items_map.insert(
                                f.path.clone(),
                                Some(crate::git::models::FileChangeKind::Renamed),
                            );
                        }
                        CachedFileChangeKind::TypeChanged => {
                            items_map.insert(
                                f.path.clone(),
                                Some(crate::git::models::FileChangeKind::TypeChanged),
                            );
                        }
                    }
                }
            }

            let mut tree_items: Vec<crate::ui::core::filetree::FileTreeItem> = items_map
                .into_iter()
                .map(
                    |(path, change_kind)| crate::ui::core::filetree::FileTreeItem {
                        path,
                        change_kind,
                    },
                )
                .collect();

            tree_items.sort_by(|a, b| {
                a.path.cmp(&b.path).then_with(|| {
                    let rank = |kind: &Option<crate::git::models::FileChangeKind>| match kind {
                        Some(crate::git::models::FileChangeKind::Added) => 0,
                        Some(crate::git::models::FileChangeKind::Modified) => 1,
                        Some(crate::git::models::FileChangeKind::Deleted) => 2,
                        Some(crate::git::models::FileChangeKind::Renamed) => 3,
                        Some(crate::git::models::FileChangeKind::TypeChanged) => 4,
                        None => 5,
                    };
                    rank(&a.change_kind).cmp(&rank(&b.change_kind))
                })
            });

            let rebuild_key = {
                let mut fingerprint = String::new();
                fingerprint.push_str(&repo_identity);
                fingerprint.push('|');
                for item in &tree_items {
                    fingerprint.push_str(&item.path);
                    fingerprint.push('|');
                    if let Some(kind) = &item.change_kind {
                        fingerprint.push_str(match kind {
                            crate::git::models::FileChangeKind::Added => "A",
                            crate::git::models::FileChangeKind::Modified => "M",
                            crate::git::models::FileChangeKind::Deleted => "D",
                            crate::git::models::FileChangeKind::Renamed => "R",
                            crate::git::models::FileChangeKind::TypeChanged => "T",
                        });
                    } else {
                        fingerprint.push('U');
                    }
                    fingerprint.push(';');
                }
                use std::hash::{Hash, Hasher};
                let mut hasher = std::collections::hash_map::DefaultHasher::new();
                fingerprint.hash(&mut hasher);
                format!("{:x}", hasher.finish())
            };

            let scroll_rect =
                egui::Rect::from_min_max(egui::pos2(rect.left() + 6.0, y), rect.right_bottom());
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("sidebar_filetree_scroll_host")
                    .max_rect(scroll_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    crate::ui::core::filetree::paint_tree_tab(
                        ui,
                        &mut sidebar_state.repo_tree_state,
                        &tree_items,
                        true, // populated
                        muted,
                        &rebuild_key,
                        "sidebar_filetree_scroll",
                    );
                },
            );
        }
    }

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
    trailing: NavRowTrailing,
) {
    let row = row_rect(rect, y, ROW_HEIGHT);
    if is_selected {
        ui.painter().rect_filled(row, 0.0, selected);
    }
    // Icon — aligned with section headers (24px from left, 13pt)
    painter_text(
        ui,
        egui::pos2(row.left() + 24.0, row.center().y),
        icon,
        13.0,
        text.linear_multiply(0.8),
        egui::Align2::CENTER_CENTER,
    );
    // Label — aligned with section headers (38px from left, 12pt)
    painter_text(
        ui,
        egui::pos2(row.left() + 38.0, row.center().y),
        label,
        12.0,
        text,
        egui::Align2::LEFT_CENTER,
    );

    // Trailing stats on the right
    let muted = text.linear_multiply(0.45);
    let green = egui::Color32::from_rgb(39, 174, 96);
    let red = egui::Color32::from_rgb(231, 76, 60);

    match trailing {
        NavRowTrailing::None => {}
        NavRowTrailing::DiffStats {
            files,
            additions,
            deletions,
        } => {
            if files > 0 {
                let mut cursor_x = row.right() - 8.0;
                // -deletions (red)
                if deletions > 0 {
                    let del_str = format!("-{}", deletions);
                    painter_bold_text(
                        ui,
                        egui::pos2(cursor_x, row.center().y),
                        &del_str,
                        9.5,
                        red.linear_multiply(0.8),
                        egui::Align2::RIGHT_CENTER,
                    );
                    let font_id = egui::FontId::proportional(9.5);
                    let galley = ui.painter().layout_no_wrap(del_str, font_id, red);
                    cursor_x -= galley.size().x + 5.0;
                }
                // +additions (green)
                if additions > 0 {
                    let add_str = format!("+{}", additions);
                    painter_bold_text(
                        ui,
                        egui::pos2(cursor_x, row.center().y),
                        &add_str,
                        9.5,
                        green.linear_multiply(0.8),
                        egui::Align2::RIGHT_CENTER,
                    );
                    let font_id = egui::FontId::proportional(9.5);
                    let galley = ui.painter().layout_no_wrap(add_str, font_id, green);
                    cursor_x -= galley.size().x + 5.0;
                }
                // file count
                let file_str = files.to_string();
                painter_text(
                    ui,
                    egui::pos2(cursor_x, row.center().y),
                    &file_str,
                    9.5,
                    muted,
                    egui::Align2::RIGHT_CENTER,
                );
            }
        }
        NavRowTrailing::Count(count) => {
            if count > 0 {
                let count_str = count.to_string();
                // Capsule badge (matching paint_section style)
                let font_id = egui::FontId::proportional(9.5);
                let galley = ui
                    .painter()
                    .layout_no_wrap(count_str.clone(), font_id, text);
                let text_width = galley.size().x;
                let badge_w = text_width + 8.0;
                let badge_h = 14.0;
                let badge_rect = egui::Rect::from_min_size(
                    egui::pos2(row.right() - 8.0 - badge_w, row.center().y - badge_h * 0.5),
                    egui::vec2(badge_w, badge_h),
                );
                let badge_bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12);
                ui.painter().rect_filled(badge_rect, 7.0, badge_bg);
                painter_text(
                    ui,
                    badge_rect.center(),
                    &count_str,
                    9.5,
                    muted,
                    egui::Align2::CENTER_CENTER,
                );
            }
        }
    }
}

enum NavRowTrailing {
    None,
    DiffStats {
        files: usize,
        additions: usize,
        deletions: usize,
    },
    Count(usize),
}

fn paint_mode_bar(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    y: f32,
    sidebar_state: &mut SidebarState,
    active_color: egui::Color32,
    muted_color: egui::Color32,
    stroke: egui::Stroke,
) {
    let row = row_rect(rect, y, 34.0);
    ui.painter()
        .line_segment([row.left_top(), row.right_top()], stroke);
    ui.painter()
        .line_segment([row.left_bottom(), row.right_bottom()], stroke);
    let third = row.width() / 3.0;

    let tab_rects = [
        egui::Rect::from_min_max(row.left_top(), egui::pos2(row.left() + third, row.bottom())),
        egui::Rect::from_min_max(
            egui::pos2(row.left() + third, row.top()),
            egui::pos2(row.left() + third * 2.0, row.bottom()),
        ),
        egui::Rect::from_min_max(
            egui::pos2(row.left() + third * 2.0, row.top()),
            row.right_bottom(),
        ),
    ];

    let tabs = [
        SidebarTab::Repository,
        SidebarTab::Search,
        SidebarTab::FileTree,
    ];
    let icons = [GIT_BRANCH, MAGNIFYING_GLASS, TREE_VIEW];

    for i in 0..3 {
        let tab_rect = tab_rects[i];
        let response = ui.interact(
            tab_rect,
            ui.make_persistent_id(("sidebar_tab_btn", i)),
            egui::Sense::click(),
        );

        if response.clicked() {
            sidebar_state.current_tab = tabs[i];
        }

        let is_active = sidebar_state.current_tab == tabs[i];
        let color = if is_active {
            active_color
        } else if response.hovered() {
            ui.visuals().text_color()
        } else {
            muted_color
        };

        painter_text(
            ui,
            egui::pos2(row.left() + third * (i as f32 + 0.5), row.center().y),
            icons[i],
            18.0,
            color,
            egui::Align2::CENTER_CENTER,
        );
    }
}

fn paint_filter(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    y: f32,
    sidebar_state: &mut SidebarState,
    text_color: egui::Color32,
    muted: egui::Color32,
    stroke: egui::Stroke,
) {
    let filter_rect = row_rect(rect, y, FILTER_HEIGHT).shrink2(egui::vec2(10.0, 2.0));

    // Draw background
    let bg_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8);
    ui.painter().rect_filled(filter_rect, 4.0, bg_color);
    ui.painter()
        .rect_stroke(filter_rect, 4.0, stroke, egui::StrokeKind::Inside);

    // Magnifying glass icon on the left
    let icon_x = filter_rect.left() + 14.0;
    painter_text(
        ui,
        egui::pos2(icon_x, filter_rect.center().y),
        MAGNIFYING_GLASS,
        12.0,
        muted,
        egui::Align2::CENTER_CENTER,
    );

    // Padding for the text edit
    let edit_min_x = filter_rect.left() + 24.0;
    let edit_max_x = filter_rect.right() - 24.0; // leave space for clear button or shortcut badge
    let edit_rect = egui::Rect::from_min_max(
        egui::pos2(edit_min_x, filter_rect.top() + 3.0),
        egui::pos2(edit_max_x, filter_rect.bottom() - 3.0),
    );

    let text_edit = egui::TextEdit::singleline(&mut sidebar_state.filter_query)
        .hint_text("Filter")
        .frame(egui::Frame::NONE)
        .text_color(text_color)
        .id(egui::Id::new("sidebar_filter_input"));

    let response = ui.put(edit_rect, text_edit);

    // Draw keyboard shortcut badge on the right if query is empty and not focused
    let show_shortcut = sidebar_state.filter_query.is_empty() && !response.has_focus();
    if show_shortcut {
        let badge_text = "Ctrl+Alt+F";
        let font_id = egui::FontId::proportional(8.0);
        let galley = ui.painter().layout_no_wrap(
            badge_text.to_string(),
            font_id,
            muted.linear_multiply(0.6),
        );
        let badge_w = galley.rect.width() + 6.0;
        let badge_h = 13.0;
        let badge_rect = egui::Rect::from_min_max(
            egui::pos2(
                filter_rect.right() - badge_w - 6.0,
                filter_rect.center().y - badge_h * 0.5,
            ),
            egui::pos2(
                filter_rect.right() - 6.0,
                filter_rect.center().y + badge_h * 0.5,
            ),
        );
        ui.painter().rect_filled(
            badge_rect,
            2.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12),
        );
        ui.painter().galley(
            egui::pos2(badge_rect.left() + 3.0, badge_rect.top() + 1.0),
            galley,
            muted.linear_multiply(0.6),
        );
    }

    // Draw clear button on the right if query is not empty
    if !sidebar_state.filter_query.is_empty() {
        let clear_btn_rect = egui::Rect::from_center_size(
            egui::pos2(filter_rect.right() - 14.0, filter_rect.center().y),
            egui::vec2(14.0, 14.0),
        );
        let clear_resp = ui.interact(
            clear_btn_rect,
            ui.make_persistent_id("sidebar_filter_clear_btn"),
            egui::Sense::click(),
        );
        if clear_resp.hovered() {
            ui.painter().rect_filled(
                clear_btn_rect,
                2.0,
                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12),
            );
        }
        painter_text(
            ui,
            clear_btn_rect.center(),
            "×",
            12.0,
            if clear_resp.hovered() {
                text_color
            } else {
                muted
            },
            egui::Align2::CENTER_CENTER,
        );
        if clear_resp.clicked() {
            sidebar_state.filter_query.clear();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn paint_section(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    y: f32,
    label: &str,
    icon: &str,
    expanded: &mut bool,
    text: egui::Color32,
    extra_action: &mut Option<SidebarAction>,
    is_fetching: bool,
    count: Option<usize>,
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

    if response.hovered() {
        ui.painter().rect_filled(
            row,
            4.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8),
        );
    }

    // Caret
    painter_text(
        ui,
        egui::pos2(row.left() + 10.0, row.center().y),
        caret,
        10.0,
        text.linear_multiply(0.6),
        egui::Align2::CENTER_CENTER,
    );

    // Section Icon
    painter_text(
        ui,
        egui::pos2(row.left() + 24.0, row.center().y),
        icon,
        13.0,
        text.linear_multiply(0.8),
        egui::Align2::CENTER_CENTER,
    );

    // Section Label (Smaller Font Size)
    let label_font = egui::FontId::proportional(13.0);
    let label_galley = ui
        .painter()
        .layout_no_wrap(label.to_string(), label_font.clone(), text);
    let label_width = label_galley.size().x;

    painter_text(
        ui,
        egui::pos2(row.left() + 38.0, row.center().y),
        label,
        13.0,
        text,
        egui::Align2::LEFT_CENTER,
    );

    // Muted premium count capsule badge
    if let Some(count) = count {
        let count_str = count.to_string();
        let font_id = egui::FontId::proportional(10.0);
        let galley = ui
            .painter()
            .layout_no_wrap(count_str.clone(), font_id.clone(), text);
        let text_width = galley.size().x;

        let badge_w = text_width + 8.0;
        let badge_h = 14.0;
        let badge_rect = egui::Rect::from_min_size(
            egui::pos2(
                row.left() + 38.0 + label_width + 8.0,
                row.center().y - badge_h * 0.5,
            ),
            egui::vec2(badge_w, badge_h),
        );

        let badge_bg = egui::Color32::from_rgba_unmultiplied(255, 255, 255, 12);
        ui.painter().rect_filled(badge_rect, 7.0, badge_bg);

        let muted_color = text.linear_multiply(0.5);
        painter_text(
            ui,
            badge_rect.center(),
            &count_str,
            10.0,
            muted_color,
            egui::Align2::CENTER_CENTER,
        );
    }

    // Fetch spinner / Refetch button for Remotes and Actions
    if label == "Remotes" || label == "Actions" {
        if is_fetching {
            let spinner_rect = egui::Rect::from_min_size(
                egui::pos2(row.right() - 24.0, row.center().y - 6.0),
                egui::vec2(12.0, 12.0),
            );
            ui.put(spinner_rect, egui::Spinner::new().size(12.0));
        } else {
            let button_rect = egui::Rect::from_min_size(
                egui::pos2(row.right() - 24.0, row.center().y - 8.0),
                egui::vec2(16.0, 16.0),
            );
            let button_id = ui.make_persistent_id(("app_sidebar_refetch", label));
            let button_resp = ui.interact(button_rect, button_id, egui::Sense::click());

            let icon_color = if button_resp.hovered() {
                ui.visuals().widgets.hovered.text_color()
            } else {
                text.linear_multiply(0.5)
            };

            painter_text(
                ui,
                button_rect.center(),
                ARROW_COUNTER_CLOCKWISE,
                11.0,
                icon_color,
                egui::Align2::CENTER_CENTER,
            );

            if button_resp.clicked() {
                if label == "Actions" {
                    *extra_action = Some(SidebarAction::RefreshActions);
                } else {
                    *extra_action = Some(SidebarAction::Fetch);
                }
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrailingStyle {
    None,
    Stash,
    Package,
}

#[allow(clippy::too_many_arguments)]
fn paint_tree_row(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    _indent: usize,
    icon: &str,
    label: &str,
    strong: bool,
    text: egui::Color32,
    muted: egui::Color32,
    trailing_label: Option<&str>,
    trailing_style: TrailingStyle,
    id_salt: &str,
    bg_gradient_color: Option<egui::Color32>,
    icon_color: Option<egui::Color32>,
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
            4.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8),
        );
    }

    let left = row.left();

    // Child Icon (indented, smaller font size)
    painter_text(
        ui,
        egui::pos2(left + 34.0, row.center().y),
        icon,
        12.0,
        icon_color.unwrap_or(if strong { text } else { muted }),
        egui::Align2::CENTER_CENTER,
    );

    // Child Label (indented, smaller font size)
    painter_text(
        ui,
        egui::pos2(left + 48.0, row.center().y),
        label,
        12.0,
        if strong { text } else { muted },
        egui::Align2::LEFT_CENTER,
    );

    match trailing_style {
        TrailingStyle::None => {}
        TrailingStyle::Stash => {
            if let Some(label_str) = trailing_label {
                painter_text(
                    ui,
                    egui::pos2(row.right() - 54.0, row.center().y),
                    label_str,
                    12.0,
                    muted,
                    egui::Align2::CENTER_CENTER,
                );
                painter_text(
                    ui,
                    egui::pos2(row.right() - 34.0, row.center().y),
                    FUNNEL,
                    12.0,
                    muted,
                    egui::Align2::CENTER_CENTER,
                );
                painter_text(
                    ui,
                    egui::pos2(row.right() - 15.0, row.center().y),
                    EYE,
                    12.0,
                    muted,
                    egui::Align2::CENTER_CENTER,
                );
            }
        }
        TrailingStyle::Package => {
            if let Some(label_str) = trailing_label {
                painter_text(
                    ui,
                    egui::pos2(row.right() - 54.0, row.center().y),
                    label_str,
                    12.0,
                    muted,
                    egui::Align2::CENTER_CENTER,
                );
                painter_text(
                    ui,
                    egui::pos2(row.right() - 34.0, row.center().y),
                    FUNNEL,
                    12.0,
                    muted,
                    egui::Align2::CENTER_CENTER,
                );
                painter_text(
                    ui,
                    egui::pos2(row.right() - 15.0, row.center().y),
                    EYE,
                    12.0,
                    muted,
                    egui::Align2::CENTER_CENTER,
                );
            }
        }
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

fn painter_bold_text(
    ui: &egui::Ui,
    pos: egui::Pos2,
    text: &str,
    size: f32,
    color: egui::Color32,
    align: egui::Align2,
) {
    let rich_text = egui::RichText::new(text).strong().size(size).color(color);
    let widget_text = egui::WidgetText::from(rich_text);
    let galley = widget_text.into_galley(ui, None, f32::INFINITY, egui::TextStyle::Body);
    let rect = galley.rect;
    let offset_x = match align.x() {
        egui::Align::Min => 0.0,
        egui::Align::Center => rect.width() * 0.5,
        egui::Align::Max => rect.width(),
    };
    let offset_y = match align.y() {
        egui::Align::Min => 0.0,
        egui::Align::Center => rect.height() * 0.5,
        egui::Align::Max => rect.height(),
    };
    let top_left = egui::pos2(pos.x - offset_x, pos.y - offset_y);
    ui.painter().galley(top_left, galley, color);
}

fn parse_github_owner_from_url(url: &str) -> Option<String> {
    if url.contains("github.com") {
        if let Some(pos) = url.find("github.com/") {
            let path = &url[pos + "github.com/".len()..];
            let parts: Vec<&str> = path.split('/').collect();
            if !parts.is_empty() && !parts[0].is_empty() {
                return Some(parts[0].to_string());
            }
        } else if let Some(pos) = url.find("github.com:") {
            let path = &url[pos + "github.com:".len()..];
            let parts: Vec<&str> = path.split('/').collect();
            if !parts.is_empty() && !parts[0].is_empty() {
                return Some(parts[0].to_string());
            }
        }
    }
    None
}

struct RemoteGroupRowArgs<'a> {
    rect: egui::Rect,
    y: f32,
    label: &'a str,
    expanded: bool,
    text: egui::Color32,
    muted: egui::Color32,
    id_salt: &'a str,
    avatar_path_or_url: Option<&'a str>,
}

fn paint_remote_group_row(ui: &mut egui::Ui, args: RemoteGroupRowArgs<'_>) -> egui::Response {
    let row = row_rect(args.rect, args.y, ROW_HEIGHT);
    let response = ui.interact(
        row,
        ui.make_persistent_id(("app_sidebar_remote_group", args.id_salt, args.label)),
        egui::Sense::click(),
    );

    if response.hovered() {
        ui.painter().rect_filled(
            row,
            4.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8),
        );
    }

    let left = row.left();
    let caret = if args.expanded {
        CARET_DOWN
    } else {
        CARET_RIGHT
    };

    // Caret
    painter_text(
        ui,
        egui::pos2(left + 24.0, row.center().y),
        caret,
        10.0,
        args.text.linear_multiply(0.6),
        egui::Align2::CENTER_CENTER,
    );

    // Avatar or Globe Icon
    if let Some(path_or_url) = args.avatar_path_or_url {
        let uri = if path_or_url.starts_with("http://") || path_or_url.starts_with("https://") {
            path_or_url.to_string()
        } else {
            url::Url::from_file_path(path_or_url)
                .map(|u| u.to_string())
                .unwrap_or_else(|_| format!("file://{}", path_or_url))
        };
        let image_rect = egui::Rect::from_center_size(
            egui::pos2(left + 38.0, row.center().y),
            egui::vec2(12.0, 12.0),
        );
        ui.put(image_rect, egui::Image::new(uri).corner_radius(2.0));
    } else {
        painter_text(
            ui,
            egui::pos2(left + 38.0, row.center().y),
            CLOUD,
            12.0,
            args.muted,
            egui::Align2::CENTER_CENTER,
        );
    }

    // Label
    painter_text(
        ui,
        egui::pos2(left + 50.0, row.center().y),
        args.label,
        12.0,
        args.text,
        egui::Align2::LEFT_CENTER,
    );

    response
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

fn parse_iso8601_to_unix(s: &str) -> Option<i64> {
    if s.len() < 19 {
        return None;
    }
    let year = s[0..4].parse::<i64>().ok()?;
    let month = s[5..7].parse::<i64>().ok()?;
    let day = s[8..10].parse::<i64>().ok()?;
    let hour = s[11..13].parse::<i64>().ok()?;
    let min = s[14..16].parse::<i64>().ok()?;
    let sec = s[17..19].parse::<i64>().ok()?;

    let days_in_month = [0, 31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    let mut days = 0;
    for y in 1970..year {
        days += if y % 4 == 0 && (y % 100 != 0 || y % 400 == 0) {
            366
        } else {
            365
        };
    }
    for m in 1..month {
        days += days_in_month[m as usize];
        if m == 2 && (year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)) {
            days += 1;
        }
    }
    days += day - 1;

    Some(days * 86400 + hour * 3600 + min * 60 + sec)
}

fn format_duration(secs: i64) -> String {
    if secs < 0 {
        return "0s".to_string();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{}h {}m {}s", h, m, s)
    } else if m > 0 {
        format!("{}m {}s", m, s)
    } else {
        format!("{}s", s)
    }
}

fn days_since_epoch_to_date(days: i64) -> String {
    let mut days = days;
    let mut year = 1970;
    loop {
        let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        let year_days = if leap { 366 } else { 365 };
        if days >= year_days {
            days -= year_days;
            year += 1;
        } else {
            break;
        }
    }

    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 1;
    for &dim in &days_in_month {
        if days >= dim {
            days -= dim;
            month += 1;
        } else {
            break;
        }
    }
    let day = days + 1;
    format!("{:04}-{:02}-{:02}", year, month, day)
}

fn format_run_time(iso_str: &str) -> String {
    if iso_str.len() >= 16 {
        let year_month_day = &iso_str[0..10];
        let hour_str = &iso_str[11..13];
        let min_str = &iso_str[14..16];

        if let (Ok(h), Ok(m)) = (hour_str.parse::<u32>(), min_str.parse::<u32>()) {
            let am_pm = if h >= 12 { "PM" } else { "AM" };
            let display_hour = if h == 0 {
                12
            } else if h > 12 {
                h - 12
            } else {
                h
            };

            if let Some(now_secs) = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .ok()
                .map(|d| d.as_secs() as i64)
            {
                let days_since_epoch = now_secs / 86400;
                let today_str = days_since_epoch_to_date(days_since_epoch);
                let yesterday_str = days_since_epoch_to_date(days_since_epoch - 1);

                if year_month_day == today_str {
                    return format!("Today at {:02}:{:02} {}", display_hour, m, am_pm);
                } else if year_month_day == yesterday_str {
                    return format!("Yesterday at {:02}:{:02} {}", display_hour, m, am_pm);
                }
            }

            return format!(
                "{} at {:02}:{:02} {}",
                year_month_day, display_hour, m, am_pm
            );
        }
    }
    iso_str.to_string()
}

#[allow(clippy::too_many_arguments)]
fn paint_action_row(
    ui: &egui::Ui,
    rect: egui::Rect,
    y: f32,
    icon: &str,
    name: &str,
    run_number: u32,
    status: &str,
    actor_login: &str,
    created_at: &str,
    updated_at: &str,
    conclusion: Option<&str>,
    head_branch: &str,
    branch_color: egui::Color32,
    text: egui::Color32,
    muted: egui::Color32,
    avatar_cache: &std::collections::HashMap<String, String>,
    id_salt: &str,
    icon_color: egui::Color32,
) -> egui::Response {
    let row_height = 40.0;
    let row = row_rect(rect, y, row_height);
    let response = ui.interact(
        row,
        ui.make_persistent_id(("app_sidebar_action_row", id_salt, name)),
        egui::Sense::click(),
    );

    if response.hovered() {
        ui.painter().rect_filled(
            row,
            4.0,
            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 8),
        );
    }

    let left = row.left();
    let center_y = row.center().y;

    // Subrow 1: center_y - 8.0
    let subrow1_y = center_y - 8.0;

    // Icon
    painter_text(
        ui,
        egui::pos2(left + 34.0, subrow1_y),
        icon,
        12.0,
        icon_color,
        egui::Align2::CENTER_CENTER,
    );

    // Branch tag / badge
    let tag_text_width = ui
        .painter()
        .layout_no_wrap(
            head_branch.to_string(),
            egui::FontId::proportional(10.0),
            egui::Color32::PLACEHOLDER,
        )
        .rect
        .width();

    let tag_height = 14.0;
    let badge_left = row.right() - 30.0 - tag_text_width - 8.0;
    let tag_rect = egui::Rect::from_min_max(
        egui::pos2(badge_left, subrow1_y - tag_height / 2.0),
        egui::pos2(row.right() - 30.0, subrow1_y + tag_height / 2.0),
    );

    let bg_color = branch_color.linear_multiply(0.15);
    let stroke_color = branch_color.linear_multiply(0.6);
    ui.painter().rect(
        tag_rect,
        3.0,
        bg_color,
        egui::Stroke::new(1.0_f32, stroke_color),
        egui::StrokeKind::Inside,
    );

    ui.painter().text(
        tag_rect.center(),
        egui::Align2::CENTER_CENTER,
        head_branch,
        egui::FontId::proportional(10.0),
        branch_color,
    );

    // Heading and Completed Label Dynamic Truncation
    let heading_start_x = left + 48.0;
    let max_total_width = badge_left - heading_start_x - 6.0;

    let raw_heading = format!("{} #{}", name, run_number);
    let raw_heading_width = ui
        .painter()
        .layout_no_wrap(
            raw_heading.clone(),
            egui::FontId::proportional(12.0),
            egui::Color32::PLACEHOLDER,
        )
        .rect
        .width();

    let (display_heading, display_completed, display_heading_width) =
        if raw_heading_width > max_total_width {
            // Truncate name so that "truncated_name... #run_number" fits
            let mut truncated_name = name.to_string();
            let mut final_heading = raw_heading.clone();
            let mut final_width = raw_heading_width;
            while truncated_name.len() > 3 {
                truncated_name.pop();
                let test_heading = format!("{}... #{}", truncated_name, run_number);
                let test_width = ui
                    .painter()
                    .layout_no_wrap(
                        test_heading.clone(),
                        egui::FontId::proportional(12.0),
                        egui::Color32::PLACEHOLDER,
                    )
                    .rect
                    .width();
                if test_width <= max_total_width {
                    final_heading = test_heading;
                    final_width = test_width;
                    break;
                }
            }
            (final_heading, "".to_string(), final_width)
        } else {
            // Heading fits completely. Now check completed_label.
            let completed_label = if actor_login.is_empty() {
                "".to_string()
            } else {
                format!(": completed by {}", actor_login)
            };

            if completed_label.is_empty() {
                (raw_heading, "".to_string(), raw_heading_width)
            } else {
                let label_start_x = heading_start_x + raw_heading_width + 4.0;
                let max_label_width = badge_left - label_start_x - 6.0;
                if max_label_width <= 0.0 {
                    (raw_heading, "".to_string(), raw_heading_width)
                } else {
                    let label_width = ui
                        .painter()
                        .layout_no_wrap(
                            completed_label.clone(),
                            egui::FontId::proportional(12.0),
                            egui::Color32::PLACEHOLDER,
                        )
                        .rect
                        .width();

                    if label_width > max_label_width {
                        let mut truncated = completed_label.clone();
                        let mut final_label = "".to_string();
                        while truncated.len() > 3 {
                            truncated.pop();
                            let test_str = format!("{}...", truncated);
                            let test_width = ui
                                .painter()
                                .layout_no_wrap(
                                    test_str.clone(),
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::PLACEHOLDER,
                                )
                                .rect
                                .width();
                            if test_width <= max_label_width {
                                final_label = test_str;
                                break;
                            }
                        }
                        (raw_heading, final_label, raw_heading_width)
                    } else {
                        (raw_heading, completed_label, raw_heading_width)
                    }
                }
            }
        };

    // Paint Heading
    painter_text(
        ui,
        egui::pos2(heading_start_x, subrow1_y),
        &display_heading,
        12.0,
        text,
        egui::Align2::LEFT_CENTER,
    );

    // Paint Completed Label
    if !display_completed.is_empty() {
        painter_text(
            ui,
            egui::pos2(heading_start_x + display_heading_width + 4.0, subrow1_y),
            &display_completed,
            12.0,
            muted,
            egui::Align2::LEFT_CENTER,
        );
    }

    // Skipped Strikethrough
    if conclusion == Some("skipped") {
        let line_y = subrow1_y;
        ui.painter().line_segment(
            [
                egui::pos2(heading_start_x, line_y),
                egui::pos2(heading_start_x + display_heading_width, line_y),
            ],
            egui::Stroke::new(1.0_f32, text.linear_multiply(0.6)),
        );

        if !display_completed.is_empty() {
            let label_start_x = heading_start_x + display_heading_width + 4.0;
            let display_completed_width = ui
                .painter()
                .layout_no_wrap(
                    display_completed.clone(),
                    egui::FontId::proportional(12.0),
                    egui::Color32::PLACEHOLDER,
                )
                .rect
                .width();
            ui.painter().line_segment(
                [
                    egui::pos2(label_start_x, line_y),
                    egui::pos2(label_start_x + display_completed_width, line_y),
                ],
                egui::Stroke::new(1.0_f32, muted.linear_multiply(0.6)),
            );
        }
    }

    // Link icon
    painter_text(
        ui,
        egui::pos2(row.right() - 15.0, subrow1_y),
        ARROW_UP_RIGHT,
        12.0,
        muted,
        egui::Align2::CENTER_CENTER,
    );

    // Subrow 2: center_y + 8.0
    let is_completed = conclusion.is_some();
    let duration_secs = if is_completed {
        let created_unix = parse_iso8601_to_unix(created_at).unwrap_or(0);
        let updated_unix = parse_iso8601_to_unix(updated_at).unwrap_or(0);
        (updated_unix - created_unix).max(0)
    } else {
        let created_unix = parse_iso8601_to_unix(created_at).unwrap_or(0);
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs() as i64)
            .unwrap_or(created_unix);
        (now_secs - created_unix).max(0)
    };
    let duration_str = format_duration(duration_secs);
    let time_str = format_run_time(created_at);
    let subtext = if created_at.is_empty() {
        "Unknown time".to_string()
    } else {
        format!("{}  •  {}", time_str, duration_str)
    };

    painter_text(
        ui,
        egui::pos2(left + 48.0, center_y + 8.0),
        &subtext,
        10.0,
        muted,
        egui::Align2::LEFT_CENTER,
    );

    response.on_hover_ui(|ui| {
        ui.set_max_width(320.0);
        ui.vertical(|ui| {
            // Row 1: Status details in fluent label
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    egui::RichText::new("Status:")
                        .color(muted)
                        .font(egui::FontId::proportional(11.0)),
                );

                let display_status = if let Some(conc) = conclusion {
                    format!("completed ({})", conc)
                } else {
                    status.to_string()
                };
                let status_color = match conclusion {
                    Some("success") => egui::Color32::from_rgb(39, 174, 96),
                    Some("skipped") => egui::Color32::from_rgb(120, 120, 120),
                    Some("failure")
                    | Some("cancelled")
                    | Some("timed_out")
                    | Some("action_required") => egui::Color32::from_rgb(231, 76, 60),
                    _ => egui::Color32::from_rgb(241, 196, 15),
                };

                ui.label(
                    egui::RichText::new(display_status)
                        .strong()
                        .color(status_color)
                        .font(egui::FontId::proportional(11.0)),
                );
            });
            ui.add_space(2.0);
            ui.separator();
            ui.add_space(4.0);

            // Row 2: Fluent action label & took y time to complete
            let full_label = if actor_login.is_empty() {
                format!("{} #{}", name, run_number)
            } else {
                format!("{} #{} : completed by {}", name, run_number, actor_login)
            };

            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                // Status/Function Icon
                ui.label(
                    egui::RichText::new(icon)
                        .color(icon_color)
                        .font(egui::FontId::proportional(11.0)),
                );

                // Full Label
                let label_text = egui::RichText::new(&full_label)
                    .color(text)
                    .font(egui::FontId::proportional(11.0));
                let label_text = if conclusion == Some("skipped") {
                    label_text.strikethrough()
                } else {
                    label_text
                };
                ui.label(label_text);

                // Fluent suffix details
                if conclusion == Some("skipped") {
                    let suffix_txt = egui::RichText::new("was skipped")
                        .color(muted)
                        .font(egui::FontId::proportional(11.0));
                    let suffix_txt = suffix_txt.strikethrough();
                    ui.label(suffix_txt);
                } else if conclusion.is_none() {
                    ui.label(
                        egui::RichText::new("is running for")
                            .color(muted)
                            .font(egui::FontId::proportional(11.0)),
                    );
                    ui.label(
                        egui::RichText::new(CLOCK)
                            .color(muted)
                            .font(egui::FontId::proportional(11.0)),
                    );
                    ui.label(
                        egui::RichText::new(duration_str)
                            .color(text)
                            .font(egui::FontId::proportional(11.0)),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("took")
                            .color(muted)
                            .font(egui::FontId::proportional(11.0)),
                    );
                    ui.label(
                        egui::RichText::new(CLOCK)
                            .color(muted)
                            .font(egui::FontId::proportional(11.0)),
                    );
                    ui.label(
                        egui::RichText::new(duration_str)
                            .color(text)
                            .font(egui::FontId::proportional(11.0)),
                    );
                    ui.label(
                        egui::RichText::new("to complete")
                            .color(muted)
                            .font(egui::FontId::proportional(11.0)),
                    );
                }
            });
            ui.add_space(2.0);

            // Row 3: Branch & Author (with avatar)
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;

                if !head_branch.is_empty() {
                    ui.label(
                        egui::RichText::new("Branch:")
                            .color(muted)
                            .font(egui::FontId::proportional(10.0)),
                    );
                    ui.label(
                        egui::RichText::new(GIT_BRANCH)
                            .color(branch_color)
                            .font(egui::FontId::proportional(10.0)),
                    );

                    let branch_txt = egui::RichText::new(head_branch)
                        .color(text)
                        .font(egui::FontId::proportional(10.0));
                    let branch_txt = if conclusion == Some("skipped") {
                        branch_txt.strikethrough()
                    } else {
                        branch_txt
                    };
                    ui.label(branch_txt);
                }

                if !actor_login.is_empty() {
                    if !head_branch.is_empty() {
                        ui.label(
                            egui::RichText::new("•")
                                .color(muted)
                                .font(egui::FontId::proportional(10.0)),
                        );
                    }
                    ui.label(
                        egui::RichText::new("Author:")
                            .color(muted)
                            .font(egui::FontId::proportional(10.0)),
                    );

                    // Render avatar image if cached
                    if let Some(path) = avatar_cache.get(actor_login) {
                        let uri = url::Url::from_file_path(path)
                            .map(|u| u.to_string())
                            .unwrap_or_else(|_| format!("file://{}", path));
                        let image = egui::Image::new(uri).corner_radius(2.0);
                        ui.add(image.max_width(12.0).max_height(12.0));
                    }

                    let actor_txt = egui::RichText::new(format!("@{}", actor_login))
                        .color(text)
                        .font(egui::FontId::proportional(10.0));
                    let actor_txt = if conclusion == Some("skipped") {
                        actor_txt.strikethrough()
                    } else {
                        actor_txt
                    };
                    ui.label(actor_txt);
                }
            });

            // Row 4: Created Time
            if !created_at.is_empty() {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 4.0;
                    ui.label(
                        egui::RichText::new("Created:")
                            .color(muted)
                            .font(egui::FontId::proportional(10.0)),
                    );
                    let time_txt = egui::RichText::new(format_run_time(created_at))
                        .color(text)
                        .font(egui::FontId::proportional(10.0));
                    let time_txt = if conclusion == Some("skipped") {
                        time_txt.strikethrough()
                    } else {
                        time_txt
                    };
                    ui.label(time_txt);
                });
            }
        });
    })
}
