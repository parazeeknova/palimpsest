use eframe::egui;
use egui_phosphor::regular::{BOOKMARK, CHECK, GIT_BRANCH, LAPTOP, PENCIL_SIMPLE, TAG, USER};
use std::collections::{BTreeMap, HashMap};
use std::ops::Range;

use crate::git::GitRepo;
use crate::git::live::RepoLiveEvent;
use crate::state::{AppState, CachedBranch, CachedCommit, CachedRemote, CachedTag};
use crate::ui::commit_drawer;
use crate::ui::commit_panel;

const HEADER_HEIGHT: f32 = 30.0;
const ROW_HEIGHT: f32 = 28.0;
const VERTICAL_ROW_HEIGHT: f32 = 44.0;
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
    color: egui::Color32,
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
        color: Option<egui::Color32>,
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
        parent_color: egui::Color32,
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

                for segment in segments.iter_mut() {
                    match segment {
                        CommitLineSegment::Straight { to_row } => {
                            if *to_row == usize::MAX {
                                *to_row = ending_row;
                            }
                        }
                        CommitLineSegment::Curve {
                            on_row, to_column, ..
                        } => {
                            if *on_row == usize::MAX {
                                *on_row = ending_row;
                            }
                            if *to_column == usize::MAX {
                                *to_column = final_destination;
                            }
                        }
                    }
                }

                let mut i = 0;
                while i + 1 < segments.len() {
                    let next = segments[i + 1].clone();
                    let current = segments[i].clone();

                    let can_merge = match (&current, &next) {
                        (
                            CommitLineSegment::Straight { to_row: r1 },
                            CommitLineSegment::Straight { to_row: r2 },
                        ) => {
                            let end = *r1.max(r2);
                            segments[i] = CommitLineSegment::Straight { to_row: end };
                            true
                        }
                        _ => false,
                    };

                    if can_merge {
                        segments.remove(i + 1);
                    } else {
                        if let CommitLineSegment::Curve {
                            on_row,
                            to_column,
                            curve_kind: _,
                        } = current
                        {
                            if on_row < ending_row {
                                if to_column != final_destination {
                                    segments.insert(
                                        i + 1,
                                        CommitLineSegment::Straight {
                                            to_row: ending_row - 1,
                                        },
                                    );
                                    segments.insert(
                                        i + 2,
                                        CommitLineSegment::Curve {
                                            to_column: final_destination,
                                            on_row: ending_row,
                                            curve_kind: CurveKind::Checkout,
                                        },
                                    );
                                    i += 2;
                                } else {
                                    segments.insert(
                                        i + 1,
                                        CommitLineSegment::Straight { to_row: ending_row },
                                    );
                                    i += 1;
                                }
                            } else if to_column != final_destination {
                                segments.insert(
                                    i + 1,
                                    CommitLineSegment::Curve {
                                        to_column: final_destination,
                                        on_row: ending_row,
                                        curve_kind: CurveKind::Checkout,
                                    },
                                );
                                i += 1;
                            }
                        }
                        i += 1;
                    }
                }

                if let Some(last) = segments.last_mut() {
                    match last {
                        CommitLineSegment::Straight { to_row } => {
                            if *to_row < ending_row {
                                if final_destination != lane_column {
                                    segments.push(CommitLineSegment::Curve {
                                        to_column: final_destination,
                                        on_row: ending_row,
                                        curve_kind: CurveKind::Checkout,
                                    });
                                } else {
                                    *to_row = ending_row;
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
                    }
                }

                Some(CommitLine {
                    child_column: starting_col,
                    full_interval: starting_row..ending_row,
                    color: final_color,
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
    color: egui::Color32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
    has_local: bool,
    has_remote: bool,
    remote_avatar: Option<String>,
}

#[derive(Clone, Debug)]
struct TopStatusRow {
    label: String,
    detail: String,
    graph_lane: Option<usize>,
    color: Option<egui::Color32>,
    show_ref_chip: bool,
    show_graph_node: bool,
}

struct GraphData {
    lane_states: Vec<LaneState>,
    lane_colors: HashMap<usize, egui::Color32>,
    parent_to_lanes: HashMap<usize, Vec<usize>>,
    next_color: usize,
    commits: Vec<CommitEntry>,
    max_lanes: usize,
    lines: Vec<CommitLine>,
    branches: Vec<CachedBranch>,
}

fn extract_merged_branch_name(message: &str) -> Option<&str> {
    let first_line = message.lines().next()?;

    if let Some(from_idx) = first_line.find("from ") {
        let rest = &first_line[from_idx + 5..];
        let branch_part = rest.lines().next()?.trim();
        let cleaned = branch_part.trim_matches(|c| c == '\'' || c == '"');
        if !cleaned.is_empty() {
            return Some(cleaned);
        }
    }

    if let Some(start_idx) = first_line.find("Merge branch '") {
        let rest = &first_line[start_idx + 14..];
        if let Some(end_idx) = rest.find('\'') {
            let branch_name = &rest[..end_idx];
            if !branch_name.is_empty() {
                return Some(branch_name);
            }
        }
    }

    None
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
            branches: Vec::new(),
        }
    }

    fn clear(&mut self) {
        self.lane_states.clear();
        self.lane_colors.clear();
        self.parent_to_lanes.clear();
        self.commits.clear();
        self.lines.clear();
        self.branches.clear();
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

    fn get_lane_color(
        &mut self,
        lane_idx: usize,
        commit: &CachedCommit,
        branches: &[CachedBranch],
    ) -> egui::Color32 {
        let matching_branches: Vec<&CachedBranch> = branches
            .iter()
            .filter(|b| b.tip_hash == commit.hash || b.tip_hash == commit.short_hash)
            .collect();

        let best_branch = matching_branches.iter().min_by_key(|b| {
            let base_name = b.name.split('/').next_back().unwrap_or(b.name.as_str());

            // Factor 1: Does this base name point to any other commit?
            let has_other_commit = branches.iter().any(|other| {
                let other_base = other
                    .name
                    .split('/')
                    .next_back()
                    .unwrap_or(other.name.as_str());
                other_base == base_name
                    && other.tip_hash != commit.hash
                    && other.tip_hash != commit.short_hash
            });
            let score_factor_1: i32 = if has_other_commit { 1 } else { 0 };

            // Factor 2: Is it a primary branch (main/master/trunk)?
            let is_primary = base_name == "main" || base_name == "master" || base_name == "trunk";
            let score_factor_2: i32 = if is_primary { -10 } else { 0 };

            // Factor 3: Is it local?
            let score_factor_3: i32 = if !b.is_remote { -2 } else { 0 };

            // Factor 4: Is it current/checked out?
            let score_factor_4: i32 = if b.is_current { -1 } else { 0 };

            score_factor_1 + score_factor_2 + score_factor_3 + score_factor_4
        });

        let branch_name = best_branch.copied().map(|b| b.name.as_str());

        if let Some(name) = branch_name {
            let color = crate::ui::colors::get_branch_color(name, branches);
            self.lane_colors.insert(lane_idx, color);
            color
        } else {
            *self.lane_colors.entry(lane_idx).or_insert_with(|| {
                let golden_ratio_conjugate = 0.618_034_f32;
                let hue = (lane_idx as f32 * golden_ratio_conjugate).fract();
                egui::Color32::from(egui::epaint::Hsva::new(hue, 0.75, 0.85, 1.0))
            })
        }
    }

    fn add_commits(&mut self, commits: &[CachedCommit], branches: &[CachedBranch]) {
        self.branches = branches.to_vec();
        self.commits.reserve(commits.len());
        self.lines.reserve(commits.len() / 2);

        for (commit_idx, commit) in commits.iter().enumerate() {
            let commit_row = self.commits.len();

            let commit_lane = self
                .parent_to_lanes
                .get(&commit_idx)
                .and_then(|lanes| lanes.iter().min().copied());

            let commit_lane = commit_lane.unwrap_or_else(|| self.first_empty_lane_idx());
            let original_lane_color = self.lane_colors.get(&commit_lane).copied();
            let commit_color = self.get_lane_color(commit_lane, commit, branches);

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
                        let mut inherited_color = None;
                        if let Some(branch_name) = extract_merged_branch_name(&commit.message) {
                            inherited_color =
                                Some(crate::ui::colors::get_branch_color(branch_name, branches));
                        }
                        let inherited_color = inherited_color.or(original_lane_color);

                        if let Some(color) = inherited_color {
                            self.lane_colors.insert(new_lane, color);
                        }

                        self.lane_states[new_lane] = LaneState::Active {
                            parent: parent_global_idx,
                            child: commit_idx,
                            color: inherited_color,
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
                color: commit_color,
            });
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct RefsFingerprint {
    branches_len: usize,
    branches_fingerprint: String,
    tags_len: usize,
    tags_fingerprint: String,
    releases_len: usize,
    releases_fingerprint: String,
    status_branch: Option<String>,
    status_staged_count: usize,
    status_unstaged_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum CommitDrawerLayout {
    #[default]
    Horizontal,
    Vertical,
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
    pub selected_commit_diff_cache: Option<crate::cdv::CommitDiffViewModel>,
    pub drawer_state: commit_drawer::State,
    pub layout: CommitDrawerLayout,
    refs_fingerprint: Option<RefsFingerprint>,
    pub scroll_to_selected: bool,
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
            selected_commit_diff_cache: None,
            drawer_state: commit_drawer::State::default(),
            layout: CommitDrawerLayout::default(),
            refs_fingerprint: None,
            scroll_to_selected: false,
        }
    }
}

impl State {
    fn refresh_refs(&mut self, app_state: &AppState) {
        self.branch_refs =
            build_branch_refs(&self.graph_data, &app_state.cached_branches, app_state);
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
            self.selected_commit_diff_cache = None;
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
            self.selected_commit_diff_cache = None;
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
        self.selected_commit_diff_cache = None;

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
                    let diff = repo.commit_diff_view(&hash).ok();

                    let _ = repo_live_tx.send(RepoLiveEvent::CommitDetails {
                        path: repo_path,
                        hash,
                        email,
                        signature,
                        files,
                        diff,
                    });
                    ctx.request_repaint();
                }
            });
        }
    }
}

