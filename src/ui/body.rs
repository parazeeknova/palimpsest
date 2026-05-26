use eframe::egui;
use egui_phosphor::regular::{BOOKMARK, CHECK, GIT_BRANCH, PENCIL_SIMPLE, TAG};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use crate::git::GitRepo;
use crate::git::live::RepoLiveEvent;
use crate::state::{AppState, CachedBranch, CachedCommit, CachedTag};
use crate::ui::commit_drawer;
use crate::ui::commit_panel;

const HEADER_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 28.0;
const REFS_GUTTER_WIDTH: f32 = 154.0;
const LANE_WIDTH: f32 = 16.0;
const LEFT_PADDING: f32 = 12.0;
const LINE_WIDTH: f32 = 1.5;
const COMMIT_CIRCLE_RADIUS: f32 = 3.5;
const WIP_NODE_RADIUS: f32 = COMMIT_CIRCLE_RADIUS + 0.85;
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

#[derive(Clone, Debug)]
enum RefKind {
    Branch,
    Tag,
    Release,
}

#[derive(Clone, Debug)]
struct RefBadge {
    row: Option<usize>,
    label: String,
    kind: RefKind,
    highlighted: bool,
    connect_to_graph: bool,
}

#[derive(Clone, Debug)]
struct TopStatusRow {
    label: String,
    detail: String,
    graph_lane: Option<usize>,
    color_idx: Option<usize>,
    show_ref_chip: bool,
    show_graph_node: bool,
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct RefsFingerprint {
    branches_len: usize,
    first_branch_name: Option<String>,
    tags_len: usize,
    first_tag_name: Option<String>,
    releases_len: usize,
    first_release_name: Option<String>,
    status_branch: Option<String>,
    status_staged_count: usize,
    status_unstaged_count: usize,
}

pub struct State {
    refs_width: f32,
    subject_width: f32,
    author_width: f32,
    hash_width: f32,
    date_width: f32,
    graph_data: GraphData,
    selected_row: Option<usize>,
    hovered_row: Option<usize>,
    branch_refs: Vec<RefBadge>,
    tag_refs: Vec<RefBadge>,
    top_status_row: Option<TopStatusRow>,
    pub selected_commit_hash: Option<String>,
    selected_commit_cache_hash: Option<String>,
    selected_commit_cache_populated_with_repo: bool,
    selected_commit_cache_repo: Option<String>,
    pub selected_commit_cache: Option<commit_drawer::CommitDrawerCommit>,
    pub selected_commit_signature_cache: Option<commit_drawer::CommitDrawerSignature>,
    pub selected_commit_files_cache: Vec<crate::git::models::FileStatus>,
    drawer_state: commit_drawer::State,
    refs_fingerprint: Option<RefsFingerprint>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            refs_width: REFS_GUTTER_WIDTH,
            subject_width: 360.0,
            author_width: 190.0,
            hash_width: 96.0,
            date_width: 170.0,
            graph_data: GraphData::new(),
            selected_row: None,
            hovered_row: None,
            branch_refs: Vec::new(),
            tag_refs: Vec::new(),
            top_status_row: None,
            selected_commit_hash: None,
            selected_commit_cache_hash: None,
            selected_commit_cache_populated_with_repo: false,
            selected_commit_cache_repo: None,
            selected_commit_cache: None,
            selected_commit_signature_cache: None,
            selected_commit_files_cache: Vec::new(),
            drawer_state: commit_drawer::State::default(),
            refs_fingerprint: None,
        }
    }
}

impl State {
    fn refresh_refs(&mut self, app_state: &AppState) {
        self.branch_refs = build_branch_refs(&self.graph_data, &app_state.cached_branches);
        self.tag_refs = build_tag_refs(&self.graph_data, &app_state.cached_tags);
        let release_badges = build_release_refs(app_state, &self.graph_data);
        self.tag_refs.extend(release_badges);
        self.top_status_row = build_top_status_row(app_state, &self.graph_data);
    }
}

fn commit_row_for_hash(graph_data: &GraphData, hash: &str) -> Option<usize> {
    graph_data
        .commits
        .iter()
        .enumerate()
        .find_map(|(idx, entry)| {
            (entry.data.hash.starts_with(hash) || entry.data.short_hash == hash).then_some(idx)
        })
}

