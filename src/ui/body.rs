use eframe::egui;
use egui_phosphor::regular::DOTS_SIX_VERTICAL;

use crate::state::AppState;
use crate::ui::commit_panel;

const HEADER_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 24.0;
const GRAPH_WIDTH: f32 = 118.0;
const MIN_SUBJECT_WIDTH: f32 = 300.0;
const MIN_AUTHOR_WIDTH: f32 = 150.0;
const MIN_HASH_WIDTH: f32 = 86.0;
const MIN_DATE_WIDTH: f32 = 150.0;

struct Commit {
    lane: usize,
    message: String,
    author: String,
    initials: String,
    avatar: egui::Color32,
    hash: String,
    date: String,
    selected: bool,
}

struct CommitGraph {
    commits: Vec<Commit>,
}

pub struct State {
    subject_width: f32,
    author_width: f32,
    hash_width: f32,
    date_width: f32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            subject_width: 440.0,
            author_width: 190.0,
            hash_width: 96.0,
            date_width: 170.0,
        }
    }
}

struct Columns {
    graph: egui::Rect,
    subject: egui::Rect,
    author: egui::Rect,
    hash: egui::Rect,
    date: egui::Rect,
}

pub fn show_cached(
    ui: &mut egui::Ui,
    state: &mut State,
    commit_panel_state: &mut commit_panel::State,
    app_state: &AppState,
) {
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

    let graph = if !app_state.cached_commits.is_empty() {
        CommitGraph::from_cached(&app_state.cached_commits)
    } else {
        CommitGraph {
            commits: Vec::new(),
        }
    };

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
                    let content_size =
                        egui::vec2(total_width, graph.commits.len() as f32 * ROW_HEIGHT);
                    let (content_rect, _) =
                        ui.allocate_exact_size(content_size, egui::Sense::hover());
                    let columns = columns_for(content_rect, state, total_width);
                    graph.paint_rows(ui, content_rect, &columns);
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

    let flexible_width = (available_width - GRAPH_WIDTH - state.hash_width - state.date_width)
        .max(MIN_SUBJECT_WIDTH + MIN_AUTHOR_WIDTH);
    let current = state.subject_width + state.author_width;
    if current > flexible_width {
        let scale = flexible_width / current;
        state.subject_width = (state.subject_width * scale).max(MIN_SUBJECT_WIDTH);
        state.author_width = (state.author_width * scale).max(MIN_AUTHOR_WIDTH);
    }
}

fn total_content_width(state: &State) -> f32 {
    GRAPH_WIDTH + state.subject_width + state.author_width + state.hash_width + state.date_width
}

