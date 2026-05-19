use eframe::egui;
use std::collections::HashMap;
use std::ops::Range;

use crate::state::{AppState, CachedCommit};
use crate::ui::commit_panel;

const HEADER_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 28.0;
const LANE_WIDTH: f32 = 16.0;
const LEFT_PADDING: f32 = 12.0;
const LINE_WIDTH: f32 = 1.5;
const COMMIT_CIRCLE_RADIUS: f32 = 3.5;
const MIN_SUBJECT_WIDTH: f32 = 300.0;
const MIN_AUTHOR_WIDTH: f32 = 150.0;
const MIN_HASH_WIDTH: f32 = 86.0;
const MIN_DATE_WIDTH: f32 = 150.0;

const BRANCH_COLORS: [egui::Color32; 6] = [
    egui::Color32::from_rgb(255, 165, 16),
    egui::Color32::from_rgb(238, 202, 34),
    egui::Color32::from_rgb(255, 45, 72),
    egui::Color32::from_rgb(151, 113, 73),
    egui::Color32::from_rgb(42, 167, 222),
    egui::Color32::from_rgb(56, 193, 114),
];

#[derive(Clone, Debug)]
enum CurveKind {
    Merge,
    Checkout,
}

#[derive(Clone, Debug)]
enum CommitLineSegment {
    Straight {
        to_row: usize,
    },
    Curve {
        to_column: usize,
        on_row: usize,
        curve_kind: CurveKind,
    },
}

#[derive(Clone, Debug)]
struct CommitLine {
    child_column: usize,
    full_interval: Range<usize>,
    color_idx: usize,
    segments: Vec<CommitLineSegment>,
}

#[derive(Clone, Debug)]
enum LaneState {
    Empty,
    Active {
        #[allow(dead_code)]
        child: usize,
        #[allow(dead_code)]
        parent: usize,
        color: Option<usize>,
        starting_row: usize,
        starting_col: usize,
        destination_column: Option<usize>,
        segments: Vec<CommitLineSegment>,
    },
}

impl LaneState {
    fn is_empty(&self) -> bool {
        matches!(self, LaneState::Empty)
    }

    fn into_commit_lines(
        self,
        ending_row: usize,
        lane_column: usize,
        parent_column: usize,
        parent_color: usize,
    ) -> Option<CommitLine> {
        match self {
            LaneState::Active {
                child: _,
                parent: _,
                color,
                starting_row,
                starting_col,
                destination_column,
                mut segments,
            } => {
                let final_destination = destination_column.unwrap_or(parent_column);
                let final_color = color.unwrap_or(parent_color);

                if let Some(last) = segments.last_mut() {
                    match last {
                        CommitLineSegment::Straight { to_row } if *to_row == usize::MAX => {
                            if final_destination != lane_column {
                                *to_row = ending_row - 1;
                                segments.push(CommitLineSegment::Curve {
                                    to_column: final_destination,
                                    on_row: ending_row,
                                    curve_kind: CurveKind::Checkout,
                                });
                            } else {
                                *to_row = ending_row;
                            }
                        }
                        CommitLineSegment::Curve {
                            on_row,
                            to_column,
                            curve_kind,
                        } if *on_row == usize::MAX => {
                            if *to_column == usize::MAX {
                                *to_column = final_destination;
                            }
                            if matches!(curve_kind, CurveKind::Merge) {
                                *on_row = starting_row + 1;
                                if *on_row < ending_row {
                                    if *to_column != final_destination {
                                        segments.push(CommitLineSegment::Straight {
                                            to_row: ending_row - 1,
                                        });
                                        segments.push(CommitLineSegment::Curve {
                                            to_column: final_destination,
                                            on_row: ending_row,
                                            curve_kind: CurveKind::Checkout,
                                        });
                                    } else {
                                        segments.push(CommitLineSegment::Straight {
                                            to_row: ending_row,
                                        });
                                    }
                                } else if *to_column != final_destination {
                                    segments.push(CommitLineSegment::Curve {
                                        to_column: final_destination,
                                        on_row: ending_row,
                                        curve_kind: CurveKind::Checkout,
                                    });
                                }
                            } else {
                                *on_row = ending_row;
                                if *to_column != final_destination {
                                    segments
                                        .push(CommitLineSegment::Straight { to_row: ending_row });
                                    segments.push(CommitLineSegment::Curve {
                                        to_column: final_destination,
                                        on_row: ending_row,
                                        curve_kind: CurveKind::Checkout,
                                    });
                                }
                            }
                        }
                        CommitLineSegment::Curve {
                            on_row, to_column, ..
                        } => {
                            if *on_row < ending_row {
                                if *to_column != final_destination {
                                    segments.push(CommitLineSegment::Straight {
                                        to_row: ending_row - 1,
                                    });
                                    segments.push(CommitLineSegment::Curve {
                                        to_column: final_destination,
                                        on_row: ending_row,
                                        curve_kind: CurveKind::Checkout,
                                    });
                                } else {
                                    segments
                                        .push(CommitLineSegment::Straight { to_row: ending_row });
                                }
                            } else if *to_column != final_destination {
                                segments.push(CommitLineSegment::Curve {
                                    to_column: final_destination,
                                    on_row: ending_row,
                                    curve_kind: CurveKind::Checkout,
                                });
                            }
                        }
                        _ => {}
                    }
                }

                Some(CommitLine {
                    child_column: starting_col,
                    full_interval: starting_row..ending_row,
                    color_idx: final_color,
                    segments,
                })
            }
            LaneState::Empty => None,
        }
    }
}