impl State {
    fn refresh_selected_commit_cache(
        &mut self,
        app_state: &AppState,
        git_repo: Option<&GitRepo>,
        repo_live_tx: &std::sync::mpsc::Sender<RepoLiveEvent>,
        ctx: &egui::Context,
    ) {
        let Some(hash) = self.selected_commit_hash.as_deref() else {
            self.selected_commit_cache_hash = None;
            self.selected_commit_cache = None;
            self.selected_commit_signature_cache = None;
            self.selected_commit_files_cache.clear();
            self.selected_commit_cache_populated_with_repo = false;
            self.selected_commit_cache_repo = None;
            return;
        };

        if self.selected_commit_cache_hash.as_deref() == Some(hash)
            && self.selected_commit_cache_populated_with_repo == git_repo.is_some()
            && self.selected_commit_cache_repo == app_state.current_repo
        {
            return;
        }

        let Some(commit) = app_state.cached_commits.iter().find(|c| c.hash == hash) else {
            self.selected_commit_hash = None;
            self.selected_commit_cache_hash = None;
            self.selected_commit_cache = None;
            self.selected_commit_signature_cache = None;
            self.selected_commit_files_cache.clear();
            self.selected_commit_cache_populated_with_repo = false;
            self.selected_commit_cache_repo = None;
            self.selected_row = None;
            return;
        };

        // Populate lightweight details synchronously
        self.selected_commit_cache_hash = Some(hash.to_string());
        self.selected_commit_cache_populated_with_repo = git_repo.is_some();
        self.selected_commit_cache_repo = app_state.current_repo.clone();
        self.selected_commit_cache = Some(commit_drawer::CommitDrawerCommit {
            hash: commit.hash.clone(),
            short_hash: commit.short_hash.clone(),
            message: commit.message.clone(),
            author: commit.author.clone(),
            email: String::new(),
            timestamp: crate::ui::repo_manager::format_relative_time(commit.timestamp_secs),
            timestamp_exact: format_commit_date_from_secs(commit.timestamp_secs),
            parents: commit.parents.clone(),
            populated: false,
        });
        self.selected_commit_signature_cache = None;
        self.selected_commit_files_cache.clear();

        if git_repo.is_some() {
            let Some(repo_path) = app_state.current_repo.clone() else {
                return;
            };
            let hash = hash.to_string();
            let repo_live_tx = repo_live_tx.clone();
            let ctx = ctx.clone();

            std::thread::spawn(move || {
                if let Ok(repo) = crate::git::repo::GitRepo::open(&repo_path) {
                    let email = repo
                        .commit_by_hash(&hash)
                        .ok()
                        .map(|c| c.email.clone())
                        .unwrap_or_default();
                    let signature = repo.commit_signature_info(&hash).ok().flatten();
                    let files = repo.commit_files(&hash).unwrap_or_default();

                    let _ = repo_live_tx.send(RepoLiveEvent::CommitDetails {
                        path: repo_path,
                        hash,
                        email,
                        signature,
                        files,
                    });
                    ctx.request_repaint();
                }
            });
        }
    }
}

fn build_branch_refs(graph_data: &GraphData, branches: &[CachedBranch]) -> Vec<RefBadge> {
    let mut refs = Vec::new();

    for branch in branches.iter().filter(|branch| branch.is_current) {
        refs.push(RefBadge {
            label: branch.name.clone(),
            kind: RefKind::Branch,
            highlighted: true,
            row: commit_row_for_hash(graph_data, branch.tip_hash.as_str()),
            connect_to_graph: true,
        });
    }

    for branch in branches
        .iter()
        .filter(|branch| !branch.is_current && !branch.is_remote)
    {
        refs.push(RefBadge {
            label: branch.name.clone(),
            kind: RefKind::Branch,
            highlighted: false,
            row: commit_row_for_hash(graph_data, branch.tip_hash.as_str()),
            connect_to_graph: true,
        });
    }

    refs
}

fn build_tag_refs(graph_data: &GraphData, tags: &[CachedTag]) -> Vec<RefBadge> {
    tags.iter()
        .map(|tag| RefBadge {
            label: tag.name.clone(),
            kind: RefKind::Tag,
            highlighted: false,
            row: commit_row_for_hash(graph_data, tag.target_hash.as_str()),
            connect_to_graph: true,
        })
        .collect()
}

fn build_release_refs(app_state: &AppState, graph_data: &GraphData) -> Vec<RefBadge> {
    app_state
        .github_releases
        .iter()
        .map(|release| {
            let target_hash = app_state
                .cached_tags
                .iter()
                .find(|tag| tag.name == release.tag_name)
                .map(|tag| tag.target_hash.as_str())
                .unwrap_or("");

            RefBadge {
                label: release
                    .name
                    .clone()
                    .unwrap_or_else(|| release.tag_name.clone()),
                kind: RefKind::Release,
                highlighted: false,
                row: if target_hash.is_empty() {
                    None
                } else {
                    commit_row_for_hash(graph_data, target_hash)
                },
                connect_to_graph: true,
            }
        })
        .filter(|badge| badge.row.is_some())
        .collect()
}

