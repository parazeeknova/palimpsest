use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use eframe::egui;
use notify::{RecursiveMode, Watcher};

use crate::auth::credentials;
use crate::auth::github_api;
use crate::git::GitRepo;
use crate::git::models::{Branch, Commit, Remote, RepoStatus, Stash, Tag};
use crate::state::{GitHubActionRun, GitHubPackage, GitHubPullRequest, GitHubRelease};

#[derive(Clone, Debug)]
pub struct RepoLocalSnapshot {
    pub commits: Vec<Commit>,
    pub branches: Vec<Branch>,
    pub remotes: Vec<Remote>,
    pub tags: Vec<Tag>,
    pub stashes: Vec<Stash>,
    pub status: RepoStatus,
    pub repo_error: Option<String>,
    pub last_refresh: Option<u128>,
    pub ownership: Option<bool>,
}

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RepoOwnership {
    Owned,
    External,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepoRemoteSnapshot {
    pub pull_requests: Vec<GitHubPullRequest>,
    pub action_runs: Vec<GitHubActionRun>,
    pub releases: Vec<GitHubRelease>,
    pub packages: Vec<GitHubPackage>,
    pub github_error: Option<String>,
    pub last_refresh: Option<u128>,
    pub ownership: RepoOwnership,
}

#[derive(Clone, Debug)]
pub enum RepoLiveEvent {
    Local {
        path: String,
        generation: u64,
        snapshot: RepoLocalSnapshot,
    },
    Remote {
        path: String,
        generation: u64,
        snapshot: RepoRemoteSnapshot,
    },
    Ownership {
        path: String,
        generation: u64,
        ownership: Option<bool>,
    },
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

pub fn classify_repo_ownership(remotes: &[Remote], github_login: Option<&str>) -> Option<bool> {
    let login = github_login?;
    let github_remote = remotes.iter().find_map(|remote| {
        let parsed = parse_github_remote(&remote.url)?;
        Some(parsed.0)
    })?;
    Some(github_remote.eq_ignore_ascii_case(login))
}

fn empty_status() -> RepoStatus {
    RepoStatus {
        branch: "HEAD".to_string(),
        staged_count: 0,
        unstaged_count: 0,
        staged_files: Vec::new(),
        unstaged_files: Vec::new(),
        additions: 0,
        deletions: 0,
        files_changed: 0,
    }
}

fn normalize_github_remote(url: &str) -> Option<String> {
    let input = url.trim();
    if input.starts_with("git@github.com:") {
        let path = input.strip_prefix("git@github.com:")?;
        let path = path.strip_suffix(".git").unwrap_or(path);
        let path = path.trim_matches('/');
        let mut parts = path.split('/');
        let owner = parts.next()?;
        let repo = parts.next()?;
        if !owner.is_empty() && !repo.is_empty() {
            return Some(format!("https://github.com/{}/{}", owner, repo));
        }
    } else if let Ok(parsed) = url::Url::parse(input) {
        if parsed.host_str() == Some("github.com") {
            let path = parsed.path().trim_matches('/');
            let path = path.strip_suffix(".git").unwrap_or(path);
            let mut parts = path.split('/');
            let owner = parts.next()?;
            let repo = parts.next()?;
            if !owner.is_empty() && !repo.is_empty() {
                return Some(format!("https://github.com/{}/{}", owner, repo));
            }
        }
    }
    None
}

pub(crate) fn parse_github_remote(url: &str) -> Option<(String, String)> {
    let base = normalize_github_remote(url)?;
    let path = base.strip_prefix("https://github.com/")?;
    let mut parts = path.split('/');
    let owner = parts.next()?.to_string();
    let repo = parts.next()?.to_string();
    if owner.is_empty() || repo.is_empty() {
        return None;
    }
    Some((owner, repo))
}

pub fn collect_commits_window(repo: &GitRepo) -> Result<Vec<Commit>, String> {
    repo.commits(Some(200)).map_err(|e| e.to_string())
}

pub fn collect_refs_summary(repo: &GitRepo) -> Result<Vec<Branch>, String> {
    repo.branches().map_err(|e| e.to_string())
}

pub fn collect_remotes(repo: &GitRepo) -> Result<Vec<Remote>, String> {
    repo.remotes().map_err(|e| e.to_string())
}

pub fn collect_tags_summary(repo: &GitRepo) -> Result<Vec<Tag>, String> {
    repo.tags_limit(Some(100)).map_err(|e| e.to_string())
}

pub fn collect_stashes_summary(repo: &GitRepo) -> Result<Vec<Stash>, String> {
    repo.stashes().map_err(|e| e.to_string())
}

pub fn collect_status_summary(repo: &GitRepo) -> Result<RepoStatus, String> {
    match repo.status() {
        Ok(mut status) => {
            if status.staged_files.len() > 500 {
                status.staged_files.truncate(500);
            }
            if status.unstaged_files.len() > 500 {
                status.unstaged_files.truncate(500);
            }
            Ok(status)
        }
        Err(e) => Err(e.to_string()),
    }
}

pub fn collect_local_snapshot(repo: &GitRepo, github_login: Option<&str>) -> RepoLocalSnapshot {
    let mut errors = Vec::new();

    let commits = match collect_commits_window(repo) {
        Ok(c) => c,
        Err(e) => {
            errors.push(format!("Commits: {e}"));
            Vec::new()
        }
    };
    let branches = match collect_refs_summary(repo) {
        Ok(b) => b,
        Err(e) => {
            errors.push(format!("Branches: {e}"));
            Vec::new()
        }
    };
    let remotes = match collect_remotes(repo) {
        Ok(r) => r,
        Err(e) => {
            errors.push(format!("Remotes: {e}"));
            Vec::new()
        }
    };
    let tags = match collect_tags_summary(repo) {
        Ok(t) => t,
        Err(e) => {
            errors.push(format!("Tags: {e}"));
            Vec::new()
        }
    };
    let stashes = match collect_stashes_summary(repo) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("Stashes: {e}"));
            Vec::new()
        }
    };
    let status = match collect_status_summary(repo) {
        Ok(s) => s,
        Err(e) => {
            errors.push(format!("Status: {e}"));
            empty_status()
        }
    };

    let ownership = classify_repo_ownership(&remotes, github_login);

    RepoLocalSnapshot {
        commits,
        branches,
        remotes,
        tags,
        stashes,
        status,
        repo_error: if errors.is_empty() {
            None
        } else {
            Some(errors.join("; "))
        },
        last_refresh: Some(now_millis()),
        ownership,
    }
}