struct CommitEntry {
    data: CachedCommit,
    lane: usize,
    color_idx: usize,
}

struct GraphData {
    lane_states: Vec<LaneState>,
    lane_colors: HashMap<usize, usize>,
    parent_to_lanes: HashMap<usize, Vec<usize>>,
    next_color: usize,
    commits: Vec<CommitEntry>,
    max_lanes: usize,
    lines: Vec<CommitLine>,
}

impl GraphData {
    fn new() -> Self {
        Self {
            lane_states: Vec::new(),
            lane_colors: HashMap::new(),
            parent_to_lanes: HashMap::new(),
            next_color: 0,
            commits: Vec::new(),
            max_lanes: 0,
            lines: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.lane_states.clear();
        self.lane_colors.clear();
        self.parent_to_lanes.clear();
        self.commits.clear();
        self.lines.clear();
        self.next_color = 0;
        self.max_lanes = 0;
    }

    fn first_empty_lane_idx(&mut self) -> usize {
        self.lane_states
            .iter()
            .position(LaneState::is_empty)
            .unwrap_or_else(|| {
                self.lane_states.push(LaneState::Empty);
                self.lane_states.len() - 1
            })
    }

    fn get_lane_color(&mut self, lane_idx: usize) -> usize {
        let accent_colors_count = BRANCH_COLORS.len();
        *self.lane_colors.entry(lane_idx).or_insert_with(|| {
            let color_idx = self.next_color;
            self.next_color = (self.next_color + 1) % accent_colors_count;
            color_idx
        })
    }

    fn add_commits(&mut self, commits: &[CachedCommit]) {
        self.commits.reserve(commits.len());
        self.lines.reserve(commits.len() / 2);

        for (commit_idx, commit) in commits.iter().enumerate() {
            let commit_row = self.commits.len();

            let commit_lane = self
                .parent_to_lanes
                .get(&commit_idx)
                .and_then(|lanes| lanes.iter().min().copied());

            let commit_lane = commit_lane.unwrap_or_else(|| self.first_empty_lane_idx());
            let commit_color = self.get_lane_color(commit_lane);

            if let Some(lanes) = self.parent_to_lanes.remove(&commit_idx) {
                for lane_column in lanes {
                    let state = &mut self.lane_states[lane_column];

                    if let LaneState::Active {
                        starting_row,
                        segments,
                        ..
                    } = state
                    {
                        if let Some(CommitLineSegment::Curve {
                            to_column,
                            curve_kind: CurveKind::Merge,
                            ..
                        }) = segments.first_mut()
                        {
                            let curve_row = *starting_row + 1;
                            let would_overlap =
                                if lane_column != commit_lane && curve_row < commit_row {
                                    self.commits[curve_row..commit_row]
                                        .iter()
                                        .any(|c| c.lane == commit_lane)
                                } else {
                                    false
                                };

                            if would_overlap {
                                *to_column = lane_column;
                            }
                        }
                    }

                    let lane_state = std::mem::replace(state, LaneState::Empty);
                    if let Some(commit_line) = lane_state.into_commit_lines(
                        commit_row,
                        lane_column,
                        commit_lane,
                        commit_color,
                    ) {
                        self.lines.push(commit_line);
                    }
                }
            }

            for (parent_idx, parent_hash) in commit.parents.iter().enumerate() {
                let parent_global_idx = commits
                    .iter()
                    .position(|c| c.hash == *parent_hash || c.short_hash == *parent_hash);

                if let Some(parent_global_idx) = parent_global_idx {
                    if parent_idx == 0 {
                        self.lane_states[commit_lane] = LaneState::Active {
                            parent: parent_global_idx,
                            child: commit_idx,
                            color: Some(commit_color),
                            starting_col: commit_lane,
                            starting_row: commit_row,
                            destination_column: None,
                            segments: vec![CommitLineSegment::Straight { to_row: usize::MAX }],
                        };

                        self.parent_to_lanes
                            .entry(parent_global_idx)
                            .or_default()
                            .push(commit_lane);
                    } else {
                        let new_lane = self.first_empty_lane_idx();

                        self.lane_states[new_lane] = LaneState::Active {
                            parent: parent_global_idx,
                            child: commit_idx,
                            color: None,
                            starting_col: commit_lane,
                            starting_row: commit_row,
                            destination_column: None,
                            segments: vec![CommitLineSegment::Curve {
                                to_column: usize::MAX,
                                on_row: usize::MAX,
                                curve_kind: CurveKind::Merge,
                            }],
                        };

                        self.parent_to_lanes
                            .entry(parent_global_idx)
                            .or_default()
                            .push(new_lane);
                    }
                }
            }

            self.max_lanes = self.max_lanes.max(self.lane_states.len());

            self.commits.push(CommitEntry {
                data: commit.clone(),
                lane: commit_lane,
                color_idx: commit_color,
            });
        }
    }
}

pub struct State {
    subject_width: f32,
    author_width: f32,
    hash_width: f32,
    date_width: f32,
    graph_data: GraphData,
    selected_row: Option<usize>,
    hovered_row: Option<usize>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            subject_width: 440.0,
            author_width: 190.0,
            hash_width: 96.0,
            date_width: 170.0,
            graph_data: GraphData::new(),
            selected_row: None,
            hovered_row: None,
        }
    }
}