fn build_top_status_row(app_state: &AppState, graph_data: &GraphData) -> Option<TopStatusRow> {
    let status = app_state.cached_status.as_ref()?;
    if status.staged_count == 0 && status.unstaged_count == 0 {
        return None;
    }

    let (graph_lane, color_idx) = app_state
        .cached_branches
        .iter()
        .find(|branch| branch.is_current)
        .and_then(|branch| {
            commit_row_for_hash(graph_data, branch.tip_hash.as_str()).map(|row| {
                let entry = &graph_data.commits[row];
                (Some(entry.lane), Some(entry.color_idx))
            })
        })
        .unwrap_or((None, None));

    let detail = match (status.staged_count, status.unstaged_count) {
        (staged, unstaged) if staged > 0 && unstaged > 0 => {
            format!("{} staged, {} unstaged", staged, unstaged)
        }
        (staged, _) if staged > 0 => format!("{} staged", staged),
        (_, unstaged) => format!("{} unstaged", unstaged),
    };

    Some(TopStatusRow {
        label: "WIP".to_string(),
        detail,
        graph_lane,
        color_idx,
        show_ref_chip: status.unstaged_count == 0,
        show_graph_node: status.unstaged_count > 0,
    })
}

pub fn show_cached(
    ui: &mut egui::Ui,
    state: &mut State,
    commit_panel_state: &mut commit_panel::State,
    app_state: &AppState,
    git_repo: Option<&GitRepo>,
    repo_live_tx: &std::sync::mpsc::Sender<RepoLiveEvent>,
) {
    let graph_commits_changed = state.graph_data.commits.len() != app_state.cached_commits.len()
        || state
            .graph_data
            .commits
            .iter()
            .zip(app_state.cached_commits.iter())
            .any(|(entry, cached)| entry.data.hash != cached.hash);

    if graph_commits_changed {
        state.graph_data.clear();
        if !app_state.cached_commits.is_empty() {
            state.graph_data.add_commits(&app_state.cached_commits);
        }
        if let Some(ref hash) = state.selected_commit_hash {
            let found_idx = state
                .graph_data
                .commits
                .iter()
                .position(|entry| entry.data.hash == *hash);
            if let Some(idx) = found_idx {
                let limit = state.graph_data.commits.len().saturating_sub(1);
                state.selected_row = Some(idx.min(limit));
            } else {
                state.selected_row = None;
                state.selected_commit_hash = None;
            }
        } else {
            state.selected_row = None;
        }
    }

    let current_refs_fp = RefsFingerprint {
        branches_len: app_state.cached_branches.len(),
        first_branch_name: app_state.cached_branches.first().map(|b| b.name.clone()),
        tags_len: app_state.cached_tags.len(),
        first_tag_name: app_state.cached_tags.first().map(|t| t.name.clone()),
        releases_len: app_state.github_releases.len(),
        first_release_name: app_state
            .github_releases
            .first()
            .and_then(|r| r.name.clone()),
        status_branch: app_state.cached_status.as_ref().map(|s| s.branch.clone()),
        status_staged_count: app_state
            .cached_status
            .as_ref()
            .map(|s| s.staged_count)
            .unwrap_or(0),
        status_unstaged_count: app_state
            .cached_status
            .as_ref()
            .map(|s| s.unstaged_count)
            .unwrap_or(0),
    };

    let refs_changed =
        graph_commits_changed || state.refs_fingerprint.as_ref() != Some(&current_refs_fp);

    if refs_changed {
        state.refresh_refs(app_state);
        state.refs_fingerprint = Some(current_refs_fp);
    }

    let rect = ui.available_rect_before_wrap();
    let (rect, _) = ui.allocate_exact_size(rect.size(), egui::Sense::hover());

    let bg = egui::Color32::from_rgb(31, 31, 31);
    let header_bg = egui::Color32::from_rgb(37, 37, 37);
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 72, 72));

    ui.painter().rect_filled(rect, 0.0, bg);

    let mut drawer_height = 0.0;

    if app_state.cached_commits.is_empty() {
        state.selected_commit_hash = None;
        state.selected_commit_cache_hash = None;
        state.selected_commit_cache = None;
        state.selected_commit_signature_cache = None;
        state.selected_commit_files_cache.clear();
        state.selected_commit_cache_populated_with_repo = false;
        state.selected_commit_cache_repo = None;
        state.selected_row = None;

        let logo = egui::Image::new(egui::include_image!("../assets/logo.svg"))
            .tint(egui::Color32::from_white_alpha(40))
            .fit_to_exact_size(egui::vec2(200.0, 200.0));
        let logo_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(200.0, 200.0));
        ui.put(logo_rect, logo);
    } else {
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

        drawer_height = if state.selected_commit_hash.is_some() {
            state.drawer_state.height.clamp(0.0, rows_rect.height())
        } else {
            0.0
        };
        let list_rect = if drawer_height > 0.0 {
            egui::Rect::from_min_max(
                rows_rect.left_top(),
                egui::pos2(rows_rect.right(), rows_rect.bottom() - drawer_height),
            )
        } else {
            rows_rect
        };

        ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt("commit_body_scroll_host")
                .max_rect(list_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| {
                egui::ScrollArea::both()
                    .id_salt("commit_body_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let extra_row_height = if state.top_status_row.is_some() {
                            ROW_HEIGHT
                        } else {
                            0.0
                        };
                        let content_size = egui::vec2(
                            total_width,
                            state.graph_data.commits.len() as f32 * ROW_HEIGHT + extra_row_height,
                        );
                        let (content_rect, _) =
                            ui.allocate_exact_size(content_size, egui::Sense::hover());
                        let columns = columns_for(content_rect, state, total_width);
                        paint_rows(ui, content_rect, &columns, state, app_state);
                    });
            },
        );

        state.refresh_selected_commit_cache(app_state, git_repo, repo_live_tx, ui.ctx());

        if let Some(selected) = state.selected_commit_cache.as_ref() {
            let drawer_rect = egui::Rect::from_min_max(
                egui::pos2(rows_rect.left(), rows_rect.bottom() - drawer_height),
                rows_rect.right_bottom(),
            );
            commit_drawer::show(
                ui,
                drawer_rect,
                &mut state.drawer_state,
                app_state,
                Some(selected),
                state.selected_commit_signature_cache.as_ref(),
                &state.selected_commit_files_cache,
            );
        }
    }

    let show_panel = app_state
        .cached_status
        .as_ref()
        .is_some_and(|s| s.staged_count > 0 || s.unstaged_count > 0);

    if show_panel {
        let bottom_offset = if state.selected_commit_hash.is_some() {
            drawer_height
        } else {
            0.0
        };
        commit_panel::show_cached_with_bottom_offset(
            ui,
            rect,
            bottom_offset,
            commit_panel_state,
            app_state,
        );
    }
}

