use eframe::egui;
use egui_phosphor::regular::{
    ARROW_SQUARE_IN, ARROW_SQUARE_OUT, FILE, FILE_PLUS, FILE_TEXT, FOLDER, X,
};

use crate::cdv;
use crate::git::models::{FileChangeKind, FileStatus};
use crate::state::AppState;

#[derive(Clone, Debug)]
pub struct CommitDrawerCommit {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub email: String,
    pub timestamp: String,
    pub timestamp_exact: String,
    pub parents: Vec<String>,
    pub populated: bool,
}

#[derive(Clone, Debug, Default)]
pub struct CommitDrawerSignature {
    pub status: String,
    pub summary: Option<String>,
    pub key_id: Option<String>,
    pub trust: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum CommitDrawerTab {
    #[default]
    Commit,
    Changes,
    FileTree,
}

pub struct State {
    pub tab: CommitDrawerTab,
    pub tree_state: crate::ui::core::filetree::TreeState,
    diff_state: cdv::DiffTimelineState,
    pub cached_tree_items: Vec<crate::ui::core::filetree::FileTreeItem>,
    pub last_rebuild_key: Option<String>,
    pub height: f32,
    pub detached: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: CommitDrawerTab::Commit,
            tree_state: crate::ui::core::filetree::TreeState::default(),
            diff_state: cdv::DiffTimelineState::default(),
            cached_tree_items: Vec::new(),
            last_rebuild_key: None,
            height: 240.0,
            detached: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitDrawerResponse {
    None,
    Close,
    Detach,
    Attach,
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    state: &mut State,
    app_state: &AppState,
    commit: Option<&CommitDrawerCommit>,
    signature: Option<&CommitDrawerSignature>,
    files: &[FileStatus],
    diff: Option<&cdv::CommitDiffViewModel>,
    vertical: bool,
) -> CommitDrawerResponse {
    let fill = egui::Color32::from_rgb(36, 36, 36);
    let header_fill = egui::Color32::from_rgb(44, 44, 44);
    let muted = egui::Color32::from_rgb(172, 172, 172);

    let panel_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top()),
        egui::pos2(rect.right(), rect.bottom()),
    );

    ui.painter().rect_filled(panel_rect, 0.0, fill);

    let header_height = 34.0;
    let header_rect = egui::Rect::from_min_size(
        panel_rect.left_top(),
        egui::vec2(panel_rect.width(), header_height),
    );
    ui.painter().rect_filled(header_rect, 0.0, header_fill);