pub fn spawn_repo_ownership_probe(
    path: String,
    generation: u64,
    tx: Sender<RepoLiveEvent>,
    stop: Arc<AtomicBool>,
    github_login: Option<String>,
) {
    thread::spawn(move || {
        if stop.load(Ordering::Relaxed) {
            return;
        }

        let ownership = match GitRepo::open(&path) {
            Ok(repo) => classify_repo_ownership(
                &repo.remotes().unwrap_or_default(),
                github_login.as_deref(),
            ),
            Err(_) => None,
        };

        let _ = tx.send(RepoLiveEvent::Ownership {
            path,
            generation,
            ownership,
        });
    });
}

fn watch_path(watcher: &mut impl Watcher, path: std::path::PathBuf, recursive: RecursiveMode) {
    let _ = watcher.watch(&path, recursive);
}

pub struct JobPermit;

impl Drop for JobPermit {
    fn drop(&mut self) {
        CONCURRENT_JOBS.fetch_sub(1, Ordering::SeqCst);
        tracing::debug!("Job permit dropped, slot released");
    }
}

use std::sync::atomic::AtomicUsize;

static CONCURRENT_JOBS: AtomicUsize = AtomicUsize::new(0);
const MAX_CONCURRENT_JOBS: usize = 2;

pub fn try_acquire_job() -> Option<JobPermit> {
    loop {
        let current = CONCURRENT_JOBS.load(Ordering::SeqCst);
        if current >= MAX_CONCURRENT_JOBS {
            return None;
        }
        if CONCURRENT_JOBS
            .compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return Some(JobPermit);
        }
    }
}

#[derive(Debug, Clone)]
pub enum WatchEvent {
    PathChanged(std::path::PathBuf),
    ForceRefresh,
}

#[derive(Debug, Clone, Default)]
struct DirtySlices {
    commits: bool,
    refs: bool,
    remotes: bool,
    tags: bool,
    stashes: bool,
    status: bool,
}

