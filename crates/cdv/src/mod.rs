use eframe::egui;
use egui_phosphor::regular::{FILE, FILE_PLUS, FILE_TEXT, FOLDER};
use std::hash::{Hash, Hasher};

const FILE_ROW_HEIGHT: f32 = 22.0;
const TIMELINE_ROW_HEIGHT: f32 = 18.0;
const LEFT_PANEL_WIDTH_RATIO: f32 = 0.20;
const LEFT_PANEL_MIN_WIDTH: f32 = 180.0;
const LEFT_PANEL_MAX_WIDTH: f32 = 320.0;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CommitDiffFileKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChanged,
    Copied,
    Untracked,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CommitDiffLineKind {
    Context,
    Addition,
    Deletion,
    Binary,
    EofAddition,
    EofDeletion,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CommitDiffLine {
    pub old_lineno: Option<u32>,
    pub new_lineno: Option<u32>,
    pub kind: CommitDiffLineKind,
    pub content: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CommitDiffHunk {
    pub header: String,
    pub old_start: u32,
    pub old_lines: u32,
    pub new_start: u32,
    pub new_lines: u32,
    pub lines: Vec<CommitDiffLine>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CommitDiffFile {
    pub path: String,
    pub old_path: Option<String>,
    pub kind: CommitDiffFileKind,
    pub staged: bool,
    pub additions: usize,
    pub deletions: usize,
    pub is_binary: bool,
    pub hunks: Vec<CommitDiffHunk>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct CommitDiffSummary {
    pub files_changed: usize,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: usize,
    pub lines: usize,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CommitDiffViewModel {
    pub commit_hash: String,
    pub files: Vec<CommitDiffFile>,
    pub summary: CommitDiffSummary,
}

#[derive(Default)]
pub struct DiffTimelineState {
    selected_file_path: Option<String>,
    cached_fingerprint: Option<u64>,
    cached_rows: Vec<TimelineRow>,
    cached_file_row_indices: Vec<(String, usize)>,
    selected_file_row_index: Option<usize>,
    pending_scroll_to_selected: bool,
}

impl DiffTimelineState {
    pub fn selected_file_path(&self) -> Option<&str> {
        self.selected_file_path.as_deref()
    }

    pub fn select_file_path(&mut self, path: Option<String>) {
        if self.selected_file_path != path {
            self.selected_file_path = path;
            self.pending_scroll_to_selected = true;
        }
    }

    fn sync_model(&mut self, model: &CommitDiffViewModel) {
        let fingerprint = model_fingerprint(model);
        if self.cached_fingerprint == Some(fingerprint) {
            if self.selected_file_path.is_none() && !model.files.is_empty() {
                self.select_file_path(Some(model.files[0].path.clone()));
            }
            return;
        }

        self.cached_fingerprint = Some(fingerprint);
        let (rows, file_row_indices) = build_timeline_rows(model);
        self.cached_rows = rows;
        self.cached_file_row_indices = file_row_indices;

        let selected_exists = self
            .selected_file_path
            .as_ref()
            .is_some_and(|path| model.files.iter().any(|file| &file.path == path));

        if !selected_exists {
            self.selected_file_path = model.files.first().map(|file| file.path.clone());
            self.pending_scroll_to_selected = self.selected_file_path.is_some();
        }

        self.selected_file_row_index = self.selected_file_path.as_ref().and_then(|path| {
            self.cached_file_row_indices
                .iter()
                .find_map(|(file_path, row_index)| (file_path == path).then_some(*row_index))
        });
    }
}

#[derive(Clone, Debug)]
enum TimelineRow {
    FileHeader {
        file_index: usize,
    },
    HunkHeader {
        file_index: usize,
        hunk_index: usize,
    },
    Line {
        file_index: usize,
        hunk_index: usize,
        line_index: usize,
    },
    BinaryNotice {
        file_index: usize,
    },
}

pub fn show(ui: &mut egui::Ui, state: &mut DiffTimelineState, model: Option<&CommitDiffViewModel>) {
    let muted = egui::Color32::from_rgb(172, 172, 172);
    let accent = egui::Color32::from_rgb(78, 190, 116);

    ui.horizontal(|ui| {
        if let Some(model) = model {
            ui.label(
                egui::RichText::new(format!("{} files", model.files.len()))
                    .size(10.0)
                    .color(muted),
            );
            ui.label(
                egui::RichText::new(format!("+{}", model.summary.additions))
                    .size(10.0)
                    .color(accent),
            );
            ui.label(
                egui::RichText::new(format!("-{}", model.summary.deletions))
                    .size(10.0)
                    .color(egui::Color32::from_rgb(230, 92, 92)),
            );
            ui.label(
                egui::RichText::new(format!("{} hunks", model.summary.hunks))
                    .size(10.0)
                    .color(muted),
            );
            if model.summary.truncated {
                ui.label(
                    egui::RichText::new("truncated")
                        .size(10.0)
                        .color(egui::Color32::from_rgb(252, 197, 34)),
                );
            }
        } else {
            ui.label(
                egui::RichText::new("Loading diff details...")
                    .size(10.0)
                    .color(muted),
            );
        }
    });

    ui.add_space(8.0);

    let Some(model) = model else {
        return;
    };

    state.sync_model(model);

    if model.files.is_empty() {
        ui.label(
            egui::RichText::new("No diff data for this commit")
                .size(10.0)
                .color(muted),
        );
        return;
    }

    let total_width = ui.available_width();
    let left_width = (total_width * LEFT_PANEL_WIDTH_RATIO)
        .clamp(LEFT_PANEL_MIN_WIDTH, LEFT_PANEL_MAX_WIDTH)
        .min(total_width * 0.45);
    let right_width = (total_width - left_width - 12.0).max(0.0);
    let content_height = ui.available_height();

    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(left_width, content_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| paint_file_list(ui, state, model, muted),
        );

        ui.add_space(12.0);

        ui.allocate_ui_with_layout(
            egui::vec2(right_width, content_height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| paint_timeline(ui, state, model, muted),
        );
    });
}

fn paint_file_list(
    ui: &mut egui::Ui,
    state: &mut DiffTimelineState,
    model: &CommitDiffViewModel,
    muted: egui::Color32,
) {
    ui.label(egui::RichText::new("Files").size(11.0).strong());
    ui.add_space(6.0);

    egui::ScrollArea::vertical()
        .id_salt("commit_diff_file_list_scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            for (index, file) in model.files.iter().enumerate() {
                let (rect, response) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), FILE_ROW_HEIGHT),
                    egui::Sense::click(),
                );

                let selected = state
                    .selected_file_path()
                    .is_some_and(|path| path == file.path);
                if selected {
                    ui.painter()
                        .rect_filled(rect, 3.0, egui::Color32::from_white_alpha(16));
                } else if response.hovered() {
                    ui.painter()
                        .rect_filled(rect, 3.0, egui::Color32::from_white_alpha(8));
                }

                let (icon, icon_color) = file_icon_for(file);
                ui.painter().text(
                    egui::pos2(rect.left() + 4.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    icon,
                    egui::FontId::proportional(11.0),
                    icon_color,
                );
                ui.painter().text(
                    egui::pos2(rect.left() + 18.0, rect.center().y),
                    egui::Align2::LEFT_CENTER,
                    file_display_name(file),
                    egui::FontId::proportional(9.5),
                    ui.visuals().text_color(),
                );
                ui.painter().text(
                    egui::pos2(rect.right() - 4.0, rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    format!(
                        "{} {}",
                        file_status_label(&file.kind),
                        file_delta_label(file)
                    ),
                    egui::FontId::monospace(8.8),
                    muted,
                );

                if response.clicked() {
                    state.select_file_path(Some(file.path.clone()));
                    state.selected_file_row_index = state
                        .cached_file_row_indices
                        .iter()
                        .find_map(|(path, row_index)| (path == &file.path).then_some(*row_index));
                }

                if index + 1 < model.files.len() {
                    ui.add_space(2.0);
                }
            }
        });
}

fn paint_timeline(
    ui: &mut egui::Ui,
    state: &mut DiffTimelineState,
    model: &CommitDiffViewModel,
    muted: egui::Color32,
) {
    ui.label(egui::RichText::new("Diff timeline").size(11.0).strong());
    ui.add_space(6.0);

    let row_count = state.cached_rows.len();
    egui::ScrollArea::vertical()
        .id_salt("commit_diff_timeline_scroll")
        .auto_shrink([false, false])
        .show_rows(ui, TIMELINE_ROW_HEIGHT, row_count, |ui, row_range| {
            if state.pending_scroll_to_selected {
                if let Some(row_index) = state.selected_file_row_index {
                    let target_y = row_index as f32 * TIMELINE_ROW_HEIGHT;
                    let target_rect = egui::Rect::from_min_size(
                        egui::pos2(0.0, target_y),
                        egui::vec2(1.0, TIMELINE_ROW_HEIGHT),
                    );
                    ui.scroll_to_rect(target_rect, Some(egui::Align::TOP));
                }
                state.pending_scroll_to_selected = false;
            }

            for row_index in row_range {
                let row = state.cached_rows[row_index].clone();
                paint_timeline_row(ui, &row, model, state, muted);
            }
        });
}

fn paint_timeline_row(
    ui: &mut egui::Ui,
    row: &TimelineRow,
    model: &CommitDiffViewModel,
    state: &mut DiffTimelineState,
    muted: egui::Color32,
) {
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), TIMELINE_ROW_HEIGHT),
        egui::Sense::click(),
    );

    let (file_index, path) = match row {
        TimelineRow::FileHeader { file_index }
        | TimelineRow::HunkHeader { file_index, .. }
        | TimelineRow::Line { file_index, .. }
        | TimelineRow::BinaryNotice { file_index } => {
            let file = &model.files[*file_index];
            (*file_index, file.path.as_str())
        }
    };
    let file = &model.files[file_index];
    let selected = state
        .selected_file_path()
        .is_some_and(|selected| selected == path);

    if selected {
        ui.painter()
            .rect_filled(rect, 2.0, egui::Color32::from_white_alpha(14));
    } else if response.hovered() {
        ui.painter()
            .rect_filled(rect, 2.0, egui::Color32::from_white_alpha(8));
    }

    match row {
        TimelineRow::FileHeader { .. } => {
            let (icon, icon_color) = file_icon_for(file);
            let label = file_display_name(file);

            ui.painter().text(
                egui::pos2(rect.left() + 6.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                icon,
                egui::FontId::proportional(11.0),
                icon_color,
            );
            ui.painter().text(
                egui::pos2(rect.left() + 20.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                label,
                egui::FontId::proportional(10.0),
                ui.visuals().text_color(),
            );
            ui.painter().text(
                egui::pos2(rect.right() - 6.0, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                file_status_label(&file.kind),
                egui::FontId::proportional(9.0),
                file_color(&file.kind),
            );
        }
        TimelineRow::HunkHeader { hunk_index, .. } => {
            let hunk = &file.hunks[*hunk_index];
            ui.painter().text(
                egui::pos2(rect.left() + 12.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &hunk.header,
                egui::FontId::monospace(9.5),
                muted,
            );
        }
        TimelineRow::Line {
            hunk_index,
            line_index,
            ..
        } => {
            let line = &file.hunks[*hunk_index].lines[*line_index];
            let (prefix, prefix_color) = match line.kind {
                CommitDiffLineKind::Addition => ("+", egui::Color32::from_rgb(78, 190, 116)),
                CommitDiffLineKind::Deletion => ("-", egui::Color32::from_rgb(230, 92, 92)),
                CommitDiffLineKind::EofAddition => (">", egui::Color32::from_rgb(78, 190, 116)),
                CommitDiffLineKind::EofDeletion => ("<", egui::Color32::from_rgb(230, 92, 92)),
                CommitDiffLineKind::Binary => ("B", muted),
                CommitDiffLineKind::Context => (" ", muted),
            };

            let old_lineno = line
                .old_lineno
                .map(|n| format!("{:>4}", n))
                .unwrap_or_else(|| "    ".to_string());
            let new_lineno = line
                .new_lineno
                .map(|n| format!("{:>4}", n))
                .unwrap_or_else(|| "    ".to_string());

            ui.painter().text(
                egui::pos2(rect.left() + 6.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                old_lineno,
                egui::FontId::monospace(9.0),
                muted,
            );
            ui.painter().text(
                egui::pos2(rect.left() + 50.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                new_lineno,
                egui::FontId::monospace(9.0),
                muted,
            );
            ui.painter().text(
                egui::pos2(rect.left() + 94.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                prefix,
                egui::FontId::monospace(9.5),
                prefix_color,
            );
            ui.painter().text(
                egui::pos2(rect.left() + 108.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                &line.content,
                egui::FontId::monospace(9.0),
                ui.visuals().text_color(),
            );
        }
        TimelineRow::BinaryNotice { .. } => {
            ui.painter().text(
                egui::pos2(rect.left() + 12.0, rect.center().y),
                egui::Align2::LEFT_CENTER,
                "Binary file",
                egui::FontId::proportional(9.5),
                muted,
            );
        }
    }

    if response.clicked() && matches!(row, TimelineRow::FileHeader { .. }) {
        state.select_file_path(Some(path.to_string()));
        state.selected_file_row_index = state
            .cached_file_row_indices
            .iter()
            .find_map(|(file_path, row_index)| (file_path == path).then_some(*row_index));
    }
}

fn build_timeline_rows(model: &CommitDiffViewModel) -> (Vec<TimelineRow>, Vec<(String, usize)>) {
    let mut rows = Vec::new();
    let mut file_row_indices = Vec::new();
    for (file_index, file) in model.files.iter().enumerate() {
        file_row_indices.push((file.path.clone(), rows.len()));
        rows.push(TimelineRow::FileHeader { file_index });
        if file.is_binary {
            rows.push(TimelineRow::BinaryNotice { file_index });
            continue;
        }
        for (hunk_index, hunk) in file.hunks.iter().enumerate() {
            rows.push(TimelineRow::HunkHeader {
                file_index,
                hunk_index,
            });
            for (line_index, _) in hunk.lines.iter().enumerate() {
                rows.push(TimelineRow::Line {
                    file_index,
                    hunk_index,
                    line_index,
                });
            }
        }
    }
    (rows, file_row_indices)
}

fn model_fingerprint(model: &CommitDiffViewModel) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    model.commit_hash.hash(&mut hasher);
    model.summary.hash(&mut hasher);

    for file in &model.files {
        file.hash(&mut hasher);
    }

    hasher.finish()
}

fn file_icon_for(file: &CommitDiffFile) -> (&'static str, egui::Color32) {
    match file.kind {
        CommitDiffFileKind::Added | CommitDiffFileKind::Copied | CommitDiffFileKind::Untracked => {
            (FILE_PLUS, egui::Color32::from_rgb(78, 190, 116))
        }
        CommitDiffFileKind::Deleted => (FILE_TEXT, egui::Color32::from_rgb(230, 92, 92)),
        CommitDiffFileKind::Renamed => (FILE_TEXT, egui::Color32::from_rgb(151, 113, 255)),
        CommitDiffFileKind::TypeChanged => (FOLDER, egui::Color32::from_rgb(172, 172, 172)),
        CommitDiffFileKind::Modified | CommitDiffFileKind::Unknown => {
            (FILE, egui::Color32::from_rgb(252, 197, 34))
        }
    }
}

fn file_display_name(file: &CommitDiffFile) -> String {
    if let Some(old_path) = &file.old_path {
        format!("{} → {}", old_path, file.path)
    } else {
        file.path.clone()
    }
}

fn file_delta_label(file: &CommitDiffFile) -> String {
    format!("+{} -{}", file.additions, file.deletions)
}

fn file_color(kind: &CommitDiffFileKind) -> egui::Color32 {
    match kind {
        CommitDiffFileKind::Added | CommitDiffFileKind::Copied | CommitDiffFileKind::Untracked => {
            egui::Color32::from_rgb(78, 190, 116)
        }
        CommitDiffFileKind::Deleted => egui::Color32::from_rgb(230, 92, 92),
        CommitDiffFileKind::Renamed => egui::Color32::from_rgb(151, 113, 255),
        CommitDiffFileKind::TypeChanged => egui::Color32::from_rgb(172, 172, 172),
        CommitDiffFileKind::Modified | CommitDiffFileKind::Unknown => {
            egui::Color32::from_rgb(252, 197, 34)
        }
    }
}

fn file_status_label(kind: &CommitDiffFileKind) -> &'static str {
    match kind {
        CommitDiffFileKind::Added | CommitDiffFileKind::Copied | CommitDiffFileKind::Untracked => {
            "A"
        }
        CommitDiffFileKind::Modified | CommitDiffFileKind::Unknown => "M",
        CommitDiffFileKind::Deleted => "D",
        CommitDiffFileKind::Renamed => "R",
        CommitDiffFileKind::TypeChanged => "T",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_model() -> CommitDiffViewModel {
        CommitDiffViewModel {
            commit_hash: "abc123".to_string(),
            summary: CommitDiffSummary {
                files_changed: 1,
                additions: 3,
                deletions: 1,
                hunks: 1,
                lines: 4,
                truncated: false,
            },
            files: vec![CommitDiffFile {
                path: "src/main.rs".to_string(),
                old_path: None,
                kind: CommitDiffFileKind::Modified,
                staged: false,
                additions: 3,
                deletions: 1,
                is_binary: false,
                hunks: vec![CommitDiffHunk {
                    header: "@@ -1,2 +1,4 @@".to_string(),
                    old_start: 1,
                    old_lines: 2,
                    new_start: 1,
                    new_lines: 4,
                    lines: vec![
                        CommitDiffLine {
                            old_lineno: Some(1),
                            new_lineno: Some(1),
                            kind: CommitDiffLineKind::Context,
                            content: "fn main() {".to_string(),
                        },
                        CommitDiffLine {
                            old_lineno: None,
                            new_lineno: Some(2),
                            kind: CommitDiffLineKind::Addition,
                            content: "+println!(\"hi\");".to_string(),
                        },
                    ],
                }],
            }],
        }
    }

    #[test]
    fn sync_model_selects_first_file() {
        let model = sample_model();
        let mut state = DiffTimelineState::default();
        state.sync_model(&model);
        assert_eq!(state.selected_file_path(), Some("src/main.rs"));
        assert!(!state.cached_rows.is_empty());
    }

    #[test]
    fn selecting_same_path_does_not_flip_scroll_flag() {
        let mut state = DiffTimelineState::default();
        state.select_file_path(Some("src/main.rs".to_string()));
        assert_eq!(state.selected_file_path(), Some("src/main.rs"));
        state.select_file_path(Some("src/main.rs".to_string()));
        assert_eq!(state.selected_file_path(), Some("src/main.rs"));
    }

    #[test]
    fn build_rows_flattens_files_and_hunks() {
        let model = sample_model();
        let (rows, file_rows) = build_timeline_rows(&model);
        assert!(matches!(rows[0], TimelineRow::FileHeader { .. }));
        assert!(
            rows.iter()
                .any(|row| matches!(row, TimelineRow::HunkHeader { .. }))
        );
        assert!(
            rows.iter()
                .any(|row| matches!(row, TimelineRow::Line { .. }))
        );
        assert_eq!(file_rows.len(), 1);
    }
}
