use eframe::egui;
use egui_phosphor::regular::{DOTS_SIX_VERTICAL, GITHUB_LOGO};
use serde::Deserialize;

use crate::git::GitRepo;
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
    branch: Option<BranchLabel>,
    author: String,
    initials: String,
    avatar: egui::Color32,
    hash: String,
    date: String,
    selected: bool,
}

struct BranchLabel {
    text: String,
    color: egui::Color32,
    icon: &'static str,
}

#[derive(Deserialize)]
struct MockCommit {
    lane: usize,
    message: String,
    branch: Option<MockBranch>,
    author: String,
    initials: String,
    avatar: String,
    hash: String,
    date: String,
    selected: bool,
}

#[derive(Deserialize)]
struct MockBranch {
    text: String,
    color: String,
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

pub fn show(
    ui: &mut egui::Ui,
    state: &mut State,
    commit_panel_state: &mut commit_panel::State,
    git_repo: Option<&GitRepo>,
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

    let graph = if let Some(repo) = git_repo {
        if let Ok(commits) = repo.commits(200) {
            CommitGraph::from_git(&commits)
        } else {
            CommitGraph::from_json(MOCK_COMMITS_JSON)
        }
    } else {
        CommitGraph::from_json(MOCK_COMMITS_JSON)
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

    let show_panel = git_repo.is_some_and(|repo| {
        repo.status()
            .map(|s| s.staged_count > 0 || s.unstaged_count > 0)
            .unwrap_or(false)
    });

    if show_panel {
        commit_panel::show(ui, rect, commit_panel_state, git_repo);
    }
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
        CommitGraph::from_json(MOCK_COMMITS_JSON)
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
    fn from_json(json: &str) -> Self {
        let commits = serde_json::from_str::<Vec<MockCommit>>(json)
            .unwrap_or_default()
            .into_iter()
            .map(Commit::from)
            .collect();
        Self { commits }
    }

    fn from_git(commits: &[crate::git::Commit]) -> Self {
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
                let date = format_commit_date(&c.timestamp);

                Commit {
                    lane,
                    message: c.message.clone(),
                    branch: None,
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
                    branch: None,
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

impl From<MockCommit> for Commit {
    fn from(value: MockCommit) -> Self {
        Self {
            lane: value.lane,
            message: value.message,
            branch: value.branch.map(BranchLabel::from),
            author: value.author,
            initials: value.initials,
            avatar: parse_color(&value.avatar),
            hash: value.hash,
            date: value.date,
            selected: value.selected,
        }
    }
}

impl From<MockBranch> for BranchLabel {
    fn from(value: MockBranch) -> Self {
        Self {
            text: value.text,
            color: parse_color(&value.color),
            icon: GITHUB_LOGO,
        }
    }
}

fn parse_color(hex: &str) -> egui::Color32 {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return egui::Color32::from_rgb(128, 128, 128);
    }
    let red = u8::from_str_radix(&hex[0..2], 16).unwrap_or(128);
    let green = u8::from_str_radix(&hex[2..4], 16).unwrap_or(128);
    let blue = u8::from_str_radix(&hex[4..6], 16).unwrap_or(128);
    egui::Color32::from_rgb(red, green, blue)
}

const MOCK_COMMITS_JSON: &str = r##"
[
  {"lane":0,"message":"Add @link jsdoc auto-complete (#43475)","branch":{"text":"master","color":"#ffa510"},"author":"Sang","initials":"S","avatar":"#ffa510","hash":"f9b35cd","date":"2 Apr 2021 02:02","selected":false},
  {"lane":0,"message":"Add @deprecated to tree walk (#43473)","branch":null,"author":"Nathan Shively-Sanc","initials":"NS","avatar":"#5484a6","hash":"c6a2e45","date":"1 Apr 2021 17:42","selected":true},
  {"lane":1,"message":"Use scriptKind in document Registry to distinguish between files Fixes #43462","branch":{"text":"origin/scriptKind","color":"#eeca22"},"author":"Sheetal Nandi","initials":"SN","avatar":"#cf6be0","hash":"0b57f47","date":"1 Apr 2021 03:13","selected":false},
  {"lane":1,"message":"Added BindingElement to isSomeImportDeclaration (#43387)","branch":null,"author":"Armando Aguirre","initials":"AA","avatar":"#f7a84f","hash":"8f8a579","date":"1 Apr 2021 03:18","selected":false},
  {"lane":2,"message":"Update LKG","branch":{"text":"origin/release-4.3","color":"#ff2d48"},"author":"TypeScript Bot","initials":"TB","avatar":"#5dc774","hash":"618b518","date":"1 Apr 2021 01:20","selected":false},
  {"lane":2,"message":"Merge remote-tracking branch 'origin/master' into release-4.3","branch":null,"author":"Daniel Rosenwasser","initials":"DR","avatar":"#f25a5a","hash":"7234457","date":"1 Apr 2021 01:04","selected":false},
  {"lane":2,"message":"Error if assignment after block (#41115)","branch":null,"author":"Wenlu Wang","initials":"WW","avatar":"#856f5b","hash":"62f3ccd","date":"1 Apr 2021 00:57","selected":false},
  {"lane":1,"message":"buckets are keyed with DocumentRegistryBucketKey","branch":null,"author":"Sheetal Nandi","initials":"SN","avatar":"#cf6be0","hash":"d529212","date":"1 Apr 2021 00:42","selected":false},
  {"lane":1,"message":"Test that fails because of change in scriptKind of untitled file","branch":null,"author":"Sheetal Nandi","initials":"SN","avatar":"#cf6be0","hash":"a4300b6","date":"1 Apr 2021 00:33","selected":false},
  {"lane":3,"message":"Only catalog types when tracing (#43446)","branch":{"text":"origin/release-4.2","color":"#977149"},"author":"Andrew Casey","initials":"AC","avatar":"#845d58","hash":"6b33949","date":"1 Apr 2021 00:09","selected":false},
  {"lane":0,"message":"Add inline completion telemetry","branch":null,"author":"Sang","initials":"S","avatar":"#ffa510","hash":"af109bc","date":"31 Mar 2021 23:41","selected":false},
  {"lane":1,"message":"Fix project service cache invalidation","branch":null,"author":"Nathan Shively-Sanc","initials":"NS","avatar":"#5484a6","hash":"41f1c02","date":"31 Mar 2021 22:18","selected":false},
  {"lane":0,"message":"Normalize branch graph lane state","branch":null,"author":"TypeScript Bot","initials":"TB","avatar":"#5dc774","hash":"96d4efa","date":"31 Mar 2021 21:58","selected":false},
  {"lane":2,"message":"Add drag payload scaffolding for branch labels","branch":{"text":"feature/drag-to-merge","color":"#2aa7de"},"author":"Harsh","initials":"H","avatar":"#2aa7de","hash":"d31ac72","date":"31 Mar 2021 20:42","selected":false},
  {"lane":2,"message":"Surface merge conflict preview in graph row","branch":null,"author":"Harsh","initials":"H","avatar":"#2aa7de","hash":"111c9aa","date":"31 Mar 2021 20:20","selected":false},
  {"lane":3,"message":"Add remote lane coloring for origin branches","branch":{"text":"origin/main","color":"#38c172"},"author":"Mira Patel","initials":"MP","avatar":"#38c172","hash":"fa21092","date":"31 Mar 2021 19:16","selected":false},
  {"lane":3,"message":"Use status badges while building file tree","branch":null,"author":"Mira Patel","initials":"MP","avatar":"#38c172","hash":"bf321ea","date":"31 Mar 2021 18:44","selected":false},
  {"lane":1,"message":"Parse unified patch hunks into display lines","branch":{"text":"diff/rendering","color":"#b66dff"},"author":"Omar Rizvi","initials":"OR","avatar":"#b66dff","hash":"9d1a7c8","date":"31 Mar 2021 17:50","selected":false},
  {"lane":1,"message":"Add word-level inline diff spans","branch":null,"author":"Omar Rizvi","initials":"OR","avatar":"#b66dff","hash":"eca24aa","date":"31 Mar 2021 17:23","selected":false},
  {"lane":0,"message":"Read repository config from existing git settings","branch":null,"author":"Sang","initials":"S","avatar":"#ffa510","hash":"2fca0ac","date":"31 Mar 2021 16:05","selected":false},
  {"lane":2,"message":"Make branch rows draggable in the sidebar","branch":{"text":"sidebar/dnd","color":"#ff8c42"},"author":"Daniel Rosenwasser","initials":"DR","avatar":"#f25a5a","hash":"7ad33ab","date":"31 Mar 2021 15:11","selected":false},
  {"lane":0,"message":"Add greedy lane assignment for merge commits","branch":null,"author":"TypeScript Bot","initials":"TB","avatar":"#5dc774","hash":"45e1a90","date":"31 Mar 2021 14:02","selected":false},
  {"lane":3,"message":"Track remote branch tips in graph model","branch":{"text":"origin/graph-lanes","color":"#0db7ed"},"author":"Andrew Casey","initials":"AC","avatar":"#845d58","hash":"88ac013","date":"31 Mar 2021 13:39","selected":false},
  {"lane":3,"message":"Handle octopus merge row spacing","branch":null,"author":"Wenlu Wang","initials":"WW","avatar":"#856f5b","hash":"ce401d9","date":"31 Mar 2021 12:56","selected":false},
  {"lane":1,"message":"Persist selected commit after refresh","branch":null,"author":"Nathan Shively-Sanc","initials":"NS","avatar":"#5484a6","hash":"0db117c","date":"31 Mar 2021 12:04","selected":false},
  {"lane":0,"message":"Initial graph renderer over egui painter","branch":{"text":"palimpsest/root","color":"#ffa510"},"author":"Harsh","initials":"H","avatar":"#2aa7de","hash":"b00b1e5","date":"31 Mar 2021 11:42","selected":false}
]
"##;

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

    for index in [5, 8, 18, 27, 38] {
        if index < commits.len() {
            let y = row_center_y(rect, index);
            ui.painter().line_segment(
                [
                    egui::pos2(lane_x(rect, 0), y),
                    egui::pos2(lane_x(rect, 3), y),
                ],
                egui::Stroke::new(2.0_f32, egui::Color32::from_rgb(255, 165, 16)),
            );
        }
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
    row_index: usize,
    text: egui::Color32,
    muted: egui::Color32,
) {
    let cell = row.intersect(column).shrink2(egui::vec2(8.0, 0.0));
    let mut x = cell.left();
    if let Some(branch) = &commit.branch {
        let label_width =
            (branch.text.len() as f32 * 7.5 + 34.0).clamp(82.0, (cell.width() * 0.55).max(82.0));
        let label_rect = egui::Rect::from_min_size(
            egui::pos2(x, row.center().y - 9.0),
            egui::vec2(label_width, 18.0),
        );
        branch_label(ui, label_rect, branch, row_index);
        x = label_rect.right() + 8.0;
    }

    clipped_text(
        ui,
        egui::Rect::from_min_max(egui::pos2(x, cell.top()), cell.right_bottom()),
        egui::pos2(x, row.center().y),
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

fn branch_label(ui: &mut egui::Ui, rect: egui::Rect, branch: &BranchLabel, row_index: usize) {
    let response = ui.interact(
        rect,
        ui.make_persistent_id(("branch_label", row_index, &branch.text)),
        egui::Sense::click_and_drag(),
    );
    let fill = if response.hovered() {
        branch.color.gamma_multiply(0.78)
    } else {
        branch.color.gamma_multiply(0.58)
    };
    ui.painter().rect_filled(rect, 2.0, fill);
    ui.painter().rect_stroke(
        rect,
        2.0,
        egui::Stroke::new(1.0_f32, branch.color),
        egui::StrokeKind::Inside,
    );
    if response.dragged() {
        ui.painter().rect_stroke(
            rect.expand(3.0),
            2.0,
            egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(120, 120, 120)),
            egui::StrokeKind::Inside,
        );
    }
    clipped_text(
        ui,
        rect,
        egui::pos2(rect.left() + 15.0, rect.center().y),
        branch.icon,
        13.0,
        egui::Color32::from_rgb(220, 220, 220),
        egui::Align2::CENTER_CENTER,
    );
    clipped_text(
        ui,
        rect.shrink2(egui::vec2(26.0, 0.0)),
        egui::pos2(rect.left() + 28.0, rect.center().y),
        &branch.text,
        12.0,
        egui::Color32::WHITE,
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

fn format_commit_date(time: &std::time::SystemTime) -> String {
    let duration = time
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = duration.as_secs();
    format_commit_date_from_secs(secs as i64)
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