    if !state.detached {
        if vertical {
            let resize_grip_width = 8.0;
            let resize_grip_rect = egui::Rect::from_min_size(
                panel_rect.left_top(),
                egui::vec2(resize_grip_width, panel_rect.height()),
            );
            let resize_response = ui.interact(
                resize_grip_rect,
                ui.make_persistent_id("commit_drawer_resize"),
                egui::Sense::drag(),
            );
            if resize_response.dragged() {
                state.height = (state.height - resize_response.drag_delta().x).clamp(200.0, 700.0);
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
            if resize_response.hovered() || resize_response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
            }
        } else {
            let resize_grip_height = 8.0;
            let resize_grip_rect = egui::Rect::from_min_size(
                panel_rect.left_top(),
                egui::vec2(panel_rect.width(), resize_grip_height),
            );
            let resize_response = ui.interact(
                resize_grip_rect,
                ui.make_persistent_id("commit_drawer_resize"),
                egui::Sense::drag(),
            );
            if resize_response.dragged() {
                state.height = (state.height - resize_response.drag_delta().y).clamp(140.0, 520.0);
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            if resize_response.hovered() || resize_response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
        }
    }

    let mut action = CommitDrawerResponse::None;

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("commit_drawer_header")
            .max_rect(header_rect.shrink2(egui::vec2(12.0, 6.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.label(egui::RichText::new("Commit").strong());
            ui.add_space(12.0);
            tab_button(ui, state, CommitDrawerTab::Commit, "Commit");
            tab_button(ui, state, CommitDrawerTab::Changes, "Changes");
            tab_button(ui, state, CommitDrawerTab::FileTree, "File Tree");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(egui::Button::new(egui::RichText::new(X).size(12.0)).frame(false))
                    .on_hover_text("Close drawer")
                    .clicked()
                {
                    action = CommitDrawerResponse::Close;
                }
                ui.add_space(6.0);
                if state.detached {
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new(ARROW_SQUARE_IN).size(12.0))
                                .frame(false),
                        )
                        .on_hover_text("Attach drawer to main window")
                        .clicked()
                    {
                        action = CommitDrawerResponse::Attach;
                    }
                } else {
                    if ui
                        .add(
                            egui::Button::new(egui::RichText::new(ARROW_SQUARE_OUT).size(12.0))
                                .frame(false),
                        )
                        .on_hover_text("Detach drawer to new window")
                        .clicked()
                    {
                        action = CommitDrawerResponse::Detach;
                    }
                }
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(format!("{} commits", app_state.cached_commits.len()))
                        .size(10.0)
                        .color(muted),
                );
            });
        },
    );

    let content_rect = egui::Rect::from_min_max(
        egui::pos2(panel_rect.left(), header_rect.bottom()),
        egui::pos2(panel_rect.right(), panel_rect.bottom()),
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("commit_drawer_content")
            .max_rect(content_rect.shrink2(egui::vec2(12.0, 10.0)))
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            if let Some(commit) = commit {
                paint_commit_summary(ui, commit, muted);
                ui.add_space(12.0);
                match state.tab {
                    CommitDrawerTab::Commit => {
                        paint_commit_tab(ui, commit, signature, files, app_state, muted)
                    }
                    CommitDrawerTab::Changes => paint_changes_tab(
                        ui,
                        &mut state.diff_state,
                        files,
                        commit.populated,
                        diff,
                        muted,
                    ),
                    CommitDrawerTab::FileTree => {
                        let rebuild_key = &commit.hash;
                        if state.last_rebuild_key.as_deref() != Some(rebuild_key) {
                            state.cached_tree_items = files
                                .iter()
                                .map(|f| crate::ui::core::filetree::FileTreeItem {
                                    path: f.path.clone(),
                                    change_kind: Some(f.kind.clone()),
                                })
                                .collect();
                            state.last_rebuild_key = Some(rebuild_key.to_string());
                        }
                        crate::ui::core::filetree::paint_tree_tab(
                            ui,
                            &mut state.tree_state,
                            &state.cached_tree_items,
                            commit.populated,
                            muted,
                            rebuild_key,
                            "commit_drawer_tree_scroll",
                        );
                    }
                }
            } else {
                ui.label(
                    egui::RichText::new("Select a commit to view details")
                        .size(12.0)
                        .color(muted),
                );
            }
        },
    );

    action
}

fn tab_button(ui: &mut egui::Ui, state: &mut State, tab: CommitDrawerTab, label: &str) {
    let selected = state.tab == tab;
    let old_bg_fill = ui.visuals().selection.bg_fill;
    ui.visuals_mut().selection.bg_fill = egui::Color32::from_rgb(62, 62, 62);
    let response = ui.selectable_label(selected, label);
    ui.visuals_mut().selection.bg_fill = old_bg_fill;
    if response.clicked() {
        state.tab = tab;
    }
}

fn paint_commit_summary(ui: &mut egui::Ui, commit: &CommitDrawerCommit, muted: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&commit.message).size(14.0).strong());
    });
    let email_part = if commit.populated {
        format!(" <{}>", commit.email)
    } else {
        "".to_string()
    };
    ui.label(
        egui::RichText::new(format!("{}{}", commit.author, email_part))
            .size(10.0)
            .color(muted),
    );
    ui.label(
        egui::RichText::new(format!("{}  {}", commit.timestamp, commit.short_hash))
            .size(10.0)
            .color(muted),
    );
}