fn get_base_branch_name(name: &str, remotes: &[CachedRemote]) -> String {
    for remote in remotes {
        let prefix = format!("{}/", remote.name);
        if name.starts_with(&prefix) {
            return name[prefix.len()..].to_string();
        }
    }
    name.to_string()
}

fn build_branch_refs(
    graph_data: &GraphData,
    branches: &[CachedBranch],
    app_state: &AppState,
) -> Vec<RefBadge> {
    let mut groups: HashMap<(String, String), Vec<&CachedBranch>> = HashMap::new();

    for branch in branches {
        let base_name = get_base_branch_name(&branch.name, &app_state.cached_remotes);
        if base_name == "HEAD" {
            continue;
        }
        groups
            .entry((branch.tip_hash.clone(), base_name))
            .or_default()
            .push(branch);
    }

    let mut refs = Vec::new();

    let user_avatar = app_state.github_user.as_ref().and_then(|u| {
        app_state.avatar_cache.get(&u.login).cloned().or_else(|| {
            u.name
                .as_ref()
                .and_then(|n| app_state.avatar_cache.get(n).cloned())
        })
    });

    for ((tip_hash, base_name), group_branches) in groups {
        let row = commit_row_for_hash(graph_data, &tip_hash);
        if row.is_none() {
            continue;
        }

        let has_local = group_branches.iter().any(|b| !b.is_remote);
        let has_remote = group_branches.iter().any(|b| b.is_remote);
        let highlighted = group_branches.iter().any(|b| b.is_current);

        refs.push(RefBadge {
            row,
            label: base_name,
            kind: RefKind::Branch,
            highlighted,
            connect_to_graph: true,
            has_local,
            has_remote,
            remote_avatar: if has_remote {
                user_avatar.clone()
            } else {
                None
            },
        });
    }

    refs.sort_by(|a, b| {
        if a.highlighted != b.highlighted {
            b.highlighted.cmp(&a.highlighted)
        } else {
            a.label.cmp(&b.label)
        }
    });

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
            has_local: false,
            has_remote: false,
            remote_avatar: None,
        })
        .collect()
}

