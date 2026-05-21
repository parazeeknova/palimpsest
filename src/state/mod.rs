use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use zed::{Store, create_reducer};

use crate::git::models::{Branch, Commit, Remote, RepoStatus, Tag};

const SESSION_VERSION: u32 = 2;
const SESSION_FILE_NAME: &str = "session.json";
const APP_ID: &str = "Palimpsest";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSession {
    pub version: u32,
    pub open_tabs: Vec<String>,
    pub active_tab: Option<usize>,
    pub recent_repos: Vec<String>,
    pub show_window_buttons: bool,
}

impl Default for AppSession {
    fn default() -> Self {
        Self {
            version: SESSION_VERSION,
            open_tabs: Vec::new(),
            active_tab: None,
            recent_repos: Vec::new(),
            show_window_buttons: true,
        }
    }
}

impl AppSession {
    fn normalize(mut self) -> Self {
        self.version = SESSION_VERSION;
        self.active_tab = match self.active_tab {
            Some(index) if index < self.open_tabs.len() => Some(index),
            _ if self.open_tabs.is_empty() => None,
            _ => Some(0),
        };
        self
    }

    fn session_path() -> Option<PathBuf> {
        eframe::storage_dir(APP_ID).map(|path| path.join(SESSION_FILE_NAME))
    }

    pub fn from_state(state: &AppState) -> Self {
        Self {
            version: SESSION_VERSION,
            open_tabs: state.open_tabs.clone(),
            active_tab: state.active_tab,
            recent_repos: state.recent_repos.clone(),
            show_window_buttons: state.show_window_buttons,
        }
        .normalize()
    }

    pub fn into_state(self) -> AppState {
        let session = self.normalize();
        let current_repo = session
            .active_tab
            .and_then(|index| session.open_tabs.get(index).cloned());

        AppState {
            open_tabs: session.open_tabs,
            active_tab: session.active_tab,
            current_repo,
            recent_repos: session.recent_repos,
            show_window_buttons: session.show_window_buttons,
            cached_commits: Vec::new(),
            cached_branches: Vec::new(),
            cached_remotes: Vec::new(),
            cached_tags: Vec::new(),
            cached_status: None,
            last_refresh: None,
            repo_error: None,
            manager_selected_repo: None,
            manager_details: None,
        }
    }

    pub fn load() -> Self {
        let Some(path) = Self::session_path() else {
            return Self::default();
        };

        match fs::read_to_string(&path) {
            Ok(contents) => match serde_json::from_str::<AppSession>(&contents) {
                Ok(session) => session.normalize(),
                Err(error) => {
                    tracing::warn!(path = %path.display(), error = %error, "Failed to parse persisted session");
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                tracing::warn!(path = %path.display(), error = %error, "Failed to read persisted session");
                Self::default()
            }
        }
    }

    pub fn save(&self) {
        let Some(path) = Self::session_path() else {
            tracing::warn!("Unable to resolve storage directory for persisted session");
            return;
        };

        if let Some(parent) = path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                tracing::warn!(path = %parent.display(), error = %error, "Failed to create session directory");
                return;
            }
        }

        let serialized = match serde_json::to_string_pretty(&self.clone().normalize()) {
            Ok(serialized) => serialized,
            Err(error) => {
                tracing::warn!(error = %error, "Failed to serialize session");
                return;
            }
        };

        let temp_path = path.with_extension("tmp");
        if let Err(error) = fs::write(&temp_path, serialized) {
            tracing::warn!(path = %temp_path.display(), error = %error, "Failed to write session temp file");
            return;
        }