fn paint_commit_tab(
    ui: &mut egui::Ui,
    commit: &CommitDrawerCommit,
    signature: Option<&CommitDrawerSignature>,
    files: &[FileStatus],
    app_state: &AppState,
    muted: egui::Color32,
) {
    paint_commit_details(ui, commit, signature, files, app_state, muted);
}

fn paint_changes_tab(
    ui: &mut egui::Ui,
    diff_state: &mut cdv::DiffTimelineState,
    files: &[FileStatus],
    populated: bool,
    diff: Option<&cdv::CommitDiffViewModel>,
    muted: egui::Color32,
) {
    if !populated {
        ui.label(
            egui::RichText::new("Loading changes...")
                .size(10.0)
                .color(muted),
        );
        return;
    }
    if let Some(diff) = diff {
        cdv::show(
            ui,
            diff_state,
            Some(diff),
            Some(&mut |ui, rect, path, _kind, color| {
                crate::ui::core::filetree::paint_file_icon_rect(ui, rect, path, color);
            }),
        );
    } else {
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{} files changed", files.len()))
                    .size(10.0)
                    .color(muted),
            );
            let (additions, deletions) = file_totals(files);
            ui.label(
                egui::RichText::new(format!("+{}", additions))
                    .size(10.0)
                    .color(egui::Color32::from_rgb(78, 190, 116)),
            );
            ui.label(
                egui::RichText::new(format!("-{}", deletions))
                    .size(10.0)
                    .color(egui::Color32::from_rgb(230, 92, 92)),
            );
        });
        ui.add_space(8.0);
        if files.is_empty() {
            ui.label(
                egui::RichText::new("No file changes")
                    .size(10.0)
                    .color(muted),
            );
        } else {
            paint_changes_list(ui, files, muted);
        }
    }
}

fn paint_commit_details(
    ui: &mut egui::Ui,
    commit: &CommitDrawerCommit,
    signature: Option<&CommitDrawerSignature>,
    files: &[FileStatus],
    app_state: &AppState,
    muted: egui::Color32,
) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                paint_author_avatar(ui, &commit.author, app_state);
                ui.vertical(|ui| {
                    ui.label(egui::RichText::new("Details").size(11.0).strong());
                    let email_part = if commit.populated {
                        format!(" <{}>", commit.email)
                    } else {
                        " <loading...>".to_string()
                    };
                    ui.label(
                        egui::RichText::new(format!("{}{}", commit.author, email_part))
                            .size(10.0)
                            .color(muted),
                    );
                });
            });
            ui.label(
                egui::RichText::new(format!("SHA: {}", commit.hash))
                    .size(10.0)
                    .color(muted),
            );
            ui.label(
                egui::RichText::new(format!("Parents: {}", commit.parents.join(", ")))
                    .size(10.0)
                    .color(muted),
            );
            ui.label(
                egui::RichText::new(format!("When: {}", commit.timestamp_exact))
                    .size(10.0)
                    .color(muted),
            );
            if commit.populated {
                ui.label(
                    egui::RichText::new(format!("Files changed: {}", files.len()))
                        .size(10.0)
                        .color(muted),
                );
                let (additions, deletions) = file_totals(files);
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("Total diff:").size(10.0).color(muted));
                    ui.label(
                        egui::RichText::new(format!("+{}", additions))
                            .size(10.0)
                            .color(egui::Color32::from_rgb(78, 190, 116)),
                    );
                    ui.label(
                        egui::RichText::new(format!("-{}", deletions))
                            .size(10.0)
                            .color(egui::Color32::from_rgb(230, 92, 92)),
                    );
                });
            } else {
                ui.label(
                    egui::RichText::new("Files changed: Loading changes...")
                        .size(10.0)
                        .color(muted),
                );
                ui.label(
                    egui::RichText::new("Total diff: Loading changes...")
                        .size(10.0)
                        .color(muted),
                );
            }
        });

        ui.add_space(18.0);

        ui.vertical(|ui| {
            ui.set_min_width(280.0);
            if commit.populated {
                if let Some(sig) = signature {
                    paint_signature_block(ui, sig, muted);
                } else {
                    ui.group(|ui| {
                        ui.label(
                            egui::RichText::new("GPG Signature Details")
                                .size(10.0)
                                .strong(),
                        );
                        ui.label(
                            egui::RichText::new("No signature data")
                                .size(10.0)
                                .color(muted),
                        );
                    });
                }
            } else {
                ui.group(|ui| {
                    ui.label(
                        egui::RichText::new("GPG Signature Details")
                            .size(10.0)
                            .strong(),
                    );
                    ui.label(
                        egui::RichText::new("Loading signature...")
                            .size(10.0)
                            .color(muted),
                    );
                });
            }
        });
    });
}

