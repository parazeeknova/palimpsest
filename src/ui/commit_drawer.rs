use eframe::egui;
use egui_phosphor::regular::{
    ARROW_SQUARE_IN, ARROW_SQUARE_OUT, CARET_DOWN, CARET_RIGHT, FILE, FILE_PLUS, FILE_TEXT, FOLDER,
    FOLDER_OPEN, X,
};
use std::collections::BTreeMap;

use crate::git::models::{FileChangeKind, FileStatus};
use crate::state::AppState;

const TREE_ROW_HEIGHT: f32 = 20.0;
const TREE_SLOT_WIDTH: f32 = 22.0;
const TREE_LEFT_PADDING: f32 = 6.0;
const TREE_CARET_SLOT: f32 = 6.0;
const TREE_ICON_GAP: f32 = 24.0;

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
    tree_state: TreeState,
    pub height: f32,
    pub detached: bool,
}

#[derive(Default)]
struct TreeState {
    rows: Vec<TreeEntry>,
    rebuild_key: Option<String>,
}

#[derive(Clone, Debug)]
struct TreeEntry {
    path: String,
    #[allow(dead_code)]
    label: String,
    kind: TreeEntryKind,
    file_kind: Option<FileChangeKind>,
    expanded: bool,
    has_children: bool,
    file_index: Option<usize>,
    children: Vec<TreeEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TreeEntryKind {
    Directory,
    File,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: CommitDrawerTab::Commit,
            tree_state: TreeState::default(),
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

pub fn show(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    state: &mut State,
    app_state: &AppState,
    commit: Option<&CommitDrawerCommit>,
    signature: Option<&CommitDrawerSignature>,
    files: &[FileStatus],
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
                    CommitDrawerTab::Changes => {
                        paint_changes_tab(ui, files, commit.populated, muted)
                    }
                    CommitDrawerTab::FileTree => {
                        paint_tree_tab(ui, &mut state.tree_state, files, commit.populated, muted)
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
    files: &[FileStatus],
    populated: bool,
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
    let (icon, icon_color) = file_icon_for_row(&file.kind);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
        ui.label(egui::RichText::new(icon).size(11.0).color(icon_color));
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

fn paint_tree_tab(
    ui: &mut egui::Ui,
    tree_state: &mut TreeState,
    files: &[FileStatus],
    populated: bool,
    muted: egui::Color32,
) {
    if !populated {
        ui.label(
            egui::RichText::new("Loading files...")
                .size(10.0)
                .color(muted),
        );
        return;
    }
    if files.is_empty() {
        ui.label(
            egui::RichText::new("No files in this commit")
                .size(10.0)
                .color(muted),
        );
        return;
    }

    rebuild_tree_if_needed(tree_state, files);
    paint_tree_header(ui, tree_state, muted);
    ui.add_space(6.0);

    egui::ScrollArea::vertical()
        .id_salt("commit_drawer_tree_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let len = tree_state.rows.len();
            let mut ancestors_last = Vec::new();
            for (index, row) in tree_state.rows.iter_mut().enumerate() {
                paint_tree_entry(ui, row, 0, &mut ancestors_last, index + 1 == len, muted);
            }
        });
}

fn paint_tree_header(ui: &mut egui::Ui, tree_state: &mut TreeState, muted: egui::Color32) {
    ui.horizontal(|ui| {
        if ui
            .button(egui::RichText::new("Expand All").size(9.0).color(muted))
            .clicked()
        {
            set_all_directories_expanded(tree_state, true);
        }
        if ui
            .button(egui::RichText::new("Collapse All").size(9.0).color(muted))
            .clicked()
        {
            set_all_directories_expanded(tree_state, false);
        }
    });
}

fn rebuild_tree_if_needed(tree_state: &mut TreeState, files: &[FileStatus]) {
    let key = tree_fingerprint(files);

    if tree_state.rebuild_key.as_deref() == Some(key.as_str()) {
        return;
    }

    tree_state.rows = build_tree_entries(files);
    tree_state.rebuild_key = Some(key);
}

fn tree_fingerprint(files: &[FileStatus]) -> String {
    let mut fingerprint = String::new();
    for file in files {
        fingerprint.push_str(&file.path);
        fingerprint.push('|');
        fingerprint.push_str(file.old_path.as_deref().unwrap_or(""));
        fingerprint.push('|');
        fingerprint.push_str(if file.staged { "1" } else { "0" });
        fingerprint.push('|');
        fingerprint.push_str(match file.kind {
            FileChangeKind::Added => "A",
            FileChangeKind::Modified => "M",
            FileChangeKind::Deleted => "D",
            FileChangeKind::Renamed => "R",
            FileChangeKind::TypeChanged => "T",
        });
        fingerprint.push(';');
    }
    fingerprint
}

fn build_tree_entries(files: &[FileStatus]) -> Vec<TreeEntry> {
    let mut root_map: BTreeMap<String, TreeEntry> = BTreeMap::new();

    for (file_index, file) in files.iter().enumerate() {
        let segments: Vec<&str> = file
            .path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        if segments.is_empty() {
            continue;
        }

        insert_tree_entry(
            &mut root_map,
            &segments,
            0,
            file_index,
            &file.kind,
            String::new(),
        );
    }

    root_map.into_values().collect()
}

fn insert_tree_entry(
    nodes: &mut BTreeMap<String, TreeEntry>,
    segments: &[&str],
    _depth: usize,
    file_index: usize,
    file_kind: &FileChangeKind,
    mut path_prefix: String,
) {
    if !path_prefix.is_empty() {
        path_prefix.push('/');
    }
    path_prefix.push_str(segments[0]);

    let is_file = segments.len() == 1;
    let entry = nodes
        .entry(segments[0].to_string())
        .or_insert_with(|| TreeEntry {
            path: path_prefix.clone(),
            label: segments[0].to_string(),
            kind: if is_file {
                TreeEntryKind::File
            } else {
                TreeEntryKind::Directory
            },
            file_kind: if is_file {
                Some(file_kind.clone())
            } else {
                None
            },
            file_index: if is_file { Some(file_index) } else { None },
            expanded: true,
            has_children: !is_file,
            children: Vec::new(),
        });

    if is_file {
        entry.kind = TreeEntryKind::File;
        entry.file_kind = Some(file_kind.clone());
        entry.file_index = Some(file_index);
        return;
    }

    if segments.len() > 1 {
        let mut child_map: BTreeMap<String, TreeEntry> = entry
            .children
            .drain(..)
            .map(|child| (child.label.clone(), child))
            .collect();
        insert_tree_entry(
            &mut child_map,
            &segments[1..],
            _depth + 1,
            file_index,
            file_kind,
            path_prefix,
        );
        entry.children = child_map.into_values().collect();
        entry.has_children = true;
    }
}

fn paint_tree_entry(
    ui: &mut egui::Ui,
    entry: &mut TreeEntry,
    depth: usize,
    ancestors_last: &mut Vec<bool>,
    is_last: bool,
    muted: egui::Color32,
) -> f32 {
    let row_height = TREE_ROW_HEIGHT;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_height),
        egui::Sense::click(),
    );

    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 3.0, egui::Color32::from_white_alpha(12));
    }

    let row_left = rect.left() + TREE_LEFT_PADDING;
    let slot_left = row_left + TREE_SLOT_WIDTH * depth as f32;
    let center_y = rect.center().y;

    paint_tree_guides(ui, rect, ancestors_last, muted);

    if matches!(entry.kind, TreeEntryKind::Directory) {
        let caret = if entry.expanded {
            CARET_DOWN
        } else {
            CARET_RIGHT
        };
        ui.painter().text(
            egui::pos2(slot_left + TREE_CARET_SLOT, center_y),
            egui::Align2::CENTER_CENTER,
            caret,
            egui::FontId::proportional(9.0),
            muted,
        );
        if response.clicked() {
            entry.expanded = !entry.expanded;
        }
    }

    let (icon, icon_color) = match entry.kind {
        TreeEntryKind::Directory => {
            let icon = if entry.expanded { FOLDER_OPEN } else { FOLDER };
            (icon, muted)
        }
        TreeEntryKind::File => (
            file_icon(entry.file_kind.as_ref(), &entry.path),
            file_icon_color(entry.file_kind.as_ref()),
        ),
    };

    let icon_x = if matches!(entry.kind, TreeEntryKind::Directory) {
        slot_left + TREE_ICON_GAP
    } else {
        slot_left + 2.0
    };

    ui.painter().text(
        egui::pos2(icon_x, center_y),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(12.0),
        icon_color,
    );

    ui.painter().text(
        egui::pos2(icon_x + 10.0, center_y),
        egui::Align2::LEFT_CENTER,
        &entry.label,
        egui::FontId::proportional(10.0),
        ui.visuals().text_color(),
    );

    if let Some(kind) = entry.file_kind.as_ref() {
        let (status_label, status_color) = file_status_label(kind.clone());
        ui.painter().text(
            egui::pos2(rect.right() - 12.0, center_y),
            egui::Align2::RIGHT_CENTER,
            status_label,
            egui::FontId::proportional(9.0),
            status_color,
        );
    }

    let mut subtree_height = row_height;

    if matches!(entry.kind, TreeEntryKind::Directory)
        && entry.expanded
        && !entry.children.is_empty()
    {
        let guide_x = slot_left + TREE_CARET_SLOT;
        let mut child_bottom = rect.bottom();

        ancestors_last.push(is_last);
        let child_len = entry.children.len();
        for (index, child) in entry.children.iter_mut().enumerate() {
            let child_height = paint_tree_entry(
                ui,
                child,
                depth + 1,
                ancestors_last,
                index + 1 == child_len,
                muted,
            );
            child_bottom += child_height;
            subtree_height += child_height;
        }
        ancestors_last.pop();

        ui.painter().line_segment(
            [
                egui::pos2(guide_x, rect.bottom() - 2.0),
                egui::pos2(guide_x, child_bottom - 1.0),
            ],
            egui::Stroke::new(1.0_f32, muted.linear_multiply(0.35)),
        );
    }

    subtree_height
}