pub fn show_cached(
    ui: &mut egui::Ui,
    state: &mut State,
    commit_panel_state: &mut commit_panel::State,
    app_state: &AppState,
) {
    if state.graph_data.commits.is_empty() || app_state.cached_commits.is_empty() {
        state.graph_data.clear();
        if !app_state.cached_commits.is_empty() {
            state.graph_data.add_commits(&app_state.cached_commits);
        }
    }

    let rect = ui.available_rect_before_wrap();
    let (rect, _) = ui.allocate_exact_size(rect.size(), egui::Sense::hover());

    let bg = egui::Color32::from_rgb(31, 31, 31);
    let header_bg = egui::Color32::from_rgb(37, 37, 37);
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 72, 72));

    ui.painter().rect_filled(rect, 0.0, bg);

    let header_rect =
        egui::Rect::from_min_size(rect.left_top(), egui::vec2(rect.width(), HEADER_HEIGHT));
    ui.painter()
        .rect_filled(header_rect.expand2(egui::vec2(0.0, 1.0)), 0.0, header_bg);
    ui.painter().line_segment(
        [header_rect.left_bottom(), header_rect.right_bottom()],
        stroke,
    );

    clamp_columns(state, rect.width());
    let total_width = total_content_width(state).max(rect.width());
    let columns = columns_for(header_rect, state, total_width);
    paint_header(ui, header_rect, &columns, state, stroke);

    let rows_rect = egui::Rect::from_min_max(
        egui::pos2(rect.left(), header_rect.bottom()),
        rect.right_bottom(),
    );

    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt("commit_body_scroll_host")
            .max_rect(rows_rect)
            .layout(egui::Layout::top_down(egui::Align::Min)),
        |ui| {
            egui::ScrollArea::both()
                .id_salt("commit_body_scroll")
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    let content_size = egui::vec2(
                        total_width,
                        state.graph_data.commits.len() as f32 * ROW_HEIGHT,
                    );
                    let (content_rect, _) =
                        ui.allocate_exact_size(content_size, egui::Sense::hover());
                    let columns = columns_for(content_rect, state, total_width);
                    paint_rows(ui, content_rect, &columns, state);
                });
        },
    );

    let show_panel = app_state
        .cached_status
        .as_ref()
        .is_some_and(|s| s.staged_count > 0 || s.unstaged_count > 0);

    if show_panel {
        commit_panel::show_cached(ui, rect, commit_panel_state, app_state);
    }
}