impl DirtySlices {
    fn mark_all(&mut self) {
        self.commits = true;
        self.refs = true;
        self.remotes = true;
        self.tags = true;
        self.stashes = true;
        self.status = true;
    }

    fn clear(&mut self) {
        self.commits = false;
        self.refs = false;
        self.remotes = false;
        self.tags = false;
        self.stashes = false;
        self.status = false;
    }

    fn any(&self) -> bool {
        self.commits || self.refs || self.remotes || self.tags || self.stashes || self.status
    }
}

fn handle_watch_path(dirty: &mut DirtySlices, path: &std::path::Path) {
    let filename = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
    if filename == "HEAD" {
        dirty.refs = true;
        dirty.commits = true;
    } else if filename == "index" {
        dirty.status = true;
    } else if filename == "packed-refs" {
        dirty.refs = true;
        dirty.tags = true;
    } else if filename == "FETCH_HEAD" {
        dirty.refs = true;
        dirty.remotes = true;
    } else if filename == "config" {
        dirty.remotes = true;
    } else {
        let path_str = path.to_string_lossy();
        if path_str.contains("refs/heads") || path_str.contains("refs/remotes") {
            dirty.refs = true;
            dirty.commits = true;
        } else if path_str.contains("refs/tags") {
            dirty.tags = true;
        }
    }
}

fn watch_repo_paths(watcher: &mut impl Watcher, repo: &GitRepo) {
    let git_dir = repo.git_dir_path();
    watch_path(watcher, git_dir.join("HEAD"), RecursiveMode::NonRecursive);
    watch_path(watcher, git_dir.join("index"), RecursiveMode::NonRecursive);
    watch_path(
        watcher,
        git_dir.join("packed-refs"),
        RecursiveMode::NonRecursive,
    );
    watch_path(
        watcher,
        git_dir.join("FETCH_HEAD"),
        RecursiveMode::NonRecursive,
    );
    watch_path(watcher, git_dir.join("config"), RecursiveMode::NonRecursive);
    watch_path(
        watcher,
        git_dir.join("refs/heads"),
        RecursiveMode::Recursive,
    );
    watch_path(
        watcher,
        git_dir.join("refs/remotes"),
        RecursiveMode::Recursive,
    );
    watch_path(watcher, git_dir.join("refs/tags"), RecursiveMode::Recursive);
}