fn clamp_columns(state: &mut State, available_width: f32) {
    state.refs_width = state.refs_width.max(REFS_GUTTER_WIDTH);
    state.subject_width = state.subject_width.max(MIN_SUBJECT_WIDTH);
    state.author_width = state.author_width.max(MIN_AUTHOR_WIDTH);
    state.hash_width = state.hash_width.max(MIN_HASH_WIDTH);
    state.date_width = state.date_width.max(MIN_DATE_WIDTH);

    let flexible_width = (available_width
        - graph_width(state)
        - state.refs_width
        - state.hash_width
        - state.date_width)
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
    state.refs_width
        + graph_width(state)
        + state.subject_width
        + state.author_width
        + state.hash_width
        + state.date_width
}

struct Columns {
    graph: egui::Rect,
    refs: egui::Rect,
    subject: egui::Rect,
    author: egui::Rect,
    hash: egui::Rect,
    date: egui::Rect,
}

fn columns_for(rect: egui::Rect, state: &State, total_width: f32) -> Columns {
    let mut left = rect.left();
    let refs = egui::Rect::from_min_size(
        egui::pos2(left, rect.top()),
        egui::vec2(state.refs_width, rect.height()),
    );
    left = refs.right();
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
        refs,
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
        columns.refs,
        columns.graph,
        columns.subject,
        columns.author,
        columns.hash,
        columns.date,
    ] {
        ui.painter()
            .line_segment([column.right_top(), column.right_bottom()], stroke);
    }

    header_text(ui, columns.refs, "Refs", muted);
    header_text(ui, columns.graph, "Graph", muted);
    header_text(ui, columns.subject, "Subject", muted);
    header_text(ui, columns.author, "Author", muted);
    header_text(ui, columns.hash, "Hash", muted);
    header_text(ui, columns.date, "Date", muted);

    resize_handle(ui, rect, columns.graph.right(), &mut state.refs_width);
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

fn paint_rows(
    ui: &mut egui::Ui,
    content_rect: egui::Rect,
    columns: &Columns,
    state: &mut State,
    app_state: &AppState,
) {
    let has_top_row = state.top_status_row.is_some();
    let row_offset = usize::from(has_top_row);

    if let Some(top_status_row) = &state.top_status_row {
        paint_top_status_row(ui, content_rect, columns, top_status_row);
    }

    for (row_idx, entry) in state.graph_data.commits.iter().enumerate() {
        let row_rect = row_rect(content_rect, row_idx, has_top_row);

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
            state.selected_commit_hash = Some(entry.data.hash.clone());
            state.drawer_state.tab = commit_drawer::CommitDrawerTab::Commit;
        }

        if response.hovered() {
            state.hovered_row = Some(row_idx);
        } else if state.hovered_row == Some(row_idx) {
            state.hovered_row = None;
        }

        paint_commit_row(ui, row_rect, columns, entry, row_idx, state, app_state);
    }

    paint_graph(ui, columns.graph, content_rect, state, row_offset);

    let mut refs = Vec::with_capacity(state.branch_refs.len() + state.tag_refs.len());
    refs.extend(state.branch_refs.iter().cloned());
    refs.extend(state.tag_refs.iter().cloned());

    paint_ref_badges(
        ui,
        content_rect,
        columns,
        &state.graph_data,
        &refs,
        has_top_row,
    );
}