fn paint_tree_guides(
    ui: &egui::Ui,
    rect: egui::Rect,
    ancestors_last: &[bool],
    muted: egui::Color32,
) {
    let row_left = rect.left() + TREE_LEFT_PADDING;

    for (depth, is_last) in ancestors_last.iter().enumerate() {
        if *is_last {
            continue;
        }

        let guide_x = row_left + TREE_SLOT_WIDTH * depth as f32 + TREE_CARET_SLOT;
        ui.painter().line_segment(
            [
                egui::pos2(guide_x, rect.top()),
                egui::pos2(guide_x, rect.bottom()),
            ],
            egui::Stroke::new(1.0_f32, muted.linear_multiply(0.28)),
        );
    }
}

#[allow(dead_code)]
fn draw_indent_guide(ui: &egui::Ui, rect: egui::Rect, depth: usize, muted: egui::Color32) {
    let guide_x = rect.left() + 10.0 + 12.0 * depth as f32;
    ui.painter().line_segment(
        [
            egui::pos2(guide_x, rect.top()),
            egui::pos2(guide_x, rect.bottom()),
        ],
        egui::Stroke::new(1.0_f32, muted.linear_multiply(0.28)),
    );
}

fn set_all_directories_expanded(tree_state: &mut TreeState, expanded: bool) {
    for entry in &mut tree_state.rows {
        set_entry_expanded(entry, expanded);
    }
}