fn columns_for(rect: egui::Rect, state: &State, total_width: f32) -> Columns {
    let mut left = rect.left();
    let graph = egui::Rect::from_min_size(
        egui::pos2(left, rect.top()),
        egui::vec2(GRAPH_WIDTH, rect.height()),
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

impl CommitGraph {
    fn from_cached(commits: &[crate::state::CachedCommit]) -> Self {
        let branch_colors = [
            egui::Color32::from_rgb(255, 165, 16),
            egui::Color32::from_rgb(238, 202, 34),
            egui::Color32::from_rgb(255, 45, 72),
            egui::Color32::from_rgb(151, 113, 73),
            egui::Color32::from_rgb(42, 167, 222),
            egui::Color32::from_rgb(56, 193, 114),
        ];
        let mut lane_map = std::collections::HashMap::new();
        let mut next_lane = 0;

        let commits = commits
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let lane = if c.parents.len() <= 1 {
                    0
                } else {
                    let key = c.hash.clone();
                    let lane = lane_map.entry(key).or_insert_with(|| {
                        let l = next_lane;
                        next_lane += 1;
                        l
                    });
                    *lane
                };

                let initials: String = c
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
                let avatar_color = branch_colors[i % branch_colors.len()];
                let date = format_commit_date_from_secs(c.timestamp_secs);

                Commit {
                    lane,
                    message: c.message.clone(),
                    author: c.author.clone(),
                    initials,
                    avatar: avatar_color,
                    hash: c.short_hash.clone(),
                    date,
                    selected: i == 0,
                }
            })
            .collect();

        Self { commits }
    }

    fn paint_rows(&self, ui: &mut egui::Ui, content_rect: egui::Rect, columns: &Columns) {
        draw_graph(ui, columns.graph, &self.commits);
        for (index, commit) in self.commits.iter().enumerate() {
            let row = row_rect(content_rect, index);
            draw_commit_row(ui, row, columns, commit, index);
        }
    }
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

fn draw_graph(ui: &egui::Ui, rect: egui::Rect, commits: &[Commit]) {
    for lane in 0..4 {
        let Some((first, last)) = lane_extent(commits, lane) else {
            continue;
        };
        let x = lane_x(rect, lane);
        let top = row_center_y(rect, first);
        let bottom = row_center_y(rect, last);
        ui.painter().line_segment(
            [egui::pos2(x, top), egui::pos2(x, bottom)],
            egui::Stroke::new(2.0_f32, lane_color(lane)),
        );
    }
}

fn draw_commit_row(
    ui: &mut egui::Ui,
    row: egui::Rect,
    columns: &Columns,
    commit: &Commit,
    row_index: usize,
) {
    let text = ui.visuals().text_color();
    let muted = egui::Color32::from_rgb(184, 184, 184);
    let selected = egui::Color32::from_rgb(76, 76, 76);

    if commit.selected {
        ui.painter().rect_filled(row, 0.0, selected);
    }
    draw_commit_node(ui, row, columns.graph, commit.lane);
    draw_subject_cell(ui, row, columns.subject, commit, row_index, text, muted);
    draw_author_cell(ui, row, columns.author, commit, text);
    cell_text(ui, columns.hash, row, &commit.hash, 13.0, text);
    cell_text(ui, columns.date, row, &commit.date, 13.0, text);
}

fn draw_subject_cell(
    ui: &mut egui::Ui,
    row: egui::Rect,
    column: egui::Rect,
    commit: &Commit,
    _row_index: usize,
    text: egui::Color32,
    muted: egui::Color32,
) {
    let cell = row.intersect(column).shrink2(egui::vec2(8.0, 0.0));

    clipped_text(
        ui,
        egui::Rect::from_min_max(egui::pos2(cell.left(), cell.top()), cell.right_bottom()),
        egui::pos2(cell.left(), row.center().y),
        &commit.message,
        if commit.selected { 14.0 } else { 13.0 },
        if commit.selected { text } else { muted },
        egui::Align2::LEFT_CENTER,
    );
}

fn draw_author_cell(
    ui: &egui::Ui,
    row: egui::Rect,
    column: egui::Rect,
    commit: &Commit,
    text: egui::Color32,
) {
    let cell = row.intersect(column).shrink2(egui::vec2(8.0, 0.0));
    let avatar = egui::Rect::from_center_size(
        egui::pos2(cell.left() + 9.0, row.center().y),
        egui::vec2(18.0, 18.0),
    );
    ui.painter().rect_filled(avatar, 2.0, commit.avatar);
    clipped_text(
        ui,
        avatar,
        avatar.center(),
        &commit.initials,
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
        &commit.author,
        13.0,
        text,
        egui::Align2::LEFT_CENTER,
    );
}

fn draw_commit_node(ui: &egui::Ui, row: egui::Rect, graph: egui::Rect, lane: usize) {
    let center = egui::pos2(lane_x(graph, lane), row.center().y);
    ui.painter()
        .circle_filled(center, 4.0, egui::Color32::from_rgb(31, 31, 31));
    ui.painter()
        .circle_stroke(center, 4.0, egui::Stroke::new(2.0_f32, lane_color(lane)));

    ui.painter().text(
        egui::pos2(graph.right() - 15.0, row.center().y),
        egui::Align2::CENTER_CENTER,
        DOTS_SIX_VERTICAL,
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgb(120, 120, 120),
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

fn row_rect(rect: egui::Rect, index: usize) -> egui::Rect {
    egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.top() + index as f32 * ROW_HEIGHT),
        egui::vec2(rect.width(), ROW_HEIGHT),
    )
}

fn lane_extent(commits: &[Commit], lane: usize) -> Option<(usize, usize)> {
    let first = commits.iter().position(|commit| commit.lane == lane)?;
    let last = commits.iter().rposition(|commit| commit.lane == lane)?;
    Some((first, last))
}

fn row_center_y(rect: egui::Rect, index: usize) -> f32 {
    rect.top() + index as f32 * ROW_HEIGHT + ROW_HEIGHT * 0.5
}

fn lane_color(lane: usize) -> egui::Color32 {
    match lane {
        0 => egui::Color32::from_rgb(255, 165, 16),
        1 => egui::Color32::from_rgb(238, 202, 34),
        2 => egui::Color32::from_rgb(255, 45, 72),
        _ => egui::Color32::from_rgb(151, 113, 73),
    }
}

fn lane_x(rect: egui::Rect, lane: usize) -> f32 {
    rect.left() + 14.0 + lane as f32 * 18.0
}