fn paint_top_status_row(
    ui: &mut egui::Ui,
    content_rect: egui::Rect,
    columns: &Columns,
    top_status_row: &TopStatusRow,
) {
    let row = row_rect(content_rect, 0, false);
    ui.painter()
        .rect_filled(row, 0.0, egui::Color32::from_rgb(42, 42, 42));

    if top_status_row.show_graph_node {
        if let (Some(lane), Some(color_idx)) = (top_status_row.graph_lane, top_status_row.color_idx)
        {
            let graph_center_x = lane_center_x(columns.graph, lane);
            let graph_center_y = row.center().y;
            let color = BRANCH_COLORS[color_idx % BRANCH_COLORS.len()];
            draw_wip_connector(
                ui,
                graph_center_x,
                graph_center_y + WIP_NODE_RADIUS,
                graph_center_y + ROW_HEIGHT - WIP_NODE_RADIUS,
                color,
            );
            draw_dotted_commit_node(ui, graph_center_x, graph_center_y, color);
        }
    }

    if top_status_row.show_ref_chip {
        let refs_rect = row.intersect(columns.refs).shrink2(egui::vec2(6.0, 4.0));
        let accent = egui::Color32::from_rgb(252, 197, 34);
        let chip_height = 14.0;
        let chip_rect = egui::Rect::from_min_size(
            egui::pos2(refs_rect.left(), refs_rect.top()),
            egui::vec2(48.0, chip_height),
        );
        ui.painter()
            .rect_filled(chip_rect, 3.0, accent.linear_multiply(0.25));
        ui.painter().rect_stroke(
            chip_rect,
            3.0,
            egui::Stroke::new(1.0_f32, accent),
            egui::StrokeKind::Inside,
        );
        clipped_text(
            ui,
            chip_rect.shrink2(egui::vec2(4.0, 0.0)),
            egui::pos2(chip_rect.left() + 4.0, chip_rect.center().y),
            PENCIL_SIMPLE,
            8.0,
            accent,
            egui::Align2::LEFT_CENTER,
        );
        clipped_text(
            ui,
            chip_rect.shrink2(egui::vec2(4.0, 0.0)),
            egui::pos2(chip_rect.left() + 14.0, chip_rect.center().y),
            &top_status_row.label,
            9.0,
            ui.visuals().text_color(),
            egui::Align2::LEFT_CENTER,
        );
    }

    let subject_rect = row.intersect(columns.subject).shrink2(egui::vec2(8.0, 0.0));
    clipped_text(
        ui,
        subject_rect,
        egui::pos2(subject_rect.left(), row.center().y),
        &top_status_row.detail,
        13.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );
}

fn draw_dotted_commit_node(ui: &egui::Ui, center_x: f32, center_y: f32, color: egui::Color32) {
    let radius = WIP_NODE_RADIUS;
    let ring_stroke = egui::Stroke::new(1.2_f32, color);
    ui.painter().circle_filled(
        egui::pos2(center_x, center_y),
        radius + 0.15,
        egui::Color32::from_rgb(42, 42, 42),
    );
    ui.painter()
        .circle_stroke(egui::pos2(center_x, center_y), radius, ring_stroke);

    let dot_radius = 0.35_f32;
    let dot_count = 24;
    for idx in 0..dot_count {
        let angle = idx as f32 / dot_count as f32 * std::f32::consts::TAU;
        let x = center_x + angle.cos() * (radius - 0.18);
        let y = center_y + angle.sin() * (radius - 0.18);
        ui.painter()
            .circle_filled(egui::pos2(x, y), dot_radius, color);
    }
}

fn draw_wip_connector(
    ui: &egui::Ui,
    center_x: f32,
    top_y: f32,
    bottom_y: f32,
    color: egui::Color32,
) {
    let stroke = egui::Stroke::new(1.0_f32, color);
    let dash = 2.0;
    let gap = 1.0;
    let mut y = top_y;

    while y < bottom_y {
        let y2 = (y + dash).min(bottom_y);
        ui.painter()
            .line_segment([egui::pos2(center_x, y), egui::pos2(center_x, y2)], stroke);
        y = y2 + gap;
    }
}