fn fingerprint_entries(entries: impl IntoIterator<Item = String>) -> String {
    let mut entries: Vec<String> = entries.into_iter().collect();
    entries.sort();
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    entries.join("\n").hash(&mut hasher);
    format!("{:x}", hasher.finish())
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
                has_local: false,
                has_remote: false,
                remote_avatar: None,
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

    let (graph_lane, color) = app_state
        .cached_branches
        .iter()
        .find(|branch| branch.is_current)
        .and_then(|branch| {
            commit_row_for_hash(graph_data, branch.tip_hash.as_str()).map(|row| {
                let entry = &graph_data.commits[row];
                (Some(entry.lane), Some(entry.color))
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
        color,
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
    let active_branch_changed = {
        let old_active = state
            .graph_data
            .branches
            .iter()
            .find(|b| b.is_current)
            .map(|b| &b.name);
        let new_active = app_state
            .cached_branches
            .iter()
            .find(|b| b.is_current)
            .map(|b| &b.name);
        old_active != new_active
    };

    if active_branch_changed {
        if let Some(new_branch) = app_state.cached_branches.iter().find(|b| b.is_current) {
            let commit_match = app_state.cached_commits.iter().find(|c| {
                c.short_hash == new_branch.tip_hash || c.hash.starts_with(&new_branch.tip_hash)
            });
            if let Some(commit) = commit_match {
                state.selected_commit_hash = Some(commit.hash.clone());
                state.scroll_to_selected = true;
                state.drawer_state.tab = commit_drawer::CommitDrawerTab::Commit;
            }
        }
    }

    let graph_commits_changed = state.graph_data.commits.len() != app_state.cached_commits.len()
        || active_branch_changed
        || state
            .graph_data
            .commits
            .iter()
            .zip(app_state.cached_commits.iter())
            .any(|(entry, cached)| entry.data.hash != cached.hash);

    if graph_commits_changed {
        state.graph_data.clear();
        if !app_state.cached_commits.is_empty() {
            state
                .graph_data
                .add_commits(&app_state.cached_commits, &app_state.cached_branches);
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
        branches_fingerprint: fingerprint_entries(
            app_state
                .cached_branches
                .iter()
                .map(|branch| format!("{}|{}", branch.name, branch.tip_hash)),
        ),
        tags_len: app_state.cached_tags.len(),
        tags_fingerprint: fingerprint_entries(
            app_state
                .cached_tags
                .iter()
                .map(|tag| format!("{}|{}", tag.name, tag.target_hash)),
        ),
        releases_len: app_state.github_releases.len(),
        releases_fingerprint: fingerprint_entries(app_state.github_releases.iter().map(
            |release| {
                let label = release
                    .name
                    .clone()
                    .unwrap_or_else(|| release.tag_name.clone());
                let target_hash = app_state
                    .cached_tags
                    .iter()
                    .find(|tag| tag.name == release.tag_name)
                    .map(|tag| tag.target_hash.clone())
                    .unwrap_or_default();
                format!("{}|{}", label, target_hash)
            },
        )),
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
    let mut drawer_width = 0.0;

    if app_state.cached_commits.is_empty() {
        state.selected_commit_hash = None;
        state.selected_commit_cache_hash = None;
        state.selected_commit_cache = None;
        state.selected_commit_signature_cache = None;
        state.selected_commit_files_cache.clear();
        state.selected_commit_diff_cache = None;
        state.selected_commit_cache_populated_with_repo = false;
        state.selected_commit_cache_repo = None;
        state.selected_row = None;

        let logo = egui::Image::new(egui::include_image!("../assets/logo.svg"))
            .tint(egui::Color32::from_white_alpha(40))
            .fit_to_exact_size(egui::vec2(200.0, 200.0));
        let logo_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(200.0, 200.0));
        ui.put(logo_rect, logo);
    } else {
        let is_vertical = state.layout == CommitDrawerLayout::Vertical;
        let header_height = if is_vertical { 0.0 } else { HEADER_HEIGHT };
        let header_rect =
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(rect.width(), header_height));

        if !is_vertical {
            clamp_columns(state, rect.width());
        }

        let total_width = if is_vertical {
            rect.width()
        } else {
            total_content_width(state).max(rect.width())
        };

        let rows_rect = egui::Rect::from_min_max(
            egui::pos2(rect.left(), header_rect.bottom()),
            rect.right_bottom(),
        );

        drawer_height = 0.0;

        if state.selected_commit_hash.is_some() && !state.drawer_state.detached {
            match state.layout {
                CommitDrawerLayout::Horizontal => {
                    drawer_height = state.drawer_state.height.clamp(0.0, rows_rect.height());
                }
                CommitDrawerLayout::Vertical => {
                    let max_width = (rows_rect.width() - 150.0).max(0.0);
                    drawer_width = state.drawer_state.height.clamp(0.0, max_width);
                }
            }
        }

        let list_rect = if drawer_height > 0.0 {
            egui::Rect::from_min_max(
                rows_rect.left_top(),
                egui::pos2(rows_rect.right(), rows_rect.bottom() - drawer_height),
            )
        } else if drawer_width > 0.0 {
            egui::Rect::from_min_max(
                rows_rect.left_top(),
                egui::pos2(rows_rect.right() - drawer_width, rows_rect.bottom()),
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
                let scroll = if is_vertical {
                    egui::ScrollArea::vertical()
                } else {
                    egui::ScrollArea::both()
                };
                scroll
                    .id_salt("commit_body_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let row_height = if is_vertical {
                            VERTICAL_ROW_HEIGHT
                        } else {
                            ROW_HEIGHT
                        };
                        let extra_row_height = if state.top_status_row.is_some() {
                            row_height
                        } else {
                            0.0
                        };
                        let content_width = if is_vertical {
                            ui.available_width()
                        } else {
                            total_width
                        };
                        let content_size = egui::vec2(
                            content_width,
                            state.graph_data.commits.len() as f32 * row_height + extra_row_height,
                        );
                        let (content_rect, _) =
                            ui.allocate_exact_size(content_size, egui::Sense::hover());
                        let columns = if is_vertical {
                            columns_for_vertical(content_rect, state)
                        } else {
                            columns_for(content_rect, state, total_width)
                        };
                        paint_rows(ui, content_rect, &columns, state, app_state);
                    });
            },
        );

        if !is_vertical {
            ui.painter()
                .rect_filled(header_rect.expand2(egui::vec2(0.0, 1.0)), 0.0, header_bg);
            ui.painter().line_segment(
                [header_rect.left_bottom(), header_rect.right_bottom()],
                stroke,
            );
            let cols = columns_for(header_rect, state, total_width);
            paint_header(ui, header_rect, &cols, state, stroke);
        }

        state.refresh_selected_commit_cache(app_state, git_repo, repo_live_tx, ui.ctx());

        let mut close_clicked = false;
        let mut detach_clicked = false;

        if let Some(selected) = state.selected_commit_cache.as_ref() {
            if !state.drawer_state.detached {
                let drawer_rect = if state.layout == CommitDrawerLayout::Horizontal {
                    egui::Rect::from_min_max(
                        egui::pos2(rows_rect.left(), rows_rect.bottom() - drawer_height),
                        rows_rect.right_bottom(),
                    )
                } else {
                    egui::Rect::from_min_max(
                        egui::pos2(rows_rect.right() - drawer_width, rows_rect.top()),
                        rows_rect.right_bottom(),
                    )
                };
                match commit_drawer::show(
                    ui,
                    drawer_rect,
                    &mut state.drawer_state,
                    app_state,
                    Some(selected),
                    state.selected_commit_signature_cache.as_ref(),
                    &state.selected_commit_files_cache,
                    state.selected_commit_diff_cache.as_ref(),
                    state.layout == CommitDrawerLayout::Vertical,
                ) {
                    commit_drawer::CommitDrawerResponse::Close => {
                        close_clicked = true;
                    }
                    commit_drawer::CommitDrawerResponse::Detach => {
                        detach_clicked = true;
                    }
                    _ => {}
                }
            }
        }

        if detach_clicked {
            state.drawer_state.detached = true;
            ui.ctx().request_repaint();
        }

        if state.drawer_state.detached {
            if let Some(selected) = state.selected_commit_cache.as_ref() {
                let mut close_detached = false;
                let mut attach_detached = false;

                ui.ctx().show_viewport_immediate(
                    egui::ViewportId::from_hash_of("commit_drawer_viewport"),
                    egui::ViewportBuilder::default()
                        .with_title("Commit Details")
                        .with_inner_size(egui::vec2(600.0, 400.0)),
                    |ui, _class| {
                        if ui.ctx().input(|i| i.viewport().close_requested()) {
                            close_detached = true;
                        }

                        egui::CentralPanel::default().show_inside(
                            ui,
                            |ui| match commit_drawer::show(
                                ui,
                                ui.max_rect(),
                                &mut state.drawer_state,
                                app_state,
                                Some(selected),
                                state.selected_commit_signature_cache.as_ref(),
                                &state.selected_commit_files_cache,
                                state.selected_commit_diff_cache.as_ref(),
                                state.layout == CommitDrawerLayout::Vertical,
                            ) {
                                commit_drawer::CommitDrawerResponse::Close => {
                                    close_detached = true;
                                }
                                commit_drawer::CommitDrawerResponse::Attach => {
                                    attach_detached = true;
                                }
                                _ => {}
                            },
                        );
                    },
                );

                if attach_detached {
                    state.drawer_state.detached = false;
                    ui.ctx().request_repaint();
                }

                if close_detached {
                    state.drawer_state.detached = false;
                    close_clicked = true;
                }
            } else {
                state.drawer_state.detached = false;
            }
        }

        if close_clicked {
            state.selected_commit_hash = None;
            state.selected_commit_cache_hash = None;
            state.selected_commit_cache = None;
            state.selected_commit_signature_cache = None;
            state.selected_commit_files_cache.clear();
            state.selected_commit_cache_populated_with_repo = false;
            state.selected_commit_cache_repo = None;
            state.selected_row = None;
            ui.ctx().request_repaint();
        }
    }

    let show_panel = app_state
        .cached_status
        .as_ref()
        .is_some_and(|s| s.staged_count > 0 || s.unstaged_count > 0);

    if show_panel {
        let bottom_offset = if state.selected_commit_hash.is_some()
            && !state.drawer_state.detached
            && state.layout == CommitDrawerLayout::Horizontal
        {
            drawer_height
        } else {
            0.0
        };
        let panel_body_rect = if state.selected_commit_hash.is_some()
            && !state.drawer_state.detached
            && state.layout == CommitDrawerLayout::Vertical
        {
            egui::Rect::from_min_max(
                rect.left_top(),
                egui::pos2(rect.right() - drawer_width, rect.bottom()),
            )
        } else {
            rect
        };
        commit_panel::show_cached_with_bottom_offset(
            ui,
            panel_body_rect,
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

fn columns_for_vertical(rect: egui::Rect, state: &State) -> Columns {
    let graph = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.top()),
        egui::vec2(graph_width(state), rect.height()),
    );
    Columns {
        graph,
        refs: egui::Rect::ZERO,
        subject: egui::Rect::ZERO,
        author: egui::Rect::ZERO,
        hash: egui::Rect::ZERO,
        date: egui::Rect::ZERO,
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

fn row_center_y(content_rect: egui::Rect, row: usize, row_height: f32) -> f32 {
    content_rect.top() + row as f32 * row_height + row_height / 2.0
}

fn paint_rows(
    ui: &mut egui::Ui,
    content_rect: egui::Rect,
    columns: &Columns,
    state: &mut State,
    app_state: &AppState,
) {
    let is_vertical = state.layout == CommitDrawerLayout::Vertical;
    let row_height = if is_vertical {
        VERTICAL_ROW_HEIGHT
    } else {
        ROW_HEIGHT
    };
    let has_top_row = state.top_status_row.is_some();
    let row_offset = usize::from(has_top_row);

    if let Some(top_status_row) = &state.top_status_row {
        paint_top_status_row(
            ui,
            content_rect,
            columns,
            top_status_row,
            row_height,
            is_vertical,
        );
    }

    let mut refs = Vec::with_capacity(state.branch_refs.len() + state.tag_refs.len());
    refs.extend(state.branch_refs.iter().cloned());
    refs.extend(state.tag_refs.iter().cloned());

    for (row_idx, entry) in state.graph_data.commits.iter().enumerate() {
        let row_rect = row_rect(content_rect, row_idx, has_top_row, row_height);

        let is_selected = state.selected_row == Some(row_idx);
        let is_hovered = state.hovered_row == Some(row_idx);

        if is_selected {
            ui.painter()
                .rect_filled(row_rect, 0.0, egui::Color32::from_rgb(48, 48, 48));
            let left_color = egui::Color32::from_rgba_unmultiplied(
                entry.color.r(),
                entry.color.g(),
                entry.color.b(),
                55,
            );
            let right_color = egui::Color32::from_rgba_unmultiplied(
                entry.color.r(),
                entry.color.g(),
                entry.color.b(),
                0,
            );
            paint_gradient_rect(ui, row_rect, left_color, right_color);
        } else if is_hovered {
            let left_color = egui::Color32::from_rgba_unmultiplied(
                entry.color.r(),
                entry.color.g(),
                entry.color.b(),
                25,
            );
            let right_color = egui::Color32::from_rgba_unmultiplied(
                entry.color.r(),
                entry.color.g(),
                entry.color.b(),
                0,
            );
            paint_gradient_rect(ui, row_rect, left_color, right_color);
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

        if state.scroll_to_selected && is_selected {
            response.scroll_to_me(Some(egui::Align::Center));
            state.scroll_to_selected = false;
        }

        if response.hovered() {
            state.hovered_row = Some(row_idx);
        } else if state.hovered_row == Some(row_idx) {
            state.hovered_row = None;
        }

        if is_vertical {
            paint_vertical_commit_row(
                ui, row_rect, columns, entry, row_idx, state, app_state, &refs,
            );
        } else {
            paint_commit_row(ui, row_rect, columns, entry, row_idx, state, app_state);
        }
    }

    paint_graph(
        ui,
        columns.graph,
        content_rect,
        state,
        row_offset,
        row_height,
    );

    if !is_vertical {
        paint_ref_badges(
            ui,
            content_rect,
            columns,
            &state.graph_data,
            &refs,
            has_top_row,
        );
    }
}

fn paint_top_status_row(
    ui: &mut egui::Ui,
    content_rect: egui::Rect,
    columns: &Columns,
    top_status_row: &TopStatusRow,
    row_height: f32,
    is_vertical: bool,
) {
    let row = row_rect(content_rect, 0, false, row_height);
    ui.painter()
        .rect_filled(row, 0.0, egui::Color32::from_rgb(42, 42, 42));

    if top_status_row.show_graph_node {
        if let (Some(lane), Some(color)) = (top_status_row.graph_lane, top_status_row.color) {
            let graph_center_x = lane_center_x(columns.graph, lane);
            let graph_center_y = row.center().y;
            draw_wip_connector(
                ui,
                graph_center_x,
                graph_center_y + WIP_NODE_RADIUS,
                graph_center_y + row_height - WIP_NODE_RADIUS,
                color,
            );
            draw_dotted_commit_node(ui, graph_center_x, graph_center_y, color);
        }
    }

    if is_vertical {
        let details_left = columns.graph.right() + 8.0;
        let details_right = row.right() - 8.0;
        let top_center_y = row.top() + 13.0;
        let bottom_center_y = row.top() + 32.0;

        if top_status_row.show_ref_chip {
            let text_width = ui
                .painter()
                .layout_no_wrap(
                    top_status_row.label.clone(),
                    egui::FontId::proportional(9.0),
                    egui::Color32::WHITE,
                )
                .rect
                .width();
            let avail = (details_right - details_left).max(0.0);
            let chip_width = if avail < 48.0 {
                (text_width + 20.0).min(avail)
            } else {
                (text_width + 20.0).clamp(48.0, avail)
            };
            let chip_rect = egui::Rect::from_min_size(
                egui::pos2(details_left, top_center_y - 8.0),
                egui::vec2(chip_width, 16.0),
            );
            let accent = egui::Color32::from_rgb(252, 197, 34);
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

        clipped_text(
            ui,
            egui::Rect::from_min_max(
                egui::pos2(details_left, row.top() + 24.0),
                egui::pos2(details_right, row.top() + 40.0),
            ),
            egui::pos2(details_left, bottom_center_y),
            &top_status_row.detail,
            12.0,
            ui.visuals().text_color(),
            egui::Align2::LEFT_CENTER,
        );
    } else {
        if top_status_row.show_ref_chip {
            let refs_rect = row.intersect(columns.refs).shrink2(egui::vec2(6.0, 4.0));
            let accent = egui::Color32::from_rgb(252, 197, 34);
            let chip_height = 16.0;
            let text_width = ui
                .painter()
                .layout_no_wrap(
                    top_status_row.label.clone(),
                    egui::FontId::proportional(9.0),
                    egui::Color32::WHITE,
                )
                .rect
                .width();
            let avail = refs_rect.width().max(0.0);
            let chip_width = if avail < 48.0 {
                (text_width + 20.0).min(avail)
            } else {
                (text_width + 20.0).clamp(48.0, avail)
            };
            let chip_rect = egui::Rect::from_min_size(
                egui::pos2(refs_rect.left(), refs_rect.top()),
                egui::vec2(chip_width, chip_height),
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

    // Deferred chips from the hovered/expanded row so they paint on top of all others.
    let mut deferred_chips: Vec<(RefBadge, egui::Rect)> = Vec::new();

    for (row_idx, mut badges) in refs_by_row {
        let row = row_rect(content_rect, row_idx, has_top_row, ROW_HEIGHT);
        let refs_rect = row.intersect(columns.refs).shrink2(egui::vec2(6.0, 4.0));
        let chip_height = 16.0;

        if badges.is_empty() {
            continue;
        }

        badges.sort_by(compare_badges_for_display);

        let primary_badge = badges[0].clone();

        // Check hover region using preliminary unconstrained primary rect
        let temp_primary_width = chip_width_for_badge(ui, &primary_badge, refs_rect.width());
        let temp_primary_rect = egui::Rect::from_min_size(
            egui::pos2(refs_rect.left(), refs_rect.center().y - chip_height * 0.5),
            egui::vec2(temp_primary_width, chip_height),
        );
        let hover_pos = ui.input(|input| input.pointer.hover_pos());
        let is_hovered = hover_pos
            .map(|pos| temp_primary_rect.contains(pos))
            .unwrap_or(false);

        let num_extra = badges.len() - 1;
        let has_extra = num_extra > 0 && !is_hovered;

        let extra_width = if has_extra {
            let extra_label = format!("+{}", num_extra);
            let extra_text_width = ui
                .painter()
                .layout_no_wrap(
                    extra_label,
                    egui::FontId::proportional(9.0),
                    egui::Color32::WHITE,
                )
                .rect
                .width();
            extra_text_width + 10.0
        } else {
            0.0
        };

        let max_primary_width = if has_extra {
            (refs_rect.width() - extra_width - 4.0).max(20.0)
        } else {
            refs_rect.width()
        };

        let primary_width = chip_width_for_badge(ui, &primary_badge, max_primary_width);
        let primary_rect = egui::Rect::from_min_size(
            egui::pos2(refs_rect.left(), refs_rect.center().y - chip_height * 0.5),
            egui::vec2(primary_width, chip_height),
        );

        let mut visible_chips = vec![(primary_badge.clone(), primary_rect)];

        if is_hovered && badges.len() > 1 {
            // For the topmost commit row, expand badges downward to avoid
            // clipping above the visible area; otherwise expand upward.
            let expand_down = row_idx == 0;
            if expand_down {
                let mut stack_bottom = primary_rect.bottom() + 2.0;
                for badge in badges.iter().skip(1).cloned() {
                    let width = chip_width_for_badge(ui, &badge, refs_rect.width());
                    visible_chips.push((
                        badge,
                        egui::Rect::from_min_size(
                            egui::pos2(refs_rect.left(), stack_bottom),
                            egui::vec2(width, chip_height),
                        ),
                    ));
                    stack_bottom += chip_height + 2.0;
                }
            } else {
                let mut stack_top = primary_rect.top() - 2.0;
                for badge in badges.iter().skip(1).cloned() {
                    let width = chip_width_for_badge(ui, &badge, refs_rect.width());
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

            // Defer expanded chips so they paint on top of all other rows.
            if let Some((badge, chip_rect)) = visible_chips.first() {
                if badge.connect_to_graph {
                    paint_ref_connector(ui, columns.graph, *chip_rect, badge, graph_data);
                }
            }
            deferred_chips.extend(visible_chips);
            continue;
        }

        if let Some((badge, chip_rect)) = visible_chips.first() {
            if badge.connect_to_graph {
                paint_ref_connector(ui, columns.graph, *chip_rect, badge, graph_data);
            }
        }

        for (badge, chip_rect) in visible_chips.into_iter().rev() {
            paint_ref_chip(ui, chip_rect, &badge, color_for_ref(&badge, graph_data));
        }

        if has_extra {
            let extra_label = format!("+{}", num_extra);
            let extra_rect = egui::Rect::from_min_size(
                egui::pos2(
                    refs_rect.left() + primary_width + 4.0,
                    refs_rect.center().y - chip_height * 0.5,
                ),
                egui::vec2(extra_width, chip_height),
            );

            let bg_color = egui::Color32::from_rgb(52, 52, 52);
            let border_color = egui::Color32::from_rgb(100, 100, 100);
            ui.painter().rect_filled(extra_rect, 3.0, bg_color);
            ui.painter().rect_stroke(
                extra_rect,
                3.0,
                egui::Stroke::new(1.0_f32, border_color),
                egui::StrokeKind::Inside,
            );

            clipped_text(
                ui,
                extra_rect,
                extra_rect.center(),
                &extra_label,
                9.0,
                ui.visuals().text_color(),
                egui::Align2::CENTER_CENTER,
            );
        }
    }

    // Paint deferred (hovered/expanded) chips last so they render on top.
    for (badge, chip_rect) in deferred_chips.into_iter().rev() {
        paint_ref_chip(ui, chip_rect, &badge, color_for_ref(&badge, graph_data));
    }
}

fn chip_width_for_badge(ui: &egui::Ui, badge: &RefBadge, max_width: f32) -> f32 {
    let font_id = egui::FontId::proportional(9.0);
    let text_width = ui
        .painter()
        .layout_no_wrap(badge.label.clone(), font_id, egui::Color32::WHITE)
        .rect
        .width();
    let mut base = if badge.highlighted { 36.0 } else { 30.0 };

    if badge.kind == RefKind::Branch {
        if badge.has_local {
            let laptop_width = ui
                .painter()
                .layout_no_wrap(
                    LAPTOP.to_string(),
                    egui::FontId::proportional(9.0),
                    egui::Color32::WHITE,
                )
                .rect
                .width();
            base += laptop_width + 4.0;
        }
        if badge.has_remote {
            base += 12.0 + 4.0; // avatar width 12 + gap 4
        }
    }

    let avail = max_width.max(0.0);
    if avail < 48.0 {
        (text_width + base).min(avail)
    } else {
        (text_width + base).clamp(48.0, avail)
    }
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

    let parse_part = |s: &str| -> Option<u64> {
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        if digits.is_empty() {
            None
        } else {
            digits.parse().ok()
        }
    };

    let major = parse_part(parts.next()?)?;
    let minor = parse_part(parts.next()?)?;
    let patch = parse_part(parts.next()?)?;
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

    let connector_color = color_for_ref(badge, graph_data).linear_multiply(0.4);
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

fn paint_ref_chip(ui: &mut egui::Ui, rect: egui::Rect, badge: &RefBadge, accent: egui::Color32) {
    // 1. Draw base dark background for the badge
    ui.painter()
        .rect_filled(rect, 3.0, egui::Color32::from_rgb(46, 46, 46));

    // 2. Draw left-to-right gradient overlay
    let left_alpha = if badge.highlighted { 90 } else { 45 };
    let left_color =
        egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), left_alpha);
    let right_color = egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 0);
    paint_gradient_rect(ui, rect, left_color, right_color);

    // 2. Draw solid opaque background for the icon area (left part)
    let icon_bg_width = if badge.kind == RefKind::Branch && badge.has_local {
        30.0
    } else {
        18.0
    };
    let left_rect = egui::Rect::from_min_max(
        rect.left_top(),
        egui::pos2(rect.left() + icon_bg_width, rect.bottom()),
    );
    let left_radius = egui::CornerRadius {
        nw: 3,
        ne: 0,
        se: 0,
        sw: 3,
    };
    ui.painter().rect_filled(left_rect, left_radius, accent);

    // 3. Draw vertical divider line
    ui.painter().line_segment(
        [
            egui::pos2(rect.left() + icon_bg_width, rect.top()),
            egui::pos2(rect.left() + icon_bg_width, rect.bottom()),
        ],
        egui::Stroke::new(1.0_f32, accent),
    );

    // 4. Draw outer border outline
    ui.painter().rect_stroke(
        rect,
        3.0,
        egui::Stroke::new(1.0_f32, accent),
        egui::StrokeKind::Inside,
    );

    let icon = match badge.kind {
        RefKind::Branch => GIT_BRANCH,
        RefKind::Tag => TAG,
        RefKind::Release => BOOKMARK,
    };

    // 5. Draw icon centered in the left rect with a dark color for high contrast
    let icon_color = egui::Color32::from_rgb(31, 31, 31);
    if badge.kind == RefKind::Branch && badge.has_local {
        let first_rect = egui::Rect::from_min_max(
            left_rect.left_top(),
            egui::pos2(left_rect.left() + 15.0, left_rect.bottom()),
        );
        let second_rect = egui::Rect::from_min_max(
            egui::pos2(left_rect.left() + 15.0, left_rect.top()),
            left_rect.right_bottom(),
        );

        clipped_text(
            ui,
            first_rect,
            first_rect.center(),
            icon,
            9.0,
            icon_color,
            egui::Align2::CENTER_CENTER,
        );

        clipped_text(
            ui,
            second_rect,
            second_rect.center(),
            LAPTOP,
            9.0,
            icon_color,
            egui::Align2::CENTER_CENTER,
        );
    } else {
        clipped_text(
            ui,
            left_rect,
            left_rect.center(),
            icon,
            9.0,
            icon_color,
            egui::Align2::CENTER_CENTER,
        );
    }

    // 6. Draw label with padding from the icon divider
    let text_start_x = rect.left() + icon_bg_width + 6.0;
    clipped_text(
        ui,
        rect.shrink2(egui::vec2(4.0, 0.0)),
        egui::pos2(text_start_x, rect.center().y),
        &badge.label,
        9.0,
        ui.visuals().text_color(),
        egui::Align2::LEFT_CENTER,
    );

    // Draw GitKraken-style sub-icons for branch badges
    if badge.kind == RefKind::Branch {
        let font_id = egui::FontId::proportional(9.0);
        let label_width = ui
            .painter()
            .layout_no_wrap(badge.label.clone(), font_id.clone(), egui::Color32::WHITE)
            .rect
            .width();

        let current_x = text_start_x + label_width + 4.0;

        if badge.has_remote {
            let avatar_rect = egui::Rect::from_center_size(
                egui::pos2(current_x + 6.0, rect.center().y),
                egui::vec2(12.0, 12.0),
            );
            if let Some(ref path) = badge.remote_avatar {
                let uri = url::Url::from_file_path(path)
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| format!("file://{}", path));
                let image = egui::Image::new(uri).corner_radius(6.0);
                image.paint_at(ui, avatar_rect);
            } else {
                let circle_color = egui::Color32::from_rgb(80, 80, 80);
                ui.painter()
                    .circle_filled(avatar_rect.center(), 6.0, circle_color);
                clipped_text(
                    ui,
                    avatar_rect,
                    avatar_rect.center(),
                    USER,
                    7.0,
                    ui.visuals().text_color(),
                    egui::Align2::CENTER_CENTER,
                );
            }
        }
    }

    if badge.highlighted {
        let check_rect = egui::Rect::from_min_size(
            egui::pos2(rect.right() - 12.0, rect.center().y - 4.0),
            egui::vec2(8.0, 8.0),
        );
        clipped_text(
            ui,
            check_rect,
            check_rect.center(),
            CHECK,
            8.0,
            egui::Color32::from_rgb(46, 204, 113), // Premium emerald green checkmark
            egui::Align2::CENTER_CENTER,
        );
    }
}

fn color_for_ref(badge: &RefBadge, graph_data: &GraphData) -> egui::Color32 {
    match badge.kind {
        RefKind::Branch => crate::ui::colors::get_branch_color(&badge.label, &graph_data.branches),
        RefKind::Tag => crate::ui::colors::get_tag_color(&badge.label),
        RefKind::Release => egui::Color32::from_rgb(255, 110, 180),
    }
}

fn paint_graph(
    ui: &egui::Ui,
    graph_rect: egui::Rect,
    content_rect: egui::Rect,
    state: &State,
    row_offset: usize,
    row_height: f32,
) {
    for line in &state.graph_data.lines {
        paint_commit_line(ui, graph_rect, content_rect, line, row_offset, row_height);
    }

    for (row_idx, entry) in state.graph_data.commits.iter().enumerate() {
        let center_x = lane_center_x(graph_rect, entry.lane);
        let center_y = row_center_y(content_rect, row_idx + row_offset, row_height);
        let color = entry.color;

        draw_commit_circle(ui, center_x, center_y, color);
    }
}

fn paint_commit_line(
    ui: &egui::Ui,
    graph_rect: egui::Rect,
    content_rect: egui::Rect,
    line: &CommitLine,
    row_offset: usize,
    row_height: f32,
) {
    let color = line.color;
    let stroke = egui::Stroke::new(LINE_WIDTH, color);

    let mut current_column = line.child_column;
    let mut current_y = row_center_y(
        content_rect,
        line.full_interval.start + row_offset,
        row_height,
    ) + COMMIT_CIRCLE_RADIUS;

    for (segment_idx, segment) in line.segments.iter().enumerate() {
        let is_last = segment_idx + 1 == line.segments.len();

        match segment {
            CommitLineSegment::Straight { to_row } => {
                let start_x = lane_center_x(graph_rect, current_column);
                let end_x = lane_center_x(graph_rect, current_column);
                let mut end_y = row_center_y(content_rect, *to_row + row_offset, row_height);
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
                let mut end_y = row_center_y(content_rect, *on_row + row_offset, row_height);
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

#[allow(clippy::too_many_arguments)]
fn paint_vertical_commit_row(
    ui: &mut egui::Ui,
    row: egui::Rect,
    columns: &Columns,
    entry: &CommitEntry,
    row_idx: usize,
    state: &State,
    app_state: &AppState,
    refs: &[RefBadge],
) {
    let text = ui.visuals().text_color();
    let muted = egui::Color32::from_rgb(140, 140, 140);
    let is_selected = state.selected_row == Some(row_idx);
    let date_str = format_commit_date_from_secs(entry.data.timestamp_secs);

    let details_left = columns.graph.right() + 8.0;
    let details_right = row.right() - 8.0;
    let top_center_y = row.top() + 13.0;
    let bottom_center_y = row.top() + 32.0;

    // 1. Top row avatar
    let avatar = egui::Rect::from_center_size(
        egui::pos2(details_left + 8.0, top_center_y),
        egui::vec2(16.0, 16.0),
    );
    if let Some(path) = app_state.avatar_cache.get(&entry.data.author) {
        let uri = url::Url::from_file_path(path)
            .map(|u| u.to_string())
            .unwrap_or_else(|_| format!("file://{}", path));
        let image = egui::Image::new(uri).corner_radius(2.0);
        image.paint_at(ui, avatar);
    } else {
        let avatar_color = entry.color;
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

    // 2. Top row Author name, hash, and date
    // far right date
    clipped_text(
        ui,
        egui::Rect::from_min_max(
            egui::pos2(details_left + 24.0, row.top()),
            egui::pos2(details_right, row.top() + 24.0),
        ),
        egui::pos2(details_right, top_center_y),
        &date_str,
        11.0,
        muted,
        egui::Align2::RIGHT_CENTER,
    );

    // Calculate name and hash limits
    let date_font = egui::FontId::proportional(11.0);
    let date_width = ui
        .painter()
        .layout_no_wrap(date_str.clone(), date_font, egui::Color32::WHITE)
        .rect
        .width();

    let name_x = details_left + 24.0;
    // name_width is bounded:
    let max_name_width = ((details_right - date_width - 8.0) - name_x - 65.0).max(40.0);
    let name_font = egui::FontId::proportional(12.0);
    let truncated_name = truncate_text_to_width(ui, &entry.data.author, 12.0, max_name_width);
    let name_width_actual = ui
        .painter()
        .layout_no_wrap(truncated_name.clone(), name_font, egui::Color32::WHITE)
        .rect
        .width();

    clipped_text(
        ui,
        egui::Rect::from_min_max(
            egui::pos2(name_x, row.top()),
            egui::pos2(details_right - date_width - 8.0, row.top() + 24.0),
        ),
        egui::pos2(name_x, top_center_y),
        &truncated_name,
        12.0,
        if is_selected {
            text
        } else {
            ui.visuals().text_color()
        },
        egui::Align2::LEFT_CENTER,
    );

    let hash_x = name_x + name_width_actual + 8.0;
    clipped_text(
        ui,
        egui::Rect::from_min_max(
            egui::pos2(hash_x, row.top()),
            egui::pos2(details_right - date_width - 8.0, row.top() + 24.0),
        ),
        egui::pos2(hash_x, top_center_y),
        &entry.data.short_hash,
        11.0,
        muted,
        egui::Align2::LEFT_CENTER,
    );

    // 3. Bottom row: Associated Refs and Commit Message
    let mut row_badges = Vec::new();
    for badge in refs {
        if badge.row == Some(row_idx) {
            row_badges.push(badge.clone());
        }
    }
    row_badges.sort_by(compare_badges_for_display);

    let mut current_badge_x = details_left;
    let chip_height = 16.0;

    let max_refs_x = details_right - 40.0; // Reserve 40px for message

    for (i, badge) in row_badges.iter().enumerate() {
        let remaining = row_badges.len() - i;
        let width = chip_width_for_badge(ui, badge, max_refs_x - current_badge_x);

        let needs_extra_indicator = remaining > 1;
        let next_extra_label = format!("+{}", remaining - 1);
        let next_extra_width = if needs_extra_indicator {
            ui.painter()
                .layout_no_wrap(
                    next_extra_label,
                    egui::FontId::proportional(9.0),
                    egui::Color32::WHITE,
                )
                .rect
                .width()
                + 10.0
        } else {
            0.0
        };
        let space_needed_after = if needs_extra_indicator {
            next_extra_width + 6.0
        } else {
            0.0
        };

        if current_badge_x + width + space_needed_after <= max_refs_x {
            let chip_rect = egui::Rect::from_min_size(
                egui::pos2(current_badge_x, bottom_center_y - chip_height * 0.5),
                egui::vec2(width, chip_height),
            );
            paint_ref_chip(
                ui,
                chip_rect,
                badge,
                color_for_ref(badge, &state.graph_data),
            );
            current_badge_x += width + 6.0;
        } else {
            // Cannot fit this badge. Draw +N for all remaining
            let final_extra_label = format!("+{}", remaining);
            let final_extra_text_width = ui
                .painter()
                .layout_no_wrap(
                    final_extra_label.clone(),
                    egui::FontId::proportional(9.0),
                    egui::Color32::WHITE,
                )
                .rect
                .width();
            let final_extra_width = final_extra_text_width + 10.0;

            if current_badge_x + final_extra_width <= max_refs_x {
                let extra_rect = egui::Rect::from_min_size(
                    egui::pos2(current_badge_x, bottom_center_y - chip_height * 0.5),
                    egui::vec2(final_extra_width, chip_height),
                );
                let bg_color = egui::Color32::from_rgb(52, 52, 52);
                let border_color = egui::Color32::from_rgb(100, 100, 100);
                ui.painter().rect_filled(extra_rect, 3.0, bg_color);
                ui.painter().rect_stroke(
                    extra_rect,
                    3.0,
                    egui::Stroke::new(1.0_f32, border_color),
                    egui::StrokeKind::Inside,
                );
                clipped_text(
                    ui,
                    extra_rect,
                    extra_rect.center(),
                    &final_extra_label,
                    9.0,
                    ui.visuals().text_color(),
                    egui::Align2::CENTER_CENTER,
                );
                current_badge_x += final_extra_width + 6.0;
            }
            break;
        }
    }

    let msg_subject = commit_subject(&entry.data.message);
    let msg_width = details_right - current_badge_x;
    let display_msg = truncate_text_to_width(ui, msg_subject, 12.0, msg_width);

    clipped_text(
        ui,
        egui::Rect::from_min_max(
            egui::pos2(current_badge_x, row.top() + 24.0),
            egui::pos2(details_right, row.top() + 40.0),
        ),
        egui::pos2(current_badge_x, bottom_center_y),
        &display_msg,
        12.0,
        if is_selected { text } else { muted },
        egui::Align2::LEFT_CENTER,
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
        let avatar_color = entry.color;
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

fn row_rect(
    content_rect: egui::Rect,
    index: usize,
    has_top_row: bool,
    row_height: f32,
) -> egui::Rect {
    let offset = if has_top_row { 1 } else { 0 };
    egui::Rect::from_min_size(
        egui::pos2(
            content_rect.left(),
            content_rect.top() + (index + offset) as f32 * row_height,
        ),
        egui::vec2(content_rect.width(), row_height),
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
        assert_eq!(parsed_tag_version("v0.0.29-beta"), Some((0, 0, 29)));
        assert_eq!(parsed_tag_version("0.0.29.rc1"), None);
    }

    #[test]
    fn compare_tags_orders_versions_descending() {
        let make_tag = |label: &str| RefBadge {
            row: Some(0),
            label: label.to_string(),
            kind: RefKind::Tag,
            highlighted: false,
            connect_to_graph: true,
            has_local: false,
            has_remote: false,
            remote_avatar: None,
        };

        assert_eq!(
            compare_tags_for_display(&make_tag("v1.2.3"), &make_tag("v1.2.4")),
            std::cmp::Ordering::Greater
        );
    }
}