fn clamp_columns(state: &mut State, available_width: f32) {
    state.subject_width = state.subject_width.max(MIN_SUBJECT_WIDTH);
    state.author_width = state.author_width.max(MIN_AUTHOR_WIDTH);
    state.hash_width = state.hash_width.max(MIN_HASH_WIDTH);
    state.date_width = state.date_width.max(MIN_DATE_WIDTH);

    let flexible_width =
        (available_width - graph_width(state) - state.hash_width - state.date_width)
            .max(MIN_SUBJECT_WIDTH + MIN_AUTHOR_WIDTH);
    let current = state.subject_width + state.author_width;
    if current > flexible_width {
        let scale = flexible_width / current;
        state.subject_width = (state.subject_width * scale).max(MIN_SUBJECT_WIDTH);
        state.author_width = (state.author_width * scale).max(MIN_AUTHOR_WIDTH);
    }
}

fn graph_width(state: &State) -> f32 {
    LEFT_PADDING * 2.0 + LANE_WIDTH * state.graph_data.max_lanes.max(4) as f32
}

fn total_content_width(state: &State) -> f32 {
    graph_width(state)
        + state.subject_width
        + state.author_width
        + state.hash_width
        + state.date_width
}

struct Columns {
    graph: egui::Rect,
    subject: egui::Rect,
    author: egui::Rect,
    hash: egui::Rect,
    date: egui::Rect,
}

fn columns_for(rect: egui::Rect, state: &State, total_width: f32) -> Columns {
    let mut left = rect.left();
    let graph = egui::Rect::from_min_size(
        egui::pos2(left, rect.top()),
        egui::vec2(graph_width(state), rect.height()),
    );
    left = graph.right();
    let subject = egui::Rect::from_min_size(
        egui::pos2(left, rect.top()),
        egui::vec2(state.subject_width, rect.height()),
    );
    left = subject.right();
    let author = egui::Rect::from_min_size(
        egui::pos2(left, rect.top()),
        egui::vec2(state.author_width, rect.height()),
    );
    left = author.right();
    let hash = egui::Rect::from_min_size(
        egui::pos2(left, rect.top()),
        egui::vec2(state.hash_width, rect.height()),
    );
    left = hash.right();
    let date = egui::Rect::from_min_size(
        egui::pos2(left, rect.top()),
        egui::vec2(total_width - (left - rect.left()), rect.height()),
    );

    Columns {
        graph,
        subject,
        author,
        hash,
        date,
    }
}