pub fn spawn_repo_tracker(
    path: String,
    generation: u64,
    tx: Sender<RepoLiveEvent>,
    stop: Arc<AtomicBool>,
    ctx: egui::Context,
    github_login: Option<String>,
) -> std::sync::mpsc::Sender<WatchEvent> {
    let (watch_tx, watch_rx) = std::sync::mpsc::channel::<WatchEvent>();
    let watch_tx_clone = watch_tx.clone();
    let path_clone = path.clone();

    thread::spawn(move || {
        let path = path_clone;
        tracing::info!(repo = %path, "Starting live git tracker");
        let repo = match GitRepo::open(&path) {
            Ok(repo) => repo,
            Err(err) => {
                let snapshot = RepoLocalSnapshot {
                    commits: Vec::new(),
                    branches: Vec::new(),
                    remotes: Vec::new(),
                    tags: Vec::new(),
                    stashes: Vec::new(),
                    status: empty_status(),
                    repo_error: Some(err.to_string()),
                    last_refresh: Some(now_millis()),
                    ownership: None,
                };
                let _ = tx.send(RepoLiveEvent::Local {
                    path,
                    generation,
                    snapshot,
                });
                ctx.request_repaint();
                return;
            }
        };

        // Cache state hydration
        let mut cached_local = None;
        let mut cached_remote = None;
        let mut prs_etag = None;
        let mut actions_etag = None;
        let mut releases_etag = None;
        let mut packages_container_etag = None;
        let mut packages_npm_etag = None;

        if let Some(disk_cache) = crate::git::cache::load_cache(&path, github_login.as_deref()) {
            cached_local = Some(disk_cache.local_snapshot.to_snapshot());
            cached_remote = disk_cache.remote_snapshot;
            prs_etag = disk_cache.prs_etag;
            actions_etag = disk_cache.actions_etag;
            releases_etag = disk_cache.releases_etag;
            packages_container_etag = disk_cache.packages_container_etag;
            packages_npm_etag = disk_cache.packages_npm_etag;
        }

        let mut current_local = cached_local.unwrap_or_else(|| RepoLocalSnapshot {
            commits: Vec::new(),
            branches: Vec::new(),
            remotes: Vec::new(),
            tags: Vec::new(),
            stashes: Vec::new(),
            status: empty_status(),
            repo_error: None,
            last_refresh: None,
            ownership: None,
        });

        let watch_tx_watcher = watch_tx.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<notify::Event, _>| {
            if let Ok(event) = res {
                for p in event.paths {
                    let _ = watch_tx_watcher.send(WatchEvent::PathChanged(p));
                }
            }
        })
        .ok();

        if let Some(ref mut watcher) = watcher {
            watch_repo_paths(watcher, &repo);
        }

        tracing::debug!(repo = %path, "Live tracker watching repository paths");

        let mut dirty_slices = DirtySlices::default();

        // Compute initial fingerprint check
        let current_fps = crate::git::cache::compute_repo_fingerprints(&path);
        let mut stored_fps = crate::git::cache::load_fingerprints(&path).unwrap_or(None);

        if let Some(ref stored) = stored_fps {
            if current_fps.head != stored.head
                || current_fps.refs_heads != stored.refs_heads
                || current_fps.refs_remotes != stored.refs_remotes
                || current_fps.packed_refs != stored.packed_refs
            {
                dirty_slices.refs = true;
                dirty_slices.commits = true;
            }
            if current_fps.refs_tags != stored.refs_tags
                || current_fps.packed_refs != stored.packed_refs
            {
                dirty_slices.tags = true;
            }
            if current_fps.config != stored.config
                || current_fps.refs_remotes != stored.refs_remotes
            {
                dirty_slices.remotes = true;
            }
            if current_fps.index != stored.index {
                dirty_slices.status = true;
            }
            dirty_slices.stashes = true;
        } else {
            dirty_slices.mark_all();
        }

        let remote_poll_interval = Duration::from_secs(180);
        let mut last_remote_refresh = Instant::now() - remote_poll_interval;
        if let Some(ref r) = cached_remote {
            if let Some(ref_time) = r.last_refresh {
                let elapsed_ms = now_millis().saturating_sub(ref_time);
                let elapsed_secs = (elapsed_ms / 1000) as u64;
                if elapsed_secs < 180 {
                    last_remote_refresh = Instant::now() - Duration::from_secs(180 - elapsed_secs);
                }
            }
        }

        const ACTIVE_POLL_INTERVAL: Duration = Duration::from_secs(10);
        const IDLE_POLL_INTERVAL: Duration = Duration::from_secs(60);
        const IDLE_TIMEOUT: Duration = Duration::from_secs(300);

        let mut last_status_poll = Instant::now();
        let mut last_activity_time = Instant::now();
        let debounce = Duration::from_millis(250);
        let mut last_event_time = None;

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            match watch_rx.recv_timeout(Duration::from_millis(250)) {
                Ok(WatchEvent::PathChanged(p)) => {
                    handle_watch_path(&mut dirty_slices, &p);
                    last_event_time = Some(Instant::now());
                    last_activity_time = Instant::now();
                }
                Ok(WatchEvent::ForceRefresh) => {
                    dirty_slices.mark_all();
                    last_remote_refresh = Instant::now() - remote_poll_interval;
                    last_event_time = None;
                    last_activity_time = Instant::now();
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            let current_poll_interval = if last_activity_time.elapsed() < IDLE_TIMEOUT {
                ACTIVE_POLL_INTERVAL
            } else {
                IDLE_POLL_INTERVAL
            };

            if last_status_poll.elapsed() >= current_poll_interval {
                dirty_slices.status = true;
                last_status_poll = Instant::now();
            }

            let should_revalidate =
                dirty_slices.any() && last_event_time.is_none_or(|t| t.elapsed() >= debounce);

            if should_revalidate {
                if let Some(_permit) = try_acquire_job() {
                    tracing::info!(
                        repo = %path,
                        commits = dirty_slices.commits,
                        refs = dirty_slices.refs,
                        remotes = dirty_slices.remotes,
                        tags = dirty_slices.tags,
                        status = dirty_slices.status,
                        "Starting local git slice revalidation"
                    );
                    let mut changed = false;
                    let mut errors = Vec::new();
                    let mut status_changed = false;
                    let mut commits_changed = false;
                    let mut refs_changed = false;
                    let mut remotes_changed = false;
                    let mut tags_changed = false;
                    let mut stashes_changed = false;

                    if dirty_slices.commits {
                        match collect_commits_window(&repo) {
                            Ok(c) => {
                                if current_local.commits != c {
                                    current_local.commits = c;
                                    commits_changed = true;
                                    changed = true;
                                }
                            }
                            Err(e) => errors.push(format!("Commits: {e}")),
                        }
                    }
                    if dirty_slices.refs {
                        match collect_refs_summary(&repo) {
                            Ok(b) => {
                                if current_local.branches != b {
                                    current_local.branches = b;
                                    refs_changed = true;
                                    changed = true;
                                }
                            }
                            Err(e) => errors.push(format!("Branches: {e}")),
                        }
                    }
                    if dirty_slices.remotes {
                        match collect_remotes(&repo) {
                            Ok(r) => {
                                if current_local.remotes != r {
                                    current_local.remotes = r;
                                    current_local.ownership = classify_repo_ownership(
                                        &current_local.remotes,
                                        github_login.as_deref(),
                                    );
                                    remotes_changed = true;
                                    changed = true;
                                }
                            }
                            Err(e) => errors.push(format!("Remotes: {e}")),
                        }
                    }
                    if dirty_slices.tags {
                        match collect_tags_summary(&repo) {
                            Ok(t) => {
                                if current_local.tags != t {
                                    current_local.tags = t;
                                    tags_changed = true;
                                    changed = true;
                                }
                            }
                            Err(e) => errors.push(format!("Tags: {e}")),
                        }
                    }
                    if dirty_slices.stashes {
                        match collect_stashes_summary(&repo) {
                            Ok(s) => {
                                if current_local.stashes != s {
                                    current_local.stashes = s;
                                    stashes_changed = true;
                                    changed = true;
                                }
                            }
                            Err(e) => errors.push(format!("Stashes: {e}")),
                        }
                    }
                    if dirty_slices.status {
                        match collect_status_summary(&repo) {
                            Ok(s) => {
                                if current_local.status != s {
                                    current_local.status = s;
                                    status_changed = true;
                                    changed = true;
                                }
                            }
                            Err(e) => errors.push(format!("Status: {e}")),
                        }
                    }

                    let new_fps = crate::git::cache::compute_repo_fingerprints(&path);
                    let fps_changed = if let Some(ref stored) = stored_fps {
                        *stored != new_fps
                    } else {
                        true
                    };

                    if changed {
                        current_local.repo_error = if errors.is_empty() {
                            None
                        } else {
                            Some(errors.join("; "))
                        };
                        current_local.last_refresh = Some(now_millis());

                        let _ = tx.send(RepoLiveEvent::Local {
                            path: path.clone(),
                            generation,
                            snapshot: current_local.clone(),
                        });
                        ctx.request_repaint();

                        if stored_fps.is_none() {
                            let dc = crate::git::cache::DiskCache {
                                schema_version: crate::git::cache::SCHEMA_VERSION,
                                repo_path: path.clone(),
                                repo_fingerprint: String::new(),
                                captured_at: now_millis(),
                                local_snapshot:
                                    crate::git::cache::BoundedLocalSnapshot::from_snapshot(
                                        &current_local,
                                    ),
                                remote_snapshot: cached_remote.clone(),
                                prs_etag: prs_etag.clone(),
                                actions_etag: actions_etag.clone(),
                                releases_etag: releases_etag.clone(),
                                packages_container_etag: packages_container_etag.clone(),
                                packages_npm_etag: packages_npm_etag.clone(),
                            };
                            let _ = crate::git::cache::save_cache(&dc, github_login.as_deref());
                        } else {
                            if status_changed {
                                let _ = crate::git::cache::save_status_slice(
                                    &path,
                                    &current_local.status,
                                    &new_fps,
                                );
                            }
                            if commits_changed {
                                let _ = crate::git::cache::save_commits_slice(
                                    &path,
                                    &current_local.commits,
                                    &new_fps,
                                );
                            }
                            if refs_changed {
                                let _ = crate::git::cache::save_refs_slice(
                                    &path,
                                    &current_local.branches,
                                    &new_fps,
                                );
                            }
                            if remotes_changed {
                                let _ = crate::git::cache::save_remotes_slice(
                                    &path,
                                    &current_local.remotes,
                                    &new_fps,
                                );
                            }
                            if tags_changed {
                                let _ = crate::git::cache::save_tags_slice(
                                    &path,
                                    &current_local.tags,
                                    &new_fps,
                                );
                            }
                            if stashes_changed {
                                let _ = crate::git::cache::save_stashes_slice(
                                    &path,
                                    &current_local.stashes,
                                    &new_fps,
                                );
                            }
                        }
                        stored_fps = Some(new_fps);
                    } else if fps_changed {
                        let _ = crate::git::cache::save_fingerprints(&path, &new_fps);
                        stored_fps = Some(new_fps);
                    }

                    if !errors.is_empty() {
                        tracing::warn!(repo = %path, errors = ?errors, "Local slice revalidation completed with errors");
                    } else if changed {
                        tracing::info!(repo = %path, "Local slice revalidation completed successfully");
                    }

                    dirty_slices.clear();
                    last_event_time = None;
                }
            }

            if last_remote_refresh.elapsed() >= remote_poll_interval
                && ownership_gate_allows_remote(&current_local.remotes, github_login.as_deref())
            {
                let creds = credentials::load_credentials();
                if let Some(token) = creds.github_token.as_ref() {
                    if let Some((owner, repo_name)) = current_local
                        .remotes
                        .iter()
                        .find_map(|r| parse_github_remote(&r.url))
                    {
                        if let Some(_permit) = try_acquire_job() {
                            tracing::info!(repo = %repo_name, owner = %owner, "Triggered GitHub remote metadata sync");
                            let mut remote_changed = false;
                            let mut errors = Vec::new();

                            match github_api::list_pull_requests_conditional(
                                token,
                                &owner,
                                &repo_name,
                                prs_etag.as_deref(),
                            ) {
                                Ok(github_api::GitHubResponse::Fresh { data, etag }) => {
                                    prs_etag = etag;
                                    let prs = data
                                        .into_iter()
                                        .map(|pr| GitHubPullRequest {
                                            number: pr.number,
                                            title: pr.title,
                                            state: pr.state,
                                            user_login: pr.user_login,
                                            html_url: pr.html_url,
                                            head_ref: pr.head_ref,
                                            base_ref: pr.base_ref,
                                            draft: pr.draft,
                                        })
                                        .collect();
                                    if let Some(ref mut r) = cached_remote {
                                        r.pull_requests = prs;
                                    } else {
                                        cached_remote = Some(RepoRemoteSnapshot {
                                            pull_requests: prs,
                                            action_runs: Vec::new(),
                                            releases: Vec::new(),
                                            packages: Vec::new(),
                                            github_error: None,
                                            last_refresh: Some(now_millis()),
                                            ownership: RepoOwnership::Owned,
                                        });
                                    }
                                    remote_changed = true;
                                }
                                Ok(github_api::GitHubResponse::NotModified) => {
                                    if let Some(ref login) = github_login {
                                        let _ = crate::git::cache::touch_github_cache_entry(
                                            &path,
                                            login,
                                            "prs",
                                            now_millis(),
                                            true,
                                        );
                                    }
                                }
                                Ok(github_api::GitHubResponse::Error(e)) => {
                                    errors.push(format!("PRs: {e}"))
                                }
                                Err(e) => errors.push(format!("PRs: {e}")),
                            }

                            match github_api::list_action_runs_conditional(
                                token,
                                &owner,
                                &repo_name,
                                actions_etag.as_deref(),
                            ) {
                                Ok(github_api::GitHubResponse::Fresh { data, etag }) => {
                                    actions_etag = etag;
                                    let runs = data
                                        .into_iter()
                                        .map(|run| GitHubActionRun {
                                            id: run.id,
                                            name: run.name,
                                            status: run.status,
                                            conclusion: run.conclusion,
                                            html_url: run.html_url,
                                            head_branch: run.head_branch,
                                        })
                                        .collect();
                                    if let Some(ref mut r) = cached_remote {
                                        r.action_runs = runs;
                                    }
                                    remote_changed = true;
                                }
                                Ok(github_api::GitHubResponse::NotModified) => {
                                    if let Some(ref login) = github_login {
                                        let _ = crate::git::cache::touch_github_cache_entry(
                                            &path,
                                            login,
                                            "actions",
                                            now_millis(),
                                            true,
                                        );
                                    }
                                }
                                Ok(github_api::GitHubResponse::Error(e)) => {
                                    errors.push(format!("Actions: {e}"))
                                }
                                Err(e) => errors.push(format!("Actions: {e}")),
                            }

                            match github_api::list_releases_conditional(
                                token,
                                &owner,
                                &repo_name,
                                releases_etag.as_deref(),
                            ) {
                                Ok(github_api::GitHubResponse::Fresh { data, etag }) => {
                                    releases_etag = etag;
                                    let rels = data
                                        .into_iter()
                                        .map(|rel| GitHubRelease {
                                            name: rel.name,
                                            tag_name: rel.tag_name,
                                            body: rel.body,
                                            html_url: rel.html_url,
                                            draft: rel.draft,
                                            prerelease: rel.prerelease,
                                        })
                                        .collect();
                                    if let Some(ref mut r) = cached_remote {
                                        r.releases = rels;
                                    }
                                    remote_changed = true;
                                }
                                Ok(github_api::GitHubResponse::NotModified) => {
                                    if let Some(ref login) = github_login {
                                        let _ = crate::git::cache::touch_github_cache_entry(
                                            &path,
                                            login,
                                            "releases",
                                            now_millis(),
                                            true,
                                        );
                                    }
                                }
                                Ok(github_api::GitHubResponse::Error(e)) => {
                                    errors.push(format!("Releases: {e}"))
                                }
                                Err(e) => errors.push(format!("Releases: {e}")),
                            }

                            let is_org =
                                match github_api::get_repo_owner_type(token, &owner, &repo_name) {
                                    Ok(owner_type) => owner_type.to_lowercase() == "organization",
                                    Err(_) => false,
                                };
                            match github_api::list_packages_conditional(
                                token,
                                &owner,
                                is_org,
                                packages_container_etag.as_deref(),
                                packages_npm_etag.as_deref(),
                            ) {
                                Ok((container_res, npm_res)) => {
                                    let mut pkgs = Vec::new();
                                    let mut container_changed = false;
                                    let mut npm_changed = false;

                                    match container_res {
                                        github_api::GitHubResponse::Fresh { data, etag } => {
                                            packages_container_etag = etag;
                                            pkgs.extend(data.into_iter().map(|p| GitHubPackage {
                                                name: p.name,
                                                package_type: p.package_type,
                                                html_url: p.html_url,
                                            }));
                                            container_changed = true;
                                        }
                                        github_api::GitHubResponse::NotModified => {
                                            if let Some(ref r) = cached_remote {
                                                pkgs.extend(
                                                    r.packages
                                                        .iter()
                                                        .filter(|p| p.package_type == "container")
                                                        .cloned(),
                                                );
                                            }
                                            if let Some(ref login) = github_login {
                                                let _ = crate::git::cache::touch_github_cache_entry(
                                                    &path,
                                                    login,
                                                    "packages_container",
                                                    now_millis(),
                                                    true,
                                                );
                                            }
                                        }
                                        github_api::GitHubResponse::Error(e) => {
                                            errors.push(format!("Container Packages: {e}"))
                                        }
                                    }

                                    match npm_res {
                                        github_api::GitHubResponse::Fresh { data, etag } => {
                                            packages_npm_etag = etag;
                                            pkgs.extend(data.into_iter().map(|p| GitHubPackage {
                                                name: p.name,
                                                package_type: p.package_type,
                                                html_url: p.html_url,
                                            }));
                                            npm_changed = true;
                                        }
                                        github_api::GitHubResponse::NotModified => {
                                            if let Some(ref r) = cached_remote {
                                                pkgs.extend(
                                                    r.packages
                                                        .iter()
                                                        .filter(|p| p.package_type == "npm")
                                                        .cloned(),
                                                );
                                            }
                                            if let Some(ref login) = github_login {
                                                let _ = crate::git::cache::touch_github_cache_entry(
                                                    &path,
                                                    login,
                                                    "packages_npm",
                                                    now_millis(),
                                                    true,
                                                );
                                            }
                                        }
                                        github_api::GitHubResponse::Error(e) => {
                                            errors.push(format!("NPM Packages: {e}"))
                                        }
                                    }

                                    if container_changed || npm_changed {
                                        if let Some(ref mut r) = cached_remote {
                                            r.packages = pkgs;
                                        }
                                        remote_changed = true;
                                    }
                                }
                                Err(e) => errors.push(format!("Packages: {e}")),
                            }

                            if remote_changed || !errors.is_empty() {
                                if let Some(ref mut r) = cached_remote {
                                    r.github_error = if errors.is_empty() {
                                        None
                                    } else {
                                        Some(errors.join(", "))
                                    };
                                    r.last_refresh = Some(now_millis());
                                }

                                if let Some(ref r) = cached_remote {
                                    let _ = tx.send(RepoLiveEvent::Remote {
                                        path: path.clone(),
                                        generation,
                                        snapshot: r.clone(),
                                    });
                                    ctx.request_repaint();
                                }

                                let new_fp = crate::git::cache::compute_repo_fingerprint(&path);
                                let dc = crate::git::cache::DiskCache {
                                    schema_version: crate::git::cache::SCHEMA_VERSION,
                                    repo_path: path.clone(),
                                    repo_fingerprint: new_fp,
                                    captured_at: now_millis(),
                                    local_snapshot:
                                        crate::git::cache::BoundedLocalSnapshot::from_snapshot(
                                            &current_local,
                                        ),
                                    remote_snapshot: cached_remote.clone(),
                                    prs_etag: prs_etag.clone(),
                                    actions_etag: actions_etag.clone(),
                                    releases_etag: releases_etag.clone(),
                                    packages_container_etag: packages_container_etag.clone(),
                                    packages_npm_etag: packages_npm_etag.clone(),
                                };
                                let _ = crate::git::cache::save_cache(&dc, github_login.as_deref());
                            }

                            if !errors.is_empty() {
                                tracing::warn!(repo = %repo_name, owner = %owner, errors = ?errors, "GitHub remote sync completed with errors");
                            } else if remote_changed {
                                tracing::info!(repo = %repo_name, owner = %owner, "GitHub remote sync completed successfully (data updated)");
                            } else {
                                tracing::info!(repo = %repo_name, owner = %owner, "GitHub remote sync completed (no changes - 304)");
                            }

                            last_remote_refresh = Instant::now();
                        }
                    }
                }
            }
        }

        tracing::info!(repo = %path, "Stopping live git tracker");
    });
    watch_tx_clone
}