fn paint_ref_badges(
    ui: &mut egui::Ui,
    content_rect: egui::Rect,
    columns: &Columns,
    graph_data: &GraphData,
    refs: &[RefBadge],
    has_top_row: bool,
) {
    let mut refs_by_row: BTreeMap<usize, Vec<RefBadge>> = BTreeMap::new();
    for badge in refs {
        let Some(row) = badge.row else {
            continue;
        };

        refs_by_row.entry(row).or_default().push(badge.clone());
    }

    for (row_idx, mut badges) in refs_by_row {
        let row = row_rect(content_rect, row_idx, has_top_row);
        let refs_rect = row.intersect(columns.refs).shrink2(egui::vec2(6.0, 4.0));
        let chip_height = 14.0;

        if badges.is_empty() {
            continue;
        }

        badges.sort_by(compare_badges_for_display);

        let primary_badge = badges[0].clone();
        let primary_width = chip_width_for_badge(&primary_badge, refs_rect.width());
        let primary_rect = egui::Rect::from_min_size(
            egui::pos2(refs_rect.left(), refs_rect.center().y - chip_height * 0.5),
            egui::vec2(primary_width, chip_height),
        );

        let hover_pos = ui.input(|input| input.pointer.hover_pos());
        let is_hovered = hover_pos
            .map(|pos| primary_rect.contains(pos))
            .unwrap_or(false);

        let mut visible_chips = vec![(primary_badge.clone(), primary_rect)];

        if is_hovered && badges.len() > 1 {
            let mut stack_top = primary_rect.top() - 2.0;
            for badge in badges.iter().skip(1).cloned() {
                let width = chip_width_for_badge(&badge, refs_rect.width());
                stack_top -= chip_height + 2.0;
                visible_chips.push((
                    badge,
                    egui::Rect::from_min_size(
                        egui::pos2(refs_rect.left(), stack_top),
                        egui::vec2(width, chip_height),
                    ),
                ));
            }
        }

        if let Some((badge, chip_rect)) = visible_chips.first() {
            if badge.connect_to_graph {
                paint_ref_connector(ui, columns.graph, *chip_rect, badge, graph_data);
            }
        }

        for (badge, chip_rect) in visible_chips.into_iter().rev() {
            paint_ref_chip(ui, chip_rect, &badge, color_for_ref(&badge));
        }
    }
}

fn chip_width_for_badge(badge: &RefBadge, max_width: f32) -> f32 {
    let base = match badge.kind {
        RefKind::Branch => 20.0,
        RefKind::Tag => 18.0,
        RefKind::Release => 18.0,
    };
    (badge.label.len() as f32 * 5.4 + base).clamp(48.0, max_width)
}

fn compare_badges_for_display(left: &RefBadge, right: &RefBadge) -> std::cmp::Ordering {
    match (&left.kind, &right.kind) {
        (RefKind::Branch, RefKind::Tag) => std::cmp::Ordering::Less,
        (RefKind::Branch, RefKind::Release) => std::cmp::Ordering::Less,
        (RefKind::Tag, RefKind::Branch) => std::cmp::Ordering::Greater,
        (RefKind::Release, RefKind::Branch) => std::cmp::Ordering::Greater,
        (RefKind::Tag, RefKind::Release) => std::cmp::Ordering::Less,
        (RefKind::Release, RefKind::Tag) => std::cmp::Ordering::Greater,
        (RefKind::Branch, RefKind::Branch) => right.highlighted.cmp(&left.highlighted),
        (RefKind::Tag, RefKind::Tag) => compare_tags_for_display(left, right),
        (RefKind::Release, RefKind::Release) => compare_tags_for_display(left, right),
    }
}

fn compare_tags_for_display(left: &RefBadge, right: &RefBadge) -> std::cmp::Ordering {
    let left_version = parsed_tag_version(&left.label);
    let right_version = parsed_tag_version(&right.label);

    match (left_version, right_version) {
        (Some(left_version), Some(right_version)) => right_version.cmp(&left_version),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.label.cmp(&right.label),
    }
}