fn set_entry_expanded(entry: &mut TreeEntry, expanded: bool) {
    if matches!(entry.kind, TreeEntryKind::Directory) {
        entry.expanded = expanded;
        for child in &mut entry.children {
            set_entry_expanded(child, expanded);
        }
    }
}

fn file_icon(file_kind: Option<&FileChangeKind>, path: &str) -> &'static str {
    match file_kind {
        Some(FileChangeKind::Added) => FILE_PLUS,
        Some(FileChangeKind::Deleted) => FILE_TEXT,
        Some(FileChangeKind::Renamed) => FILE_TEXT,
        Some(FileChangeKind::TypeChanged) => FOLDER,
        Some(FileChangeKind::Modified) | None => file_icon_by_extension(path),
    }
}

fn file_icon_color(file_kind: Option<&FileChangeKind>) -> egui::Color32 {
    match file_kind {
        Some(FileChangeKind::Added) => egui::Color32::from_rgb(78, 190, 116),
        Some(FileChangeKind::Deleted) => egui::Color32::from_rgb(228, 86, 86),
        Some(FileChangeKind::Renamed) => egui::Color32::from_rgb(172, 172, 172),
        Some(FileChangeKind::TypeChanged) => egui::Color32::from_rgb(172, 172, 172),
        Some(FileChangeKind::Modified) | None => egui::Color32::from_rgb(252, 197, 34),
    }
}

fn file_status_label(kind: FileChangeKind) -> (&'static str, egui::Color32) {
    match kind {
        FileChangeKind::Added => ("A", egui::Color32::from_rgb(78, 190, 116)),
        FileChangeKind::Modified => ("M", egui::Color32::from_rgb(252, 197, 34)),
        FileChangeKind::Deleted => ("D", egui::Color32::from_rgb(228, 86, 86)),
        FileChangeKind::Renamed => ("R", egui::Color32::from_rgb(172, 172, 172)),
        FileChangeKind::TypeChanged => ("T", egui::Color32::from_rgb(172, 172, 172)),
    }
}

fn file_icon_by_extension(path: &str) -> &'static str {
    match path.rsplit('.').next() {
        Some("rs") => FILE_TEXT,
        Some("toml") => FILE_TEXT,
        Some("md") => FILE_TEXT,
        Some("json") => FILE_TEXT,
        Some("yaml") | Some("yml") => FILE_TEXT,
        Some("png") | Some("jpg") | Some("jpeg") | Some("gif") | Some("svg") => FILE,
        _ => FILE_TEXT,
    }
}