fn ownership_gate_allows_remote(remotes: &[Remote], login: Option<&str>) -> bool {
    let Some(login) = login else {
        return false;
    };
    classify_repo_ownership(remotes, Some(login)) == Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_repo_ownership_owned_when_remote_matches_login() {
        let remotes = vec![Remote {
            name: "origin".to_string(),
            url: "https://github.com/alice/project".to_string(),
        }];

        assert_eq!(classify_repo_ownership(&remotes, Some("alice")), Some(true));
    }

    #[test]
    fn classify_repo_ownership_external_when_remote_differs() {
        let remotes = vec![Remote {
            name: "origin".to_string(),
            url: "git@github.com:bob/project.git".to_string(),
        }];

        assert_eq!(
            classify_repo_ownership(&remotes, Some("alice")),
            Some(false)
        );
    }

    #[test]
    fn classify_repo_ownership_unknown_without_login() {
        let remotes = vec![Remote {
            name: "origin".to_string(),
            url: "https://github.com/alice/project".to_string(),
        }];

        assert_eq!(classify_repo_ownership(&remotes, None), None);
    }

    #[test]
    fn test_concurrency_limit() {
        // Reset job slot counter before running test
        CONCURRENT_JOBS.store(0, Ordering::SeqCst);
        let permit1 = try_acquire_job();
        assert!(permit1.is_some());
        let permit2 = try_acquire_job();
        assert!(permit2.is_some());
        let permit3 = try_acquire_job();
        assert!(permit3.is_none());
        drop(permit1);
        let permit4 = try_acquire_job();
        assert!(permit4.is_some());
    }

    #[test]
    fn ownership_gate_uses_cached_remote_data() {
        let remotes = vec![Remote {
            name: "origin".to_string(),
            url: "https://github.com/alice/project".to_string(),
        }];

        assert!(ownership_gate_allows_remote(&remotes, Some("alice")));
        assert!(!ownership_gate_allows_remote(&remotes, Some("bob")));
        assert!(!ownership_gate_allows_remote(&remotes, None));
    }
}