fn parsed_tag_version(label: &str) -> Option<(u64, u64, u64)> {
    let stripped = label.strip_prefix('v').unwrap_or(label);
    let mut parts = stripped.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn paint_ref_connector(
    ui: &egui::Ui,
    graph_rect: egui::Rect,
    chip_rect: egui::Rect,
    badge: &RefBadge,
    graph_data: &GraphData,
) {
    let Some(row_idx) = badge.row else {
        return;
    };

    let Some(commit_entry) = graph_data.commits.get(row_idx) else {
        return;
    };

    let connector_color = color_for_ref(badge).linear_multiply(0.4);
    let commit_x = lane_center_x(graph_rect, commit_entry.lane);
    let row_center = (chip_rect.center().y * 2.0).round() / 2.0;
    let start_x = chip_rect.right() + 2.0;
    let end_x = if chip_rect.center().x <= commit_x {
        commit_x - COMMIT_CIRCLE_RADIUS - 1.25
    } else {
        commit_x + COMMIT_CIRCLE_RADIUS + 1.25
    };

    if end_x <= start_x {
        return;
    }

    ui.painter().line_segment(
        [
            egui::pos2(start_x, row_center),
            egui::pos2(end_x, row_center),
        ],
        egui::Stroke::new(1.0_f32, connector_color),
    );
}

fn paint_ref_chip(ui: &mut egui::Ui, rect: egui::Rect, badge: &RefBadge, accent: egui::Color32) {
    let fill = if badge.highlighted {
        accent.linear_multiply(0.25)
    } else {
        egui::Color32::from_rgb(52, 52, 52)
    };
    ui.painter().rect_filled(rect, 3.0, fill);
    ui.painter().rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0_f32, accent),
        egui::StrokeKind::Inside,
    );

    let icon = match badge.kind {
        RefKind::Branch if badge.highlighted => CHECK,
        RefKind::Branch => GIT_BRANCH,
        RefKind::Tag => TAG,
        RefKind::Release => BOOKMARK,
    };

    clipped_text(
        ui,
        rect.shrink2(egui::vec2(4.0, 0.0)),
        egui::pos2(rect.left() + 4.0, rect.center().y),
        icon,
        8.0,
        accent,
        egui::Align2::LEFT_CENTER,
    );

    clipped_text(
        ui,
        rect.shrink2(egui::vec2(4.0, 0.0)),
        egui::pos2(rect.left() + 14.0, rect.center().y),
        &badge.label,
        9.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );

    if badge.highlighted {
        let pencil_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 10.0, rect.center().y - 4.0),
            egui::vec2(8.0, 8.0),
        );
        clipped_text(
            ui,
            pencil_rect,
            pencil_rect.center(),
            PENCIL_SIMPLE,
            7.0,
            accent,
            egui::Align2::CENTER_CENTER,
        );
    }
}

fn color_for_ref(badge: &RefBadge) -> egui::Color32 {
    match badge.kind {
        RefKind::Branch => egui::Color32::from_rgb(76, 167, 255),
        RefKind::Tag => egui::Color32::from_rgb(173, 132, 255),
        RefKind::Release => egui::Color32::from_rgb(255, 110, 180), // Sleek pink/red for Releases
    }
}

fn paint_graph(
    ui: &egui::Ui,
    graph_rect: egui::Rect,
    content_rect: egui::Rect,
    state: &State,
    row_offset: usize,
) {
    for line in &state.graph_data.lines {
        paint_commit_line(ui, graph_rect, content_rect, line, row_offset);
    }

    for (row_idx, entry) in state.graph_data.commits.iter().enumerate() {
        let center_x = lane_center_x(graph_rect, entry.lane);
        let center_y = row_center_y(content_rect, row_idx + row_offset);
        let color = BRANCH_COLORS[entry.color_idx % BRANCH_COLORS.len()];

        draw_commit_circle(ui, center_x, center_y, color);
    }
}