        if let Err(error) = fs::rename(&temp_path, &path) {
            tracing::warn!(from = %temp_path.display(), to = %path.display(), error = %error, "Failed to commit session file");
            let _ = fs::remove_file(&temp_path);
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    pub open_tabs: Vec<String>,
    pub active_tab: Option<usize>,
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
    pub manager_selected_repo: Option<String>,
    pub manager_details: Option<ManagerRepoDetails>,
}

impl PartialEq for AppState {
    fn eq(&self, other: &Self) -> bool {
        self.open_tabs == other.open_tabs
            && self.active_tab == other.active_tab
            && self.current_repo == other.current_repo
            && self.recent_repos == other.recent_repos
            && self.show_window_buttons == other.show_window_buttons
            && self.cached_commits == other.cached_commits
            && self.cached_branches == other.cached_branches
            && self.cached_remotes == other.cached_remotes
            && self.cached_tags == other.cached_tags
            && self.cached_status == other.cached_status
            && self.last_refresh == other.last_refresh
            && self.repo_error == other.repo_error
            && self.manager_selected_repo == other.manager_selected_repo
            && self.manager_details == other.manager_details
    }
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
pub enum CachedFileChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChanged,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CachedFileStatus {
    pub path: String,
    pub old_path: Option<String>,
    pub kind: CachedFileChangeKind,
    pub staged: bool,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CachedRepoStatus {
    pub branch: String,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub staged_files: Vec<CachedFileStatus>,
    pub unstaged_files: Vec<CachedFileStatus>,
    pub additions: usize,
    pub deletions: usize,
    pub files_changed: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManagerRepoDetails {
    pub repo_path: String,
    pub repo_name: String,
    pub branch: String,
    pub uncommitted_files: usize,
    pub total_commits: usize,
    pub initial_commit_date: String,
    pub last_commit_date: String,
    pub remotes: Vec<ManagerRemote>,
    pub branches: Vec<ManagerBranch>,
    pub tags: Vec<ManagerTag>,
    pub commits: Vec<ManagerCommit>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManagerRemote {
    pub name: String,
    pub url: String,
    pub is_github: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManagerBranch {
    pub name: String,
    pub last_message: String,
    pub author: String,
    pub relative_date: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManagerTag {
    pub name: String,
    pub author: String,
    pub relative_date: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ManagerCommit {
    pub message: String,
    pub author: String,
    pub relative_date: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            open_tabs: Vec::new(),
            active_tab: None,
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
            manager_selected_repo: None,
            manager_details: None,
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

    fn open_or_activate(mut self, path: &str) -> Self {
        self.recent_repos.retain(|recent| recent != path);
        self.recent_repos.insert(0, path.to_string());
        if self.recent_repos.len() > 10 {
            self.recent_repos.truncate(10);
        }

        if let Some(index) = self.open_tabs.iter().position(|tab| tab == path) {
            self.active_tab = Some(index);
        } else {
            self.open_tabs.push(path.to_string());
            self.active_tab = Some(self.open_tabs.len().saturating_sub(1));
        }

        self.current_repo = Some(path.to_string());
        self
    }

    fn activate_tab(mut self, index: usize) -> Self {
        if let Some(path) = self.open_tabs.get(index).cloned() {
            self.active_tab = Some(index);
            self.current_repo = Some(path);
        }
        self
    }

    fn close_tab(mut self, index: usize) -> Self {
        if index >= self.open_tabs.len() {
            return self;
        }

        self.open_tabs.remove(index);

        if self.open_tabs.is_empty() {
            self.active_tab = None;
            self.current_repo = None;
            return self.clear_cache();
        }

        let next_index = if Some(index) == self.active_tab {
            index.min(self.open_tabs.len().saturating_sub(1))
        } else {
            match self.active_tab {
                Some(active) if active > index => active - 1,
                Some(active) => active,
                None => 0,
            }
        };

        self.active_tab = Some(next_index);
        self.current_repo = self.open_tabs.get(next_index).cloned();
        self
    }

    fn clear_non_persistent_state(mut self) -> Self {
        self.cached_commits.clear();
        self.cached_branches.clear();
        self.cached_remotes.clear();
        self.cached_tags.clear();
        self.cached_status = None;
        self.last_refresh = None;
        self.repo_error = None;
        self
    }

    pub fn push_recent(mut self, path: &str) -> Self {
        self.recent_repos.retain(|p| p != path);
        self.recent_repos.insert(0, path.to_string());
        if self.recent_repos.len() > 10 {
            self.recent_repos.truncate(10);
        }
        self
    }

    pub fn mark_refreshed(mut self) -> Self {
        self.last_refresh = Some(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        );
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
        use crate::git::models::FileChangeKind;
        let map_file = |f: &crate::git::models::FileStatus| CachedFileStatus {
            path: f.path.clone(),
            old_path: f.old_path.clone(),
            kind: match f.kind {
                FileChangeKind::Added => CachedFileChangeKind::Added,
                FileChangeKind::Modified => CachedFileChangeKind::Modified,
                FileChangeKind::Deleted => CachedFileChangeKind::Deleted,
                FileChangeKind::Renamed => CachedFileChangeKind::Renamed,
                FileChangeKind::TypeChanged => CachedFileChangeKind::TypeChanged,
            },
            staged: f.staged,
            additions: f.additions,
            deletions: f.deletions,
        };
        self.cached_status = Some(CachedRepoStatus {
            branch: status.branch.clone(),
            staged_count: status.staged_count,
            unstaged_count: status.unstaged_count,
            staged_files: status.staged_files.iter().map(&map_file).collect(),
            unstaged_files: status.unstaged_files.iter().map(&map_file).collect(),
            additions: status.additions,
            deletions: status.deletions,
            files_changed: status.files_changed,
        });
        self
    }

    pub fn clear_cache(self) -> Self {
        self.clear_non_persistent_state()
    }
}

#[derive(Clone, Debug)]
pub enum AppAction {
    OpenRepo(String),
    SelectRecent(usize),
    ActivateTab(usize),
    CloseTab(usize),
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
    SelectManagerRepo(Option<String>),
    SetManagerDetails(Option<ManagerRepoDetails>),
    RemoveRecentRepo(String),
}

#[derive(Clone, Debug)]
pub enum CommitAction {
    StageFile(String),
    UnstageFile(String),
    DiscardFile(String),
    StageAll,
    DiscardAll,
}

fn reducer(state: &AppState, action: &AppAction) -> AppState {
    match action {
        AppAction::OpenRepo(path) => state.clone().open_or_activate(path).clear_cache(),
        AppAction::SelectRecent(index) => {
            if let Some(path) = state.recent_repos.get(*index).cloned() {
                state.clone().open_or_activate(path.as_str()).clear_cache()
            } else {
                state.clone()
            }
        }
        AppAction::ActivateTab(index) => state.clone().activate_tab(*index).clear_cache(),
        AppAction::CloseTab(index) => state.clone().close_tab(*index).clear_cache(),
        AppAction::ToggleWindowButtons(show) => AppState {
            open_tabs: state.open_tabs.clone(),
            active_tab: state.active_tab,
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
            manager_selected_repo: state.manager_selected_repo.clone(),
            manager_details: state.manager_details.clone(),
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
            open_tabs: state.open_tabs.clone(),
            active_tab: state.active_tab,
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
            manager_selected_repo: state.manager_selected_repo.clone(),
            manager_details: state.manager_details.clone(),
        },
        AppAction::SelectManagerRepo(path) => AppState {
            open_tabs: state.open_tabs.clone(),
            active_tab: state.active_tab,
            current_repo: state.current_repo.clone(),
            recent_repos: state.recent_repos.clone(),
            show_window_buttons: state.show_window_buttons,
            cached_commits: state.cached_commits.clone(),
            cached_branches: state.cached_branches.clone(),
            cached_remotes: state.cached_remotes.clone(),
            cached_tags: state.cached_tags.clone(),
            cached_status: state.cached_status.clone(),
            last_refresh: state.last_refresh,
            repo_error: state.repo_error.clone(),
            manager_selected_repo: path.clone(),
            manager_details: None,
        },
        AppAction::SetManagerDetails(details) => AppState {
            open_tabs: state.open_tabs.clone(),
            active_tab: state.active_tab,
            current_repo: state.current_repo.clone(),
            recent_repos: state.recent_repos.clone(),
            show_window_buttons: state.show_window_buttons,
            cached_commits: state.cached_commits.clone(),
            cached_branches: state.cached_branches.clone(),
            cached_remotes: state.cached_remotes.clone(),
            cached_tags: state.cached_tags.clone(),
            cached_status: state.cached_status.clone(),
            last_refresh: state.last_refresh,
            repo_error: state.repo_error.clone(),
            manager_selected_repo: state.manager_selected_repo.clone(),
            manager_details: details.clone(),
        },
        AppAction::RemoveRecentRepo(path) => AppState {
            open_tabs: state.open_tabs.clone(),
            active_tab: state.active_tab,
            current_repo: state.current_repo.clone(),
            recent_repos: state
                .recent_repos
                .iter()
                .filter(|r| r.as_str() != path.as_str())
                .cloned()
                .collect(),
            show_window_buttons: state.show_window_buttons,
            cached_commits: state.cached_commits.clone(),
            cached_branches: state.cached_branches.clone(),
            cached_remotes: state.cached_remotes.clone(),
            cached_tags: state.cached_tags.clone(),
            cached_status: state.cached_status.clone(),
            last_refresh: state.last_refresh,
            repo_error: state.repo_error.clone(),
            manager_selected_repo: if state.manager_selected_repo.as_deref() == Some(path) {
                None
            } else {
                state.manager_selected_repo.clone()
            },
            manager_details: if state.manager_selected_repo.as_deref() == Some(path) {
                None
            } else {
                state.manager_details.clone()
            },
        },
    }
}

pub type AppStore = Store<AppState, AppAction>;

pub fn create_store(session: AppSession) -> Arc<AppStore> {
    let initial = session.into_state();
    Arc::new(Store::new(initial, Box::new(create_reducer(reducer))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_state() {
        let state = AppState::default();
        assert!(state.open_tabs.is_empty());
        assert!(state.active_tab.is_none());
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
        assert_eq!(result.open_tabs, vec!["/test".to_string()]);
        assert_eq!(result.active_tab, Some(0));
        assert_eq!(result.recent_repos.len(), 1);
    }

    #[test]
    fn test_open_repo_action_activates_existing_tab() {
        let state = AppState {
            open_tabs: vec!["/a".to_string(), "/b".to_string()],
            active_tab: Some(0),
            current_repo: Some("/a".to_string()),
            ..Default::default()
        };

        let result = reducer(&state, &AppAction::OpenRepo("/b".to_string()));
        assert_eq!(result.open_tabs, vec!["/a".to_string(), "/b".to_string()]);
        assert_eq!(result.active_tab, Some(1));
        assert_eq!(result.current_repo, Some("/b".to_string()));
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
    fn test_close_tab_promotes_next_tab() {
        let state = AppState {
            open_tabs: vec!["/a".to_string(), "/b".to_string(), "/c".to_string()],
            active_tab: Some(1),
            current_repo: Some("/b".to_string()),
            ..Default::default()
        };

        let result = reducer(&state, &AppAction::CloseTab(1));
        assert_eq!(result.open_tabs, vec!["/a".to_string(), "/c".to_string()]);
        assert_eq!(result.active_tab, Some(1));
        assert_eq!(result.current_repo, Some("/c".to_string()));
    }

    #[test]
    fn test_toggle_window_buttons() {
        let state = AppState::default();
        let result = reducer(&state, &AppAction::ToggleWindowButtons(false));
        assert!(!result.show_window_buttons);
    }

    #[test]
    fn test_create_store() {
        let store = create_store(AppSession::default());
        let state = store.get_state();
        assert!(state.current_repo.is_none());
        assert!(state.show_window_buttons);
    }

    #[test]
    fn test_session_round_trip() {
        let session = AppSession {
            version: SESSION_VERSION,
            open_tabs: vec!["/one".to_string(), "/two".to_string()],
            active_tab: Some(1),
            recent_repos: vec!["/two".to_string(), "/one".to_string()],
            show_window_buttons: false,
        };

        let state = session.clone().into_state();
        let restored = AppSession::from_state(&state);

        assert_eq!(restored, session);
        assert_eq!(state.current_repo, Some("/two".to_string()));
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
