use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use zed::{Store, create_reducer};

use crate::git::models::{Branch, Commit, Remote, RepoStatus, Tag};

const REFRESH_INTERVAL_MS: u64 = 2000;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppState {
    pub current_repo: Option<String>,
    pub recent_repos: Vec<String>,
    pub show_window_buttons: bool,
    pub cached_commits: Vec<CachedCommit>,
    pub cached_branches: Vec<CachedBranch>,
    pub cached_remotes: Vec<CachedRemote>,
    pub cached_tags: Vec<CachedTag>,
    pub cached_status: Option<CachedRepoStatus>,
    pub last_refresh: Option<u128>,
    pub repo_error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CachedCommit {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub timestamp_secs: i64,
    pub parents: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CachedBranch {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
    pub upstream: Option<String>,
    pub tip_hash: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CachedRemote {
    pub name: String,
    pub url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CachedTag {
    pub name: String,
    pub target_hash: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CachedRepoStatus {
    pub branch: String,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub staged_files: Vec<String>,
    pub additions: usize,
    pub deletions: usize,
    pub files_changed: usize,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_repo: None,
            recent_repos: Vec::new(),
            show_window_buttons: true,
            cached_commits: Vec::new(),
            cached_branches: Vec::new(),
            cached_remotes: Vec::new(),
            cached_tags: Vec::new(),
            cached_status: None,
            last_refresh: None,
            repo_error: None,
        }
    }
}

impl AppState {
    pub fn repo_name(&self) -> Option<&str> {
        self.current_repo.as_deref().map(|p| {
            Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(p)
        })
    }

    pub fn push_recent(mut self, path: &str) -> Self {
        self.recent_repos.retain(|p| p != path);
        self.recent_repos.insert(0, path.to_string());
        if self.recent_repos.len() > 10 {
            self.recent_repos.truncate(10);
        }
        self
    }

    fn with_current_repo(mut self, repo: Option<String>) -> Self {
        self.current_repo = repo;
        self
    }

    pub fn needs_refresh(&self) -> bool {
        match self.last_refresh {
            Some(last) => {
                let elapsed = Instant::now()
                    .duration_since(Instant::now() - Duration::from_millis(last as u64));
                elapsed.as_millis() >= REFRESH_INTERVAL_MS as u128
            }
            None => true,
        }
    }

    pub fn mark_refreshed(mut self) -> Self {
        self.last_refresh = Some(Instant::now().elapsed().as_millis());
        self
    }

    pub fn with_cached_commits(mut self, commits: &[Commit]) -> Self {
        self.cached_commits = commits
            .iter()
            .map(|c| CachedCommit {
                hash: c.hash.clone(),
                short_hash: c.short_hash.clone(),
                message: c.message.clone(),
                author: c.author.clone(),
                timestamp_secs: c
                    .timestamp
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
                parents: c.parents.clone(),
            })
            .collect();
        self
    }

    pub fn with_cached_branches(mut self, branches: &[Branch]) -> Self {
        self.cached_branches = branches
            .iter()
            .map(|b| CachedBranch {
                name: b.name.clone(),
                is_current: b.is_current,
                is_remote: b.is_remote,
                upstream: b.upstream.clone(),
                tip_hash: b.tip_hash.clone(),
            })
            .collect();
        self
    }

    pub fn with_cached_remotes(mut self, remotes: &[Remote]) -> Self {
        self.cached_remotes = remotes
            .iter()
            .map(|r| CachedRemote {
                name: r.name.clone(),
                url: r.url.clone(),
            })
            .collect();
        self
    }

    pub fn with_cached_tags(mut self, tags: &[Tag]) -> Self {
        self.cached_tags = tags
            .iter()
            .map(|t| CachedTag {
                name: t.name.clone(),
                target_hash: t.target_hash.clone(),
            })
            .collect();
        self
    }

    pub fn with_cached_status(mut self, status: &RepoStatus) -> Self {
        self.cached_status = Some(CachedRepoStatus {
            branch: status.branch.clone(),
            staged_count: status.staged_count,
            unstaged_count: status.unstaged_count,
            staged_files: status.staged_files.clone(),
            additions: status.additions,
            deletions: status.deletions,
            files_changed: status.files_changed,
        });
        self
    }

    pub fn clear_cache(mut self) -> Self {
        self.cached_commits.clear();
        self.cached_branches.clear();
        self.cached_remotes.clear();
        self.cached_tags.clear();
        self.cached_status = None;
        self.last_refresh = None;
        self
    }
}

#[derive(Clone, Debug)]
pub enum AppAction {
    OpenRepo(String),
    SelectRecent(usize),
    ToggleWindowButtons(bool),
    RefreshGitData {
        commits: Vec<Commit>,
        branches: Vec<Branch>,
        remotes: Vec<Remote>,
        tags: Vec<Tag>,
        status: RepoStatus,
    },
    ClearGitCache,
    SetRepoError(Option<String>),
}

fn reducer(state: &AppState, action: &AppAction) -> AppState {
    match action {
        AppAction::OpenRepo(path) => state
            .clone()
            .push_recent(path)
            .with_current_repo(Some(path.clone()))
            .clear_cache(),
        AppAction::SelectRecent(index) => {
            if let Some(path) = state.recent_repos.get(*index).cloned() {
                AppState {
                    current_repo: Some(path),
                    recent_repos: state.recent_repos.clone(),
                    show_window_buttons: state.show_window_buttons,
                    cached_commits: Vec::new(),
                    cached_branches: Vec::new(),
                    cached_remotes: Vec::new(),
                    cached_tags: Vec::new(),
                    cached_status: None,
                    last_refresh: None,
                    repo_error: None,
                }
            } else {
                state.clone()
            }
        }
        AppAction::ToggleWindowButtons(show) => AppState {
            current_repo: state.current_repo.clone(),
            recent_repos: state.recent_repos.clone(),
            show_window_buttons: *show,
            cached_commits: state.cached_commits.clone(),
            cached_branches: state.cached_branches.clone(),
            cached_remotes: state.cached_remotes.clone(),
            cached_tags: state.cached_tags.clone(),
            cached_status: state.cached_status.clone(),
            last_refresh: state.last_refresh,
            repo_error: state.repo_error.clone(),
        },
        AppAction::RefreshGitData {
            commits,
            branches,
            remotes,
            tags,
            status,
        } => state
            .clone()
            .with_cached_commits(commits)
            .with_cached_branches(branches)
            .with_cached_remotes(remotes)
            .with_cached_tags(tags)
            .with_cached_status(status)
            .mark_refreshed(),
        AppAction::ClearGitCache => state.clone().clear_cache(),
        AppAction::SetRepoError(error) => AppState {
            current_repo: state.current_repo.clone(),
            recent_repos: state.recent_repos.clone(),
            show_window_buttons: state.show_window_buttons,
            cached_commits: state.cached_commits.clone(),
            cached_branches: state.cached_branches.clone(),
            cached_remotes: state.cached_remotes.clone(),
            cached_tags: state.cached_tags.clone(),
            cached_status: state.cached_status.clone(),
            last_refresh: state.last_refresh,
            repo_error: error.clone(),
        },
    }
}

pub type AppStore = Store<AppState, AppAction>;

pub fn create_store() -> Arc<AppStore> {
    let initial = AppState::default();
    Arc::new(Store::new(initial, Box::new(create_reducer(reducer))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = AppState::default();
        assert!(state.current_repo.is_none());
        assert!(state.recent_repos.is_empty());
        assert!(state.show_window_buttons);
        assert!(state.cached_commits.is_empty());
        assert!(state.cached_branches.is_empty());
        assert!(state.cached_status.is_none());
    }

    #[test]
    fn test_repo_name_from_path() {
        let state = AppState {
            current_repo: Some("/home/user/projects/my-repo".to_string()),
            ..Default::default()
        };
        assert_eq!(state.repo_name(), Some("my-repo"));
    }

    #[test]
    fn test_repo_name_none() {
        let state = AppState::default();
        assert_eq!(state.repo_name(), None);
    }

    #[test]
    fn test_push_recent_adds_to_empty() {
        let state = AppState::default();
        let state = state.push_recent("/path/to/repo");
        assert_eq!(state.recent_repos.len(), 1);
        assert_eq!(state.recent_repos[0], "/path/to/repo");
    }

    #[test]
    fn test_push_recent_moves_existing_to_front() {
        let state = AppState {
            recent_repos: vec!["/a".to_string(), "/b".to_string()],
            ..Default::default()
        };
        let state = state.push_recent("/b");
        assert_eq!(state.recent_repos[0], "/b");
        assert_eq!(state.recent_repos.len(), 2);
    }

    #[test]
    fn test_push_recent_truncates_at_10() {
        let mut state = AppState {
            recent_repos: (0..10).map(|i| format!("/repo{}", i)).collect(),
            ..Default::default()
        };
        state = state.push_recent("/new-repo");
        assert_eq!(state.recent_repos.len(), 10);
        assert_eq!(state.recent_repos[0], "/new-repo");
    }

    #[test]
    fn test_open_repo_action() {
        let state = AppState::default();
        let result = reducer(&state, &AppAction::OpenRepo("/test".to_string()));
        assert_eq!(result.current_repo, Some("/test".to_string()));
        assert_eq!(result.recent_repos.len(), 1);
    }

    #[test]
    fn test_select_recent_action() {
        let state = AppState {
            recent_repos: vec!["/a".to_string(), "/b".to_string()],
            ..Default::default()
        };
        let result = reducer(&state, &AppAction::SelectRecent(1));
        assert_eq!(result.current_repo, Some("/b".to_string()));
    }

    #[test]
    fn test_select_recent_invalid_index() {
        let state = AppState::default();
        let result = reducer(&state, &AppAction::SelectRecent(5));
        assert_eq!(result, state);
    }

    #[test]
    fn test_toggle_window_buttons() {
        let state = AppState::default();
        let result = reducer(&state, &AppAction::ToggleWindowButtons(false));
        assert!(!result.show_window_buttons);
    }

    #[test]
    fn test_create_store() {
        let store = create_store();
        let state = store.get_state();
        assert!(state.current_repo.is_none());
        assert!(state.show_window_buttons);
    }

    #[test]
    fn test_needs_refresh_initial() {
        let state = AppState::default();
        assert!(state.needs_refresh());
    }

    #[test]
    fn test_clear_cache() {
        let state = AppState::default()
            .with_cached_commits(&[])
            .with_cached_branches(&[])
            .mark_refreshed();
        let cleared = state.clear_cache();
        assert!(cleared.cached_commits.is_empty());
        assert!(cleared.cached_branches.is_empty());
        assert!(cleared.last_refresh.is_none());
    }
}