fn paint_commit_line(
    ui: &egui::Ui,
    graph_rect: egui::Rect,
    content_rect: egui::Rect,
    line: &CommitLine,
    row_offset: usize,
) {
    let color = BRANCH_COLORS[line.color_idx % BRANCH_COLORS.len()];
    let stroke = egui::Stroke::new(LINE_WIDTH, color);

    let mut current_column = line.child_column;
    let mut current_y =
        row_center_y(content_rect, line.full_interval.start + row_offset) + COMMIT_CIRCLE_RADIUS;

    for (segment_idx, segment) in line.segments.iter().enumerate() {
        let is_last = segment_idx + 1 == line.segments.len();

        match segment {
            CommitLineSegment::Straight { to_row } => {
                let start_x = lane_center_x(graph_rect, current_column);
                let end_x = lane_center_x(graph_rect, current_column);
                let mut end_y = row_center_y(content_rect, *to_row + row_offset);
                if is_last {
                    end_y -= COMMIT_CIRCLE_RADIUS;
                }

                ui.painter().line_segment(
                    [egui::pos2(start_x, current_y), egui::pos2(end_x, end_y)],
                    stroke,
                );

                current_y = end_y;
            }
            CommitLineSegment::Curve {
                to_column,
                on_row,
                curve_kind,
            } => {
                let start_x = lane_center_x(graph_rect, current_column);
                let end_x = lane_center_x(graph_rect, *to_column);
                let mut end_y = row_center_y(content_rect, *on_row + row_offset);
                if is_last {
                    end_y -= COMMIT_CIRCLE_RADIUS;
                }

                match curve_kind {
                    CurveKind::Merge | CurveKind::Checkout => {
                        let mid_y = (current_y + end_y) / 2.0;
                        let points = [
                            egui::pos2(start_x, current_y),
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

                current_y = end_y;
                current_column = *to_column;
            }
        }
    }
}

fn commit_subject(message: &str) -> &str {
    message.lines().next().unwrap_or(message).trim_end()
}

fn truncate_text_to_width(ui: &egui::Ui, text: &str, size: f32, max_width: f32) -> String {
    if text.is_empty() || max_width <= 0.0 {
        return String::new();
    }

    let font_id = egui::FontId::proportional(size);
    let painter = ui.painter();
    let full_width = painter
        .layout_no_wrap(text.to_owned(), font_id.clone(), egui::Color32::WHITE)
        .rect
        .width();

    if full_width <= max_width {
        return text.to_owned();
    }

    let ellipsis = "...";
    let ellipsis_width = painter
        .layout_no_wrap(ellipsis.to_owned(), font_id.clone(), egui::Color32::WHITE)
        .rect
        .width();

    if ellipsis_width > max_width {
        return String::new();
    }

    let chars: Vec<char> = text.chars().collect();
    let mut low = 0;
    let mut high = chars.len();

    while low < high {
        let mid = low + (high - low).div_ceil(2);
        let candidate: String = chars[..mid].iter().collect::<String>() + ellipsis;
        let width = painter
            .layout_no_wrap(candidate, font_id.clone(), egui::Color32::WHITE)
            .rect
            .width();

        if width <= max_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    let mut truncated: String = chars[..low].iter().collect();
    truncated.push_str(ellipsis);
    truncated
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
    app_state: &AppState,
) {
    let text = ui.visuals().text_color();
    let muted = egui::Color32::from_rgb(184, 184, 184);
    let is_selected = state.selected_row == Some(row_idx);
    let date_str = format_commit_date_from_secs(entry.data.timestamp_secs);

    draw_subject_cell(ui, row, columns.subject, entry, is_selected, text, muted);
    draw_author_cell(
        ui,
        row,
        columns.author,
        entry,
        text,
        &app_state.avatar_cache,
    );
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
    let font_size = if is_selected { 14.0 } else { 13.0 };
    let display_text = truncate_text_to_width(
        ui,
        commit_subject(&entry.data.message),
        font_size,
        max_width,
    );

    clipped_text(
        ui,
        cell,
        egui::pos2(cell.left(), row.center().y),
        &display_text,
        font_size,
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
    avatar_cache: &std::collections::HashMap<String, String>,
) {
    let cell = row.intersect(column).shrink2(egui::vec2(8.0, 0.0));
    let avatar = egui::Rect::from_center_size(
        egui::pos2(cell.left() + 9.0, row.center().y),
        egui::vec2(18.0, 18.0),
    );

    if let Some(path) = avatar_cache.get(&entry.data.author) {
        let uri = url::Url::from_file_path(path)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| format!("file://{}", path));
        let image = egui::Image::new(uri).corner_radius(2.0);
        image.paint_at(ui, avatar);
    } else {
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
    }

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

fn row_rect(content_rect: egui::Rect, index: usize, has_top_row: bool) -> egui::Rect {
    let offset = if has_top_row { 1 } else { 0 };
    egui::Rect::from_min_size(
        egui::pos2(
            content_rect.left(),
            content_rect.top() + (index + offset) as f32 * ROW_HEIGHT,
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

#[cfg(test)]
mod tests {
    use super::{RefBadge, RefKind, commit_subject, compare_tags_for_display, parsed_tag_version};

    #[test]
    fn commit_subject_uses_only_first_line() {
        assert_eq!(
            commit_subject("feat: add graph\n\nfull body text"),
            "feat: add graph"
        );
    }

    #[test]
    fn commit_subject_keeps_single_line_messages() {
        assert_eq!(
            commit_subject("fix: tighten graph lines"),
            "fix: tighten graph lines"
        );
    }

    #[test]
    fn parsed_tag_version_handles_semver_like_tags() {
        assert_eq!(parsed_tag_version("v1.2.3"), Some((1, 2, 3)));
        assert_eq!(parsed_tag_version("1.2.3"), Some((1, 2, 3)));
        assert_eq!(parsed_tag_version("v1.2"), None);
    }

    #[test]
    fn compare_tags_orders_versions_descending() {
        let make_tag = |label: &str| RefBadge {
            row: Some(0),
            label: label.to_string(),
            kind: RefKind::Tag,
            highlighted: false,
            connect_to_graph: true,
        };

        assert_eq!(
            compare_tags_for_display(&make_tag("v1.2.3"), &make_tag("v1.2.4")),
            std::cmp::Ordering::Greater
        );
    }
}