fn paint_author_avatar(ui: &mut egui::Ui, author: &str, app_state: &AppState) {
    let size = egui::vec2(28.0, 28.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    if let Some(path) = app_state.avatar_cache.get(author) {
        let uri = url::Url::from_file_path(path)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| format!("file://{}", path));
        let image = egui::Image::new(uri).corner_radius(2.0);
        image.paint_at(ui, rect);
        return;
    }

    ui.painter()
        .circle_filled(rect.center(), 14.0, egui::Color32::from_rgb(76, 76, 76));
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        author.chars().next().unwrap_or('?').to_string(),
        egui::FontId::proportional(14.0),
        egui::Color32::WHITE,
    );
}

fn paint_signature_block(ui: &mut egui::Ui, sig: &CommitDrawerSignature, muted: egui::Color32) {
    ui.group(|ui| {
        ui.label(
            egui::RichText::new("GPG Signature Details")
                .size(10.0)
                .strong(),
        );
        ui.label(egui::RichText::new(&sig.status).size(10.0).color(muted));
        if let Some(summary) = &sig.summary {
            ui.label(egui::RichText::new(summary).size(10.0).color(muted));
        }
        if let Some(key_id) = &sig.key_id {
            ui.label(
                egui::RichText::new(format!("Key ID: {}", key_id))
                    .size(10.0)
                    .color(muted),
            );
        }
        if let Some(trust) = &sig.trust {
            ui.label(
                egui::RichText::new(format!("Trust: {}", trust))
                    .size(10.0)
                    .color(muted),
            );
        }
    });
}

fn paint_changes_list(ui: &mut egui::Ui, files: &[FileStatus], _muted: egui::Color32) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        for file in files {
            paint_change_row(ui, file);
        }
    });
}

fn paint_change_row(ui: &mut egui::Ui, file: &FileStatus) {
    let (_, icon_color) = file_icon_for_row(&file.kind);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
        let (rect, _) = ui.allocate_exact_size(egui::vec2(13.0, 13.0), egui::Sense::hover());
        crate::ui::core::filetree::paint_file_icon_rect(ui, rect, &file.path, icon_color);
        ui.label(egui::RichText::new(&file.path).size(10.0));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("-{}", file.deletions))
                    .size(9.0)
                    .color(egui::Color32::from_rgb(230, 92, 92)),
            );
            ui.label(
                egui::RichText::new(format!("+{}", file.additions))
                    .size(9.0)
                    .color(egui::Color32::from_rgb(78, 190, 116)),
            );
        });
    });
}

fn file_totals(files: &[FileStatus]) -> (usize, usize) {
    files.iter().fold((0usize, 0usize), |(adds, dels), file| {
        (adds + file.additions, dels + file.deletions)
    })
}

fn file_icon_for_row(kind: &FileChangeKind) -> (&'static str, egui::Color32) {
    match kind {
        FileChangeKind::Added => (FILE_PLUS, egui::Color32::from_rgb(78, 190, 116)),
        FileChangeKind::Modified => (FILE, egui::Color32::from_rgb(252, 197, 34)),
        FileChangeKind::Deleted => (FILE_TEXT, egui::Color32::from_rgb(230, 92, 92)),
        FileChangeKind::Renamed => (FILE_TEXT, egui::Color32::from_rgb(151, 113, 255)),
        FileChangeKind::TypeChanged => (FOLDER, egui::Color32::from_rgb(172, 172, 172)),
    }
}
