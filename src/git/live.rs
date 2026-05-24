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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RepoOwnership {
    Owned,
    External,
    Unknown,
}

#[derive(Clone, Debug)]
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

fn owned_by_authed_user(remote_urls: &[Remote], login: Option<&str>) -> Option<bool> {
    let login = login?;
    Some(remote_urls.iter().any(|remote| {
        remote.url.contains(&format!("github.com/{}/", login))
            || remote.url.contains(&format!("git@github.com:{}", login))
    }))
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

fn parse_github_remote(url: &str) -> Option<(String, String)> {
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

pub fn collect_local_snapshot(repo: &GitRepo, github_login: Option<&str>) -> RepoLocalSnapshot {
    let mut errors = Vec::new();

    let commits = match repo.commits(Some(200)) {
        Ok(commits) => commits,
        Err(err) => {
            errors.push(err.to_string());
            Vec::new()
        }
    };
    let branches = match repo.branches() {
        Ok(branches) => branches,
        Err(err) => {
            errors.push(err.to_string());
            Vec::new()
        }
    };
    let remotes = match repo.remotes() {
        Ok(remotes) => remotes,
        Err(err) => {
            errors.push(err.to_string());
            Vec::new()
        }
    };
    let tags = match repo.tags() {
        Ok(tags) => tags,
        Err(err) => {
            errors.push(err.to_string());
            Vec::new()
        }
    };
    let stashes = match repo.stashes() {
        Ok(stashes) => stashes,
        Err(err) => {
            errors.push(err.to_string());
            Vec::new()
        }
    };
    let status = match repo.status() {
        Ok(status) => status,
        Err(err) => {
            errors.push(err.to_string());
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

fn collect_remote_snapshot(
    repo: &GitRepo,
    github_login: Option<&str>,
) -> Option<RepoRemoteSnapshot> {
    let remotes = repo.remotes().ok()?;
    let (owner, repo_name) = remotes
        .iter()
        .find_map(|remote| parse_github_remote(&remote.url))?;

    let ownership = match classify_repo_ownership(&remotes, github_login) {
        Some(true) => RepoOwnership::Owned,
        Some(false) => RepoOwnership::External,
        None => RepoOwnership::Unknown,
    };

    let creds = credentials::load_credentials();
    let token = creds.github_token?;

    let mut errors = Vec::new();

    let pull_requests = match github_api::list_pull_requests(&token, &owner, &repo_name) {
        Ok(list) => list
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
            .collect(),
        Err(err) => {
            errors.push(format!("PRs: {}", err));
            Vec::new()
        }
    };

    let action_runs = match github_api::list_action_runs(&token, &owner, &repo_name) {
        Ok(list) => list
            .into_iter()
            .map(|run| GitHubActionRun {
                id: run.id,
                name: run.name,
                status: run.status,
                conclusion: run.conclusion,
                html_url: run.html_url,
                head_branch: run.head_branch,
            })
            .collect(),
        Err(err) => {
            errors.push(format!("Actions: {}", err));
            Vec::new()
        }
    };

    let releases = match github_api::list_releases(&token, &owner, &repo_name) {
        Ok(list) => list
            .into_iter()
            .map(|release| GitHubRelease {
                name: release.name,
                tag_name: release.tag_name,
                body: release.body,
                html_url: release.html_url,
                draft: release.draft,
                prerelease: release.prerelease,
            })
            .collect(),
        Err(err) => {
            errors.push(format!("Releases: {}", err));
            Vec::new()
        }
    };

    let is_org = match github_api::get_repo_owner_type(&token, &owner, &repo_name) {
        Ok(owner_type) => owner_type.to_lowercase() == "organization",
        Err(err) => {
            errors.push(format!("Owner type: {}", err));
            false
        }
    };

    let packages = match github_api::list_packages(&token, &owner, is_org) {
        Ok(list) => list
            .into_iter()
            .map(|package| GitHubPackage {
                name: package.name,
                package_type: package.package_type,
                html_url: package.html_url,
            })
            .collect(),
        Err(err) => {
            errors.push(format!("Packages: {}", err));
            Vec::new()
        }
    };

    Some(RepoRemoteSnapshot {
        pull_requests,
        action_runs,
        releases,
        packages,
        github_error: if errors.is_empty() {
            None
        } else {
            Some(errors.join(", "))
        },
        last_refresh: Some(now_millis()),
        ownership,
    })
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

fn watch_repo_paths(watcher: &mut impl Watcher, repo: &GitRepo) {
    if let Some(workdir) = repo.workdir_path() {
        watch_path(watcher, workdir, RecursiveMode::Recursive);
    }

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
) {
    thread::spawn(move || {
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

        let (watch_tx, watch_rx) = std::sync::mpsc::channel::<()>();
        let mut watcher = notify::recommended_watcher(move |_event| {
            let _ = watch_tx.send(());
        })
        .ok();

        if let Some(ref mut watcher) = watcher {
            watch_repo_paths(watcher, &repo);
        }

        tracing::debug!(repo = %path, "Live tracker watching repository paths");

        let local_poll_interval = Duration::from_secs(15);
        let remote_poll_interval = Duration::from_secs(180);
        let debounce = Duration::from_millis(250);

        let mut last_local_refresh = Instant::now() - local_poll_interval;
        let mut last_remote_refresh = Instant::now() - remote_poll_interval;
        let mut last_event = Instant::now();
        let mut local_dirty = true;

        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }

            match watch_rx.recv_timeout(Duration::from_secs(1)) {
                Ok(()) => {
                    local_dirty = true;
                    last_event = Instant::now();
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }

            if local_dirty
                && (last_event.elapsed() >= debounce
                    || last_local_refresh.elapsed() >= local_poll_interval)
            {
                let snapshot = collect_local_snapshot(&repo, github_login.as_deref());
                let _ = tx.send(RepoLiveEvent::Local {
                    path: path.clone(),
                    generation,
                    snapshot,
                });
                ctx.request_repaint();
                local_dirty = false;
                last_local_refresh = Instant::now();
            } else if last_local_refresh.elapsed() >= local_poll_interval {
                let snapshot = collect_local_snapshot(&repo, github_login.as_deref());
                let _ = tx.send(RepoLiveEvent::Local {
                    path: path.clone(),
                    generation,
                    snapshot,
                });
                ctx.request_repaint();
                last_local_refresh = Instant::now();
            }

            if ownership_gate_allows_remote(&repo, github_login.as_deref())
                && last_remote_refresh.elapsed() >= remote_poll_interval
            {
                if let Some(snapshot) = collect_remote_snapshot(&repo, github_login.as_deref()) {
                    tracing::info!(repo = %path, "Refreshing GitHub remote data for owned repo");
                    let _ = tx.send(RepoLiveEvent::Remote {
                        path: path.clone(),
                        generation,
                        snapshot,
                    });
                    ctx.request_repaint();
                }
                last_remote_refresh = Instant::now();
            }
        }

        tracing::info!(repo = %path, "Stopping live git tracker");
    });
}

fn ownership_gate_allows_remote(repo: &GitRepo, login: Option<&str>) -> bool {
    let Some(login) = login else {
        return false;
    };
    let Ok(remotes) = repo.remotes() else {
        return false;
    };
    owned_by_authed_user(&remotes, Some(login)) == Some(true)
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
}