fn paint_header(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    columns: &Columns,
    state: &mut State,
    stroke: egui::Stroke,
) {
    let muted = egui::Color32::from_rgb(188, 188, 188);
    for column in [
        columns.graph,
        columns.subject,
        columns.author,
        columns.hash,
        columns.date,
    ] {
        ui.painter()
            .line_segment([column.right_top(), column.right_bottom()], stroke);
    }

    header_text(ui, columns.graph, "Graph", muted);
    header_text(ui, columns.subject, "Subject", muted);
    header_text(ui, columns.author, "Author", muted);
    header_text(ui, columns.hash, "Hash", muted);
    header_text(ui, columns.date, "Date", muted);

    resize_handle(ui, rect, columns.subject.right(), &mut state.subject_width);
    resize_handle(ui, rect, columns.author.right(), &mut state.author_width);
    resize_handle(ui, rect, columns.hash.right(), &mut state.hash_width);
    resize_handle(ui, rect, columns.date.right(), &mut state.date_width);
}

fn header_text(ui: &egui::Ui, rect: egui::Rect, label: &str, color: egui::Color32) {
    ui.painter().text(
        egui::pos2(rect.left() + 10.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(12.0),
        color,
    );
}

fn resize_handle(ui: &mut egui::Ui, rect: egui::Rect, x: f32, width: &mut f32) {
    let handle = egui::Rect::from_min_max(
        egui::pos2(x - 4.0, rect.top()),
        egui::pos2(x + 4.0, rect.bottom()),
    );
    let response = ui.interact(
        handle,
        ui.make_persistent_id(("commit_column_resize", x.to_bits())),
        egui::Sense::drag(),
    );
    if response.dragged() {
        *width += response.drag_delta().x;
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
}

fn lane_center_x(graph_rect: egui::Rect, lane: usize) -> f32 {
    graph_rect.left() + LEFT_PADDING + lane as f32 * LANE_WIDTH + LANE_WIDTH / 2.0
}

fn row_center_y(content_rect: egui::Rect, row: usize) -> f32 {
    content_rect.top() + row as f32 * ROW_HEIGHT + ROW_HEIGHT / 2.0
}

fn paint_rows(ui: &mut egui::Ui, content_rect: egui::Rect, columns: &Columns, state: &mut State) {
    paint_graph(ui, columns.graph, content_rect, state);

    for (row_idx, entry) in state.graph_data.commits.iter().enumerate() {
        let row_rect = row_rect(content_rect, row_idx);

        let is_selected = state.selected_row == Some(row_idx);
        let is_hovered = state.hovered_row == Some(row_idx);

        if is_selected {
            ui.painter()
                .rect_filled(row_rect, 0.0, egui::Color32::from_rgb(76, 76, 76));
        } else if is_hovered {
            ui.painter()
                .rect_filled(row_rect, 0.0, egui::Color32::from_rgb(48, 48, 48));
        }

        let response = ui.interact(
            row_rect,
            ui.make_persistent_id(("commit_row", row_idx)),
            egui::Sense::click(),
        );

        if response.clicked() {
            state.selected_row = Some(row_idx);
        }

        if response.hovered() {
            state.hovered_row = Some(row_idx);
        } else if state.hovered_row == Some(row_idx) {
            state.hovered_row = None;
        }

        paint_commit_row(ui, row_rect, columns, entry, row_idx, state);
    }
}

fn paint_graph(ui: &egui::Ui, graph_rect: egui::Rect, content_rect: egui::Rect, state: &State) {
    for line in &state.graph_data.lines {
        paint_commit_line(ui, graph_rect, content_rect, line);
    }

    for (row_idx, entry) in state.graph_data.commits.iter().enumerate() {
        let center_x = lane_center_x(graph_rect, entry.lane);
        let center_y = row_center_y(content_rect, row_idx);
        let color = BRANCH_COLORS[entry.color_idx % BRANCH_COLORS.len()];

        draw_commit_circle(ui, center_x, center_y, color);
    }
}

fn paint_commit_line(
    ui: &egui::Ui,
    graph_rect: egui::Rect,
    content_rect: egui::Rect,
    line: &CommitLine,
) {
    let color = BRANCH_COLORS[line.color_idx % BRANCH_COLORS.len()];
    let stroke = egui::Stroke::new(LINE_WIDTH, color);

    let mut current_column = line.child_column;

    for segment in &line.segments {
        match segment {
            CommitLineSegment::Straight { to_row } => {
                let start_x = lane_center_x(graph_rect, current_column);
                let start_y = row_center_y(
                    content_rect,
                    line.full_interval.start.max(to_row.saturating_sub(1)),
                );
                let end_x = lane_center_x(graph_rect, current_column);
                let end_y = row_center_y(content_rect, *to_row);

                ui.painter().line_segment(
                    [egui::pos2(start_x, start_y), egui::pos2(end_x, end_y)],
                    stroke,
                );
            }
            CommitLineSegment::Curve {
                to_column,
                on_row,
                curve_kind,
            } => {
                let start_x = lane_center_x(graph_rect, current_column);
                let start_y = row_center_y(content_rect, line.full_interval.start);
                let end_x = lane_center_x(graph_rect, *to_column);
                let end_y = row_center_y(content_rect, *on_row);

                match curve_kind {
                    CurveKind::Merge | CurveKind::Checkout => {
                        let mid_y = (start_y + end_y) / 2.0;
                        let points = [
                            egui::pos2(start_x, start_y),
                            egui::pos2(start_x, mid_y),
                            egui::pos2(end_x, mid_y),
                            egui::pos2(end_x, end_y),
                        ];
                        let shape = egui::epaint::CubicBezierShape::from_points_stroke(
                            points,
                            false,
                            egui::Color32::TRANSPARENT,
                            stroke,
                        );
                        ui.painter().add(shape);
                    }
                }

                current_column = *to_column;
            }
        }
    }
}

fn draw_commit_circle(ui: &egui::Ui, center_x: f32, center_y: f32, color: egui::Color32) {
    ui.painter().circle_filled(
        egui::pos2(center_x, center_y),
        COMMIT_CIRCLE_RADIUS,
        egui::Color32::from_rgb(31, 31, 31),
    );
    ui.painter().circle_stroke(
        egui::pos2(center_x, center_y),
        COMMIT_CIRCLE_RADIUS,
        egui::Stroke::new(2.0_f32, color),
    );
}

fn paint_commit_row(
    ui: &egui::Ui,
    row: egui::Rect,
    columns: &Columns,
    entry: &CommitEntry,
    row_idx: usize,
    state: &State,
) {
    let text = ui.visuals().text_color();
    let muted = egui::Color32::from_rgb(184, 184, 184);
    let is_selected = state.selected_row == Some(row_idx);
    let date_str = format_commit_date_from_secs(entry.data.timestamp_secs);

    draw_subject_cell(ui, row, columns.subject, entry, is_selected, text, muted);
    draw_author_cell(ui, row, columns.author, entry, text);
    cell_text(ui, columns.hash, row, &entry.data.short_hash, 13.0, text);
    cell_text(ui, columns.date, row, &date_str, 13.0, text);
}

fn draw_subject_cell(
    ui: &egui::Ui,
    row: egui::Rect,
    column: egui::Rect,
    entry: &CommitEntry,
    is_selected: bool,
    text: egui::Color32,
    muted: egui::Color32,
) {
    let cell = row.intersect(column).shrink2(egui::vec2(8.0, 0.0));
    let max_width = cell.width();
    let font_id = egui::FontId::proportional(if is_selected { 14.0 } else { 13.0 });
    let galley = ui
        .painter()
        .layout_no_wrap(entry.data.message.clone(), font_id, text);

    let display_text = if galley.rect.width() > max_width {
        let char_width = galley.rect.width() / entry.data.message.len().max(1) as f32;
        let max_chars = ((max_width - 20.0) / char_width).floor() as usize;
        let truncated = entry
            .data
            .message
            .chars()
            .take(max_chars.saturating_sub(3))
            .collect::<String>();
        format!("{}...", truncated)
    } else {
        entry.data.message.clone()
    };

    clipped_text(
        ui,
        cell,
        egui::pos2(cell.left(), row.center().y),
        &display_text,
        if is_selected { 14.0 } else { 13.0 },
        if is_selected { text } else { muted },
        egui::Align2::LEFT_CENTER,
    );
}

fn draw_author_cell(
    ui: &egui::Ui,
    row: egui::Rect,
    column: egui::Rect,
    entry: &CommitEntry,
    text: egui::Color32,
) {
    let cell = row.intersect(column).shrink2(egui::vec2(8.0, 0.0));
    let avatar = egui::Rect::from_center_size(
        egui::pos2(cell.left() + 9.0, row.center().y),
        egui::vec2(18.0, 18.0),
    );
    let avatar_color = BRANCH_COLORS[entry.color_idx % BRANCH_COLORS.len()];
    ui.painter().rect_filled(avatar, 2.0, avatar_color);

    let initials: String = entry
        .data
        .author
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

    clipped_text(
        ui,
        avatar,
        avatar.center(),
        &initials,
        8.0,
        egui::Color32::WHITE,
        egui::Align2::CENTER_CENTER,
    );
    clipped_text(
        ui,
        egui::Rect::from_min_max(
            egui::pos2(avatar.right() + 8.0, cell.top()),
            cell.right_bottom(),
        ),
        egui::pos2(avatar.right() + 8.0, row.center().y),
        &entry.data.author,
        13.0,
        text,
        egui::Align2::LEFT_CENTER,
    );
}

fn cell_text(
    ui: &egui::Ui,
    column: egui::Rect,
    row: egui::Rect,
    text: &str,
    size: f32,
    color: egui::Color32,
) {
    let cell = row.intersect(column).shrink2(egui::vec2(8.0, 0.0));
    clipped_text(
        ui,
        cell,
        egui::pos2(cell.left(), row.center().y),
        text,
        size,
        color,
        egui::Align2::LEFT_CENTER,
    );
}

fn clipped_text(
    ui: &egui::Ui,
    clip_rect: egui::Rect,
    pos: egui::Pos2,
    text: &str,
    size: f32,
    color: egui::Color32,
    align: egui::Align2,
) {
    let painter = ui.painter().with_clip_rect(clip_rect);
    painter.text(pos, align, text, egui::FontId::proportional(size), color);
}

fn row_rect(content_rect: egui::Rect, index: usize) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(
            content_rect.left(),
            content_rect.top() + index as f32 * ROW_HEIGHT,
        ),
        egui::vec2(content_rect.width(), ROW_HEIGHT),
    )
}

fn format_commit_date_from_secs(secs: i64) -> String {
    let days = secs / 86400;
    let months = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut year = 1970;
    let mut remaining = days;
    loop {
        let days_in_year = if year % 4 == 0 { 366 } else { 365 };
        if remaining < days_in_year {
            break;
        }
        remaining -= days_in_year;
        year += 1;
    }
    let mut month = 0;
    for (i, &days) in months.iter().enumerate() {
        let d = if i == 1 && year % 4 == 0 { 29 } else { days };
        if remaining < d {
            month = i;
            break;
        }
        remaining -= d;
    }
    const MONTH_NAMES: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let day = remaining + 1;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    format!(
        "{} {} {} {:02}:{:02}",
        day, MONTH_NAMES[month], year, hours, mins
    )
}
