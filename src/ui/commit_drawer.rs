use eframe::egui;
use egui_phosphor::regular::{
    CARET_DOWN, CARET_RIGHT, FILE, FILE_PLUS, FILE_TEXT, FOLDER, FOLDER_OPEN,
};
use std::collections::BTreeMap;

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
    pub parents: Vec<String>,
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
        }
    }
}

pub fn show(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    state: &mut State,
    app_state: &AppState,
    commit: Option<&CommitDrawerCommit>,
    files: &[FileStatus],
) {
    let fill = egui::Color32::from_rgb(36, 36, 36);
    let header_fill = egui::Color32::from_rgb(44, 44, 44);
    let footer_fill = egui::Color32::from_rgb(40, 40, 40);
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(78, 78, 78));
    let muted = egui::Color32::from_rgb(172, 172, 172);

    let panel_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), rect.top()),
        egui::pos2(rect.right(), rect.bottom()),
    );

    ui.painter().rect_filled(panel_rect, 0.0, fill);
    ui.painter()
        .line_segment([panel_rect.left_top(), panel_rect.right_top()], stroke);

    let header_height = 34.0;
    let footer_height = 28.0;
    let header_rect = egui::Rect::from_min_size(
        panel_rect.left_top(),
        egui::vec2(panel_rect.width(), header_height),
    );
    let footer_rect = egui::Rect::from_min_size(
        egui::pos2(panel_rect.left(), panel_rect.bottom() - footer_height),
        egui::vec2(panel_rect.width(), footer_height),
    );
    ui.painter().rect_filled(header_rect, 0.0, header_fill);
    ui.painter().rect_filled(footer_rect, 0.0, footer_fill);
    ui.painter()
        .line_segment([footer_rect.left_top(), footer_rect.right_top()], stroke);

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
        egui::pos2(panel_rect.right(), footer_rect.top()),
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
                    CommitDrawerTab::Commit => paint_commit_tab(ui, commit, files, muted),
                    CommitDrawerTab::Changes => paint_changes_tab(ui, files, muted),
                    CommitDrawerTab::FileTree => {
                        paint_tree_tab(ui, &mut state.tree_state, files, muted)
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

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("commit_drawer_footer")
            .max_rect(footer_rect.shrink2(egui::vec2(12.0, 4.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.label(
                egui::RichText::new("Changes and file tree are UI placeholders for now")
                    .size(9.0)
                    .color(muted),
            );
        },
    );
}

fn tab_button(ui: &mut egui::Ui, state: &mut State, tab: CommitDrawerTab, label: &str) {
    let selected = state.tab == tab;
    let response = ui.selectable_label(selected, label);
    if response.clicked() {
        state.tab = tab;
    }
}

fn paint_commit_summary(ui: &mut egui::Ui, commit: &CommitDrawerCommit, muted: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(&commit.message).size(14.0).strong());
    });
    ui.label(
        egui::RichText::new(format!("{} <{}>", commit.author, commit.email))
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
    files: &[FileStatus],
    muted: egui::Color32,
) {
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
    ui.add_space(8.0);
    paint_changes_list(ui, files, muted);
}

fn paint_changes_tab(ui: &mut egui::Ui, files: &[FileStatus], muted: egui::Color32) {
    paint_changes_list(ui, files, muted);
}

fn paint_tree_tab(
    ui: &mut egui::Ui,
    tree_state: &mut TreeState,
    files: &[FileStatus],
    muted: egui::Color32,
) {
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
            for row in &mut tree_state.rows {
                paint_tree_entry(ui, row, 0, muted);
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

fn paint_tree_entry(ui: &mut egui::Ui, entry: &mut TreeEntry, depth: usize, muted: egui::Color32) {
    let row_height = 20.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_height),
        egui::Sense::click(),
    );

    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 3.0, egui::Color32::from_white_alpha(12));
    }

    let indent = 12.0 * depth as f32;
    let mut x = rect.left() + 4.0 + indent;
    let center_y = rect.center().y;

    if matches!(entry.kind, TreeEntryKind::Directory) {
        let caret = if entry.expanded {
            CARET_DOWN
        } else {
            CARET_RIGHT
        };
        ui.painter().text(
            egui::pos2(x, center_y),
            egui::Align2::CENTER_CENTER,
            caret,
            egui::FontId::proportional(9.0),
            muted,
        );
        if response.clicked() {
            entry.expanded = !entry.expanded;
        }
    }
    x += 8.0;

    let show_guide = depth > 0;
    if show_guide {
        draw_indent_guide(ui, rect, depth, muted);
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

    ui.painter().text(
        egui::pos2(x, center_y),
        egui::Align2::CENTER_CENTER,
        icon,
        egui::FontId::proportional(12.0),
        icon_color,
    );
    x += 16.0;

    ui.painter().text(
        egui::pos2(x, center_y),
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

    if matches!(entry.kind, TreeEntryKind::Directory) && entry.expanded {
        if !entry.children.is_empty() {
            let guide_x = rect.left() + 10.0 + 12.0 * depth as f32;
            ui.painter().line_segment(
                [egui::pos2(guide_x, rect.bottom() - 2.0), egui::pos2(guide_x, rect.bottom() + 10.0)],
                egui::Stroke::new(1.0_f32, muted.linear_multiply(0.35)),
            );
        }

        for child in &mut entry.children {
            paint_tree_entry(ui, child, depth + 1, muted);
        }
    }
}

fn draw_indent_guide(ui: &egui::Ui, rect: egui::Rect, depth: usize, muted: egui::Color32) {
    let guide_x = rect.left() + 10.0 + 12.0 * depth.saturating_sub(1) as f32;
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

fn file_status_label(kind: FileChangeKind) -> (&'static str, egui::Color32) {
    match kind {
        FileChangeKind::Added => ("A", egui::Color32::from_rgb(78, 190, 116)),
        FileChangeKind::Modified => ("M", egui::Color32::from_rgb(252, 197, 34)),
        FileChangeKind::Deleted => ("D", egui::Color32::from_rgb(228, 86, 86)),
        FileChangeKind::Renamed => ("R", egui::Color32::from_rgb(172, 172, 172)),
        FileChangeKind::TypeChanged => ("T", egui::Color32::from_rgb(172, 172, 172)),
    }
}

fn paint_changes_list(ui: &mut egui::Ui, files: &[FileStatus], muted: egui::Color32) {
    if files.is_empty() {
        ui.label(
            egui::RichText::new("No file changes")
                .size(10.0)
                .color(muted),
        );
        return;
    }

    egui::ScrollArea::vertical().show(ui, |ui| {
        for file in files {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("•").size(12.0).color(muted));
                ui.label(egui::RichText::new(&file.path).size(10.0));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(format!("+{} -{}", file.additions, file.deletions))
                            .size(9.0)
                            .color(muted),
                    );
                });
            });
        }
    });
}
