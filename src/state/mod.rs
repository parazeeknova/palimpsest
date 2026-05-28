use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use zed::{Store, create_reducer};

use crate::git::models::{Branch, Commit, Remote, RepoStatus, Stash, Tag};
use crate::ui::body::CommitDrawerLayout;

const SESSION_VERSION: u32 = 2;
const SESSION_FILE_NAME: &str = "session.json";
const APP_ID: &str = "Palimpsest";

fn compare_tag_names_by_parsed_version(a: &str, b: &str) -> std::cmp::Ordering {
    use crate::ui::repo_manager::parse_tag_version;
    let va = parse_tag_version(a);
    let vb = parse_tag_version(b);
    match vb.cmp(&va) {
        std::cmp::Ordering::Equal => b.cmp(a),
        other => other,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentRepo {
    pub path: String,
    pub last_opened: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepoSidebarStates {
    pub branches_expanded: bool,
    pub remotes_expanded: bool,
    pub tags_expanded: bool,
    pub stashes_expanded: bool,
    pub prs_expanded: bool,
    pub runs_expanded: bool,
    pub releases_expanded: bool,
    pub packages_expanded: bool,
}

impl Default for RepoSidebarStates {
    fn default() -> Self {
        Self {
            branches_expanded: true,
            remotes_expanded: false,
            tags_expanded: false,
            stashes_expanded: false,
            prs_expanded: false,
            runs_expanded: false,
            releases_expanded: false,
            packages_expanded: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSession {
    pub version: u32,
    pub open_tabs: Vec<String>,
    pub active_tab: Option<usize>,
    pub recent_repos: Vec<RecentRepo>,
    pub show_window_buttons: bool,
    #[serde(default = "default_manager_repo_filter")]
    pub manager_repo_filter: String,
    #[serde(default)]
    pub setup_completed: bool,
    #[serde(default)]
    pub git_user_name: Option<String>,
    #[serde(default)]
    pub git_user_email: Option<String>,
    #[serde(default)]
    pub commit_drawer_layout: CommitDrawerLayout,
    #[serde(default = "default_drawer_height")]
    pub commit_drawer_height: u32,
    #[serde(default)]
    pub repo_sidebar_states: std::collections::HashMap<String, RepoSidebarStates>,
}

impl Default for AppSession {
    fn default() -> Self {
        Self {
            version: SESSION_VERSION,
            open_tabs: Vec::new(),
            active_tab: None,
            recent_repos: Vec::new(),
            show_window_buttons: true,
            manager_repo_filter: default_manager_repo_filter(),
            setup_completed: false,
            git_user_name: None,
            git_user_email: None,
            commit_drawer_layout: CommitDrawerLayout::Horizontal,
            commit_drawer_height: default_drawer_height(),
            repo_sidebar_states: std::collections::HashMap::new(),
        }
    }
}

fn default_drawer_height() -> u32 {
    240
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
            manager_repo_filter: state.manager_repo_filter.clone(),
            setup_completed: state.setup_completed,
            git_user_name: state.git_identity.as_ref().and_then(|i| i.name.clone()),
            git_user_email: state.git_identity.as_ref().and_then(|i| i.email.clone()),
            commit_drawer_layout: state.commit_drawer_layout,
            commit_drawer_height: state.commit_drawer_height as u32,
            repo_sidebar_states: state.repo_sidebar_states.clone(),
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
            cached_stashes: Vec::new(),
            cached_status: None,
            last_refresh: None,
            repo_error: None,
            manager_selected_repo: None,
            manager_details: None,
            manager_details_cache: Vec::new(),
            repo_ownership: Vec::new(),
            manager_repo_filter: session.manager_repo_filter,
            repo_sidebar_states: session.repo_sidebar_states,
            github_user: None,
            git_identity: if session.git_user_name.is_some() || session.git_user_email.is_some() {
                Some(CachedGitIdentity {
                    name: session.git_user_name,
                    email: session.git_user_email,
                    signing_key: None,
                    gpg_sign_commits: false,
                    ssh_key_count: 0,
                    gpg_key_count: 0,
                    ssh_keys: Vec::new(),
                    gpg_keys: Vec::new(),
                })
            } else {
                None
            },
            auth_status: AuthStatus::Unknown,
            setup_completed: session.setup_completed,
            github_pull_requests: Vec::new(),
            github_action_runs: Vec::new(),
            github_releases: Vec::new(),
            github_packages: Vec::new(),
            github_loading: false,
            github_error: None,
            avatar_cache: std::collections::HashMap::new(),
            commit_drawer_layout: session.commit_drawer_layout,
            commit_drawer_height: session.commit_drawer_height as f32,
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

    pub fn save(self) {
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

        let serialized = match serde_json::to_string_pretty(&self.normalize()) {
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
            if error.kind() == std::io::ErrorKind::AlreadyExists {
                let remove_res = fs::remove_file(&path);
                let can_retry = match &remove_res {
                    Ok(()) => true,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
                    Err(_) => false,
                };

                if can_retry {
                    if let Err(retry_error) = fs::rename(&temp_path, &path) {
                        tracing::warn!(
                            from = %temp_path.display(),
                            to = %path.display(),
                            error = %retry_error,
                            "Failed to rename temp file to destination after attempting removal of existing file"
                        );
                    }
                } else if let Err(remove_error) = remove_res {
                    tracing::warn!(
                        path = %path.display(),
                        temp_path = %temp_path.display(),
                        error = %remove_error,
                        "Failed to remove existing file during rename retry"
                    );
                }
            } else {
                tracing::warn!(from = %temp_path.display(), to = %path.display(), error = %error, "Failed to commit session file");
            }
            let _ = fs::remove_file(&temp_path);
        }
    }
}

fn default_manager_repo_filter() -> String {
    "all".to_string()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    pub open_tabs: Vec<String>,
    pub active_tab: Option<usize>,
    pub current_repo: Option<String>,
    pub recent_repos: Vec<RecentRepo>,
    pub show_window_buttons: bool,
    pub cached_commits: Vec<CachedCommit>,
    pub cached_branches: Vec<CachedBranch>,
    pub cached_remotes: Vec<CachedRemote>,
    pub cached_tags: Vec<CachedTag>,
    pub cached_stashes: Vec<CachedStash>,
    pub cached_status: Option<CachedRepoStatus>,
    pub last_refresh: Option<u128>,
    pub repo_error: Option<String>,
    pub manager_selected_repo: Option<String>,
    pub manager_details: Option<ManagerRepoDetails>,
    pub manager_details_cache: Vec<(String, ManagerRepoDetails)>,
    pub repo_ownership: Vec<(String, Option<bool>)>,
    pub manager_repo_filter: String,
    pub github_user: Option<GitHubUserProfile>,
    pub git_identity: Option<CachedGitIdentity>,
    pub auth_status: AuthStatus,
    pub setup_completed: bool,
    pub github_pull_requests: Vec<GitHubPullRequest>,
    pub github_action_runs: Vec<GitHubActionRun>,
    pub github_releases: Vec<GitHubRelease>,
    pub github_packages: Vec<GitHubPackage>,
    pub github_loading: bool,
    pub github_error: Option<String>,
    #[serde(skip)]
    pub avatar_cache: std::collections::HashMap<String, String>,
    pub commit_drawer_layout: CommitDrawerLayout,
    pub commit_drawer_height: f32,
    pub repo_sidebar_states: std::collections::HashMap<String, RepoSidebarStates>,
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
            && self.cached_stashes == other.cached_stashes
            && self.cached_status == other.cached_status
            && self.last_refresh == other.last_refresh
            && self.repo_error == other.repo_error
            && self.manager_selected_repo == other.manager_selected_repo
            && self.manager_details == other.manager_details
            && self.manager_details_cache == other.manager_details_cache
            && self.repo_ownership == other.repo_ownership
            && self.manager_repo_filter == other.manager_repo_filter
            && self.github_user == other.github_user
            && self.git_identity == other.git_identity
            && self.auth_status == other.auth_status
            && self.setup_completed == other.setup_completed
            && self.github_pull_requests == other.github_pull_requests
            && self.github_action_runs == other.github_action_runs
            && self.github_releases == other.github_releases
            && self.github_packages == other.github_packages
            && self.github_loading == other.github_loading
            && self.github_error == other.github_error
            && self.commit_drawer_layout == other.commit_drawer_layout
            && self.commit_drawer_height == other.commit_drawer_height
            && self.repo_sidebar_states == other.repo_sidebar_states
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
    pub ahead: Option<usize>,
    pub behind: Option<usize>,
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
pub struct CachedStash {
    pub message: String,
    pub hash: String,
    pub timestamp_secs: i64,
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
    #[serde(default)]
    pub owned_by_authed_user: Option<bool>,
    #[serde(default)]
    pub is_org: Option<bool>,
    #[serde(default)]
    pub is_private: Option<bool>,
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

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub enum AuthStatus {
    #[default]
    Unknown,
    Disconnected,
    Connecting,
    Connected,
    Failed(String),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GitHubUserProfile {
    pub login: String,
    pub name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: String,
    pub html_url: String,
    pub bio: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GitHubPullRequest {
    pub number: u64,
    pub title: String,
    pub state: String,
    pub user_login: String,
    pub html_url: String,
    pub head_ref: String,
    pub base_ref: String,
    pub draft: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GitHubActionRun {
    pub id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub html_url: String,
    pub head_branch: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub run_number: u32,
    #[serde(default)]
    pub actor_login: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GitHubRelease {
    pub name: Option<String>,
    pub tag_name: String,
    pub body: Option<String>,
    pub html_url: String,
    pub draft: bool,
    pub prerelease: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GitHubPackage {
    pub name: String,
    pub package_type: String,
    pub html_url: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CachedGitIdentity {
    pub name: Option<String>,
    pub email: Option<String>,
    pub signing_key: Option<String>,
    pub gpg_sign_commits: bool,
    pub ssh_key_count: usize,
    pub gpg_key_count: usize,
    #[serde(default)]
    pub ssh_keys: Vec<crate::auth::git_identity::SshKeyInfo>,
    #[serde(default)]
    pub gpg_keys: Vec<crate::auth::git_identity::GpgKeyInfo>,
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
            cached_stashes: Vec::new(),
            cached_status: None,
            last_refresh: None,
            repo_error: None,
            manager_selected_repo: None,
            manager_details: None,
            manager_details_cache: Vec::new(),
            repo_ownership: Vec::new(),
            manager_repo_filter: default_manager_repo_filter(),
            github_user: None,
            git_identity: None,
            auth_status: AuthStatus::Unknown,
            setup_completed: false,
            github_pull_requests: Vec::new(),
            github_action_runs: Vec::new(),
            github_releases: Vec::new(),
            github_packages: Vec::new(),
            github_loading: false,
            github_error: None,
            avatar_cache: std::collections::HashMap::new(),
            commit_drawer_layout: CommitDrawerLayout::Horizontal,
            commit_drawer_height: 240.0,
            repo_sidebar_states: std::collections::HashMap::new(),
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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.recent_repos.retain(|r| r.path != path);
        self.recent_repos.insert(
            0,
            RecentRepo {
                path: path.to_string(),
                last_opened: now,
            },
        );
        if self.recent_repos.len() > 20 {
            self.recent_repos.truncate(20);
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
        self.cached_stashes.clear();
        self.cached_status = None;
        self.last_refresh = None;
        self.repo_error = None;
        self.github_pull_requests.clear();
        self.github_action_runs.clear();
        self.github_releases.clear();
        self.github_packages.clear();
        self.github_loading = false;
        self.github_error = None;
        self.repo_ownership.clear();
        self
    }

    pub fn repo_ownership_for(&self, path: &str) -> Option<bool> {
        self.repo_ownership
            .iter()
            .rev()
            .find(|(p, _)| p == path)
            .and_then(|(_, owned)| *owned)
    }

    pub fn push_recent(mut self, path: &str) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.recent_repos.retain(|r| r.path != path);
        self.recent_repos.insert(
            0,
            RecentRepo {
                path: path.to_string(),
                last_opened: now,
            },
        );
        if self.recent_repos.len() > 20 {
            self.recent_repos.truncate(20);
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
                ahead: b.ahead,
                behind: b.behind,
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
        let mut cached: Vec<CachedTag> = tags
            .iter()
            .map(|t| CachedTag {
                name: t.name.clone(),
                target_hash: t.target_hash.clone(),
            })
            .collect();

        cached.sort_by(|a, b| compare_tag_names_by_parsed_version(&a.name, &b.name));

        self.cached_tags = cached;
        self
    }

    pub fn with_cached_stashes(mut self, stashes: &[Stash]) -> Self {
        self.cached_stashes = stashes
            .iter()
            .map(|s| CachedStash {
                message: s.message.clone(),
                hash: s.hash.clone(),
                timestamp_secs: s
                    .timestamp
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0),
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
        stashes: Vec<Stash>,
        status: RepoStatus,
    },
    ClearGitCache,
    SetRepoError(Option<String>),
    SelectManagerRepo(Option<String>),
    SetManagerDetails(Option<ManagerRepoDetails>),
    SetRepoOwnership {
        path: String,
        owned: Option<bool>,
    },
    SetManagerRepoFilter(String),
    RemoveRecentRepo(String),
    SetGitHubUser(Option<GitHubUserProfile>),
    SetGitIdentity(Option<CachedGitIdentity>),
    SetAuthStatus(AuthStatus),
    SetSetupCompleted(bool),
    SetGitHubData {
        pull_requests: Vec<GitHubPullRequest>,
        action_runs: Vec<GitHubActionRun>,
        releases: Vec<GitHubRelease>,
        packages: Vec<GitHubPackage>,
    },
    SetGitHubLoading(bool),
    SetGitHubError(Option<String>),
    SetAvatarPath {
        key: String,
        path: String,
    },
    SetCommitDrawerLayout(CommitDrawerLayout),
    SetCommitDrawerHeight(f32),
    SetRepoSidebarStates {
        repo_path: String,
        states: RepoSidebarStates,
    },
}

#[derive(Clone, Debug)]
pub enum CommitAction {
    StageFile(String),
    UnstageFile(String),
    DiscardFile(String),
    StageAll,
    DiscardAll,
    Commit { message: String, amend: bool },
    UnstageAll,
}

#[derive(Clone, Debug)]
pub enum StashAction {
    Save(Option<String>),
    Pop(usize),
    Apply(usize),
    Drop(usize),
}

#[derive(Clone, Debug)]
pub enum BranchAction {
    Create(String),
    Checkout(String),
    Delete(String),
    CreateAndCheckout(String),
    CheckoutRemote {
        local_name: String,
        remote_name: String,
    },
}

fn reducer(state: &AppState, action: &AppAction) -> AppState {
    match action {
        AppAction::OpenRepo(path) => state.clone().open_or_activate(path).clear_cache(),
        AppAction::SelectRecent(index) => {
            if let Some(repo) = state.recent_repos.get(*index) {
                state
                    .clone()
                    .open_or_activate(repo.path.as_str())
                    .clear_cache()
            } else {
                state.clone()
            }
        }
        AppAction::ActivateTab(index) => {
            let prev_repo = state.current_repo.clone();
            let next_state = state.clone().activate_tab(*index);
            if next_state.current_repo != prev_repo {
                next_state.clear_cache()
            } else {
                next_state
            }
        }
        AppAction::CloseTab(index) => {
            let prev_repo = state.current_repo.clone();
            let was_active = state.active_tab == Some(*index);
            let next_state = state.clone().close_tab(*index);
            if was_active && next_state.current_repo != prev_repo {
                next_state.clear_cache()
            } else {
                next_state
            }
        }
        AppAction::ToggleWindowButtons(show) => AppState {
            show_window_buttons: *show,
            ..state.clone()
        },
        AppAction::RefreshGitData {
            commits,
            branches,
            remotes,
            tags,
            stashes,
            status,
        } => state
            .clone()
            .with_cached_commits(commits)
            .with_cached_branches(branches)
            .with_cached_remotes(remotes)
            .with_cached_tags(tags)
            .with_cached_stashes(stashes)
            .with_cached_status(status)
            .mark_refreshed(),
        AppAction::ClearGitCache => state.clone().clear_cache(),
        AppAction::SetRepoError(error) => AppState {
            repo_error: error.clone(),
            ..state.clone()
        },
        AppAction::SelectManagerRepo(path) => {
            let cached = path.as_ref().and_then(|p| {
                state
                    .manager_details_cache
                    .iter()
                    .find(|(k, _)| k == p)
                    .map(|(_, v)| v.clone())
            });
            AppState {
                manager_selected_repo: path.clone(),
                manager_details: cached,
                ..state.clone()
            }
        }
        AppAction::SetManagerDetails(details) => {
            let mut cache = state.manager_details_cache.clone();
            let mut new_details = state.manager_details.clone();
            if let Some(d) = details {
                cache.retain(|(k, _)| k != &d.repo_path);
                cache.push((d.repo_path.clone(), d.clone()));
                if cache.len() > 10 {
                    cache.remove(0);
                }
                if state.manager_selected_repo.as_ref() == Some(&d.repo_path) {
                    new_details = Some(d.clone());
                }
            } else {
                new_details = None;
            }
            AppState {
                manager_details: new_details,
                manager_details_cache: cache,
                ..state.clone()
            }
        }
        AppAction::SetRepoOwnership { path, owned } => AppState {
            repo_ownership: {
                let mut ownership = state.repo_ownership.clone();
                ownership.push((path.clone(), *owned));
                ownership
            },
            ..state.clone()
        },
        AppAction::SetManagerRepoFilter(filter) => AppState {
            manager_repo_filter: filter.clone(),
            ..state.clone()
        },
        AppAction::RemoveRecentRepo(path) => AppState {
            recent_repos: state
                .recent_repos
                .iter()
                .filter(|r| r.path != *path)
                .cloned()
                .collect(),
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
            manager_details_cache: state
                .manager_details_cache
                .clone()
                .into_iter()
                .filter(|(k, _)| k != path)
                .collect(),
            repo_ownership: state
                .repo_ownership
                .clone()
                .into_iter()
                .filter(|(k, _)| k != path)
                .collect(),
            repo_sidebar_states: state
                .repo_sidebar_states
                .clone()
                .into_iter()
                .filter(|(k, _)| k != path)
                .collect(),
            ..state.clone()
        },
        AppAction::SetGitHubUser(user) => AppState {
            github_user: user.clone(),
            ..state.clone()
        },
        AppAction::SetGitIdentity(identity) => AppState {
            git_identity: identity.clone(),
            ..state.clone()
        },
        AppAction::SetAuthStatus(status) => AppState {
            auth_status: status.clone(),
            ..state.clone()
        },
        AppAction::SetSetupCompleted(completed) => AppState {
            setup_completed: *completed,
            ..state.clone()
        },
        AppAction::SetGitHubData {
            pull_requests,
            action_runs,
            releases,
            packages,
        } => {
            let mut sorted_releases = releases.clone();
            sorted_releases
                .sort_by(|a, b| compare_tag_names_by_parsed_version(&a.tag_name, &b.tag_name));

            AppState {
                github_pull_requests: pull_requests.clone(),
                github_action_runs: action_runs.clone(),
                github_releases: sorted_releases,
                github_packages: packages.clone(),
                github_loading: false,
                github_error: None,
                ..state.clone()
            }
        }
        AppAction::SetGitHubLoading(loading) => AppState {
            github_loading: *loading,
            ..state.clone()
        },
        AppAction::SetGitHubError(error) => AppState {
            github_error: error.clone(),
            github_loading: false,
            ..state.clone()
        },
        AppAction::SetAvatarPath { key, path } => {
            let mut avatar_cache = state.avatar_cache.clone();
            avatar_cache.insert(key.clone(), path.clone());
            AppState {
                avatar_cache,
                ..state.clone()
            }
        }
        AppAction::SetCommitDrawerLayout(layout) => AppState {
            commit_drawer_layout: *layout,
            ..state.clone()
        },
        AppAction::SetCommitDrawerHeight(height) => AppState {
            commit_drawer_height: *height,
            ..state.clone()
        },
        AppAction::SetRepoSidebarStates { repo_path, states } => {
            tracing::debug!("Sidebar section states updated");
            let mut repo_sidebar_states = state.repo_sidebar_states.clone();
            repo_sidebar_states.insert(repo_path.clone(), states.clone());
            AppState {
                repo_sidebar_states,
                ..state.clone()
            }
        }
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
    fn test_set_avatar_path() {
        let state = AppState::default();
        assert!(state.avatar_cache.is_empty());

        let action = AppAction::SetAvatarPath {
            key: "John Doe".to_string(),
            path: "/path/to/avatar.png".to_string(),
        };
        let result = reducer(&state, &action);
        assert_eq!(
            result.avatar_cache.get("John Doe"),
            Some(&"/path/to/avatar.png".to_string())
        );
    }

    #[test]
    fn test_push_recent_adds_to_empty() {
        let state = AppState::default();
        let state = state.push_recent("/path/to/repo");
        assert_eq!(state.recent_repos.len(), 1);
        assert_eq!(state.recent_repos[0].path, "/path/to/repo");
    }

    #[test]
    fn test_push_recent_moves_existing_to_front() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let state = AppState {
            recent_repos: vec![
                RecentRepo {
                    path: "/a".to_string(),
                    last_opened: now,
                },
                RecentRepo {
                    path: "/b".to_string(),
                    last_opened: now,
                },
            ],
            ..Default::default()
        };
        let state = state.push_recent("/b");
        assert_eq!(state.recent_repos[0].path, "/b");
        assert_eq!(state.recent_repos.len(), 2);
    }

    #[test]
    fn test_push_recent_truncates_at_20() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut state = AppState {
            recent_repos: (0..20)
                .map(|i| RecentRepo {
                    path: format!("/repo{}", i),
                    last_opened: now,
                })
                .collect(),
            ..Default::default()
        };
        state = state.push_recent("/new-repo");
        assert_eq!(state.recent_repos.len(), 20);
        assert_eq!(state.recent_repos[0].path, "/new-repo");
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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let state = AppState {
            recent_repos: vec![
                RecentRepo {
                    path: "/a".to_string(),
                    last_opened: now,
                },
                RecentRepo {
                    path: "/b".to_string(),
                    last_opened: now,
                },
            ],
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
    fn test_setup_and_auth_reducer_actions() {
        let state = AppState::default();

        let state = reducer(&state, &AppAction::SetSetupCompleted(true));
        assert!(state.setup_completed);

        let state = reducer(&state, &AppAction::SetAuthStatus(AuthStatus::Connected));
        assert_eq!(state.auth_status, AuthStatus::Connected);

        let user = GitHubUserProfile {
            login: "testuser".to_string(),
            name: Some("Test Name".to_string()),
            email: Some("test@example.com".to_string()),
            avatar_url: "https://example.com/avatar".to_string(),
            html_url: "https://github.com/testuser".to_string(),
            bio: None,
        };
        let state = reducer(&state, &AppAction::SetGitHubUser(Some(user.clone())));
        assert_eq!(state.github_user, Some(user));

        let identity = CachedGitIdentity {
            name: Some("Git User".to_string()),
            email: Some("git@example.com".to_string()),
            signing_key: None,
            gpg_sign_commits: false,
            ssh_key_count: 2,
            gpg_key_count: 1,
            ssh_keys: Vec::new(),
            gpg_keys: Vec::new(),
        };
        let state = reducer(&state, &AppAction::SetGitIdentity(Some(identity.clone())));
        assert_eq!(state.git_identity, Some(identity));
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
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let session = AppSession {
            version: SESSION_VERSION,
            open_tabs: vec!["/one".to_string(), "/two".to_string()],
            active_tab: Some(1),
            recent_repos: vec![
                RecentRepo {
                    path: "/two".to_string(),
                    last_opened: now,
                },
                RecentRepo {
                    path: "/one".to_string(),
                    last_opened: now,
                },
            ],
            show_window_buttons: false,
            manager_repo_filter: "owned".to_string(),
            setup_completed: false,
            git_user_name: None,
            git_user_email: None,
            commit_drawer_layout: CommitDrawerLayout::Horizontal,
            commit_drawer_height: 240,
            repo_sidebar_states: std::collections::HashMap::new(),
        };

        let state = session.clone().into_state();
        let restored = AppSession::from_state(&state);

        assert_eq!(restored, session);
        assert_eq!(state.current_repo, Some("/two".to_string()));
    }

    #[test]
    fn test_app_session_default_manager_repo_filter() {
        assert_eq!(AppSession::default().manager_repo_filter, "all");
    }

    #[test]
    fn test_set_repo_ownership_updates_lookup_true() {
        let state = AppState::default();
        let result = reducer(
            &state,
            &AppAction::SetRepoOwnership {
                path: "/repo1".to_string(),
                owned: Some(true),
            },
        );
        assert_eq!(result.repo_ownership_for("/repo1"), Some(true));
    }

    #[test]
    fn test_app_session_from_state_uses_default_manager_repo_filter() {
        let state = AppState::default();
        let session = AppSession::from_state(&state);
        assert_eq!(session.manager_repo_filter, "all");
    }

    #[test]
    fn test_set_manager_repo_filter_updates_state() {
        let state = AppState::default();
        let result = reducer(
            &state,
            &AppAction::SetManagerRepoFilter("owned".to_string()),
        );
        assert_eq!(result.manager_repo_filter, "owned");
    }

    #[test]
    fn test_manager_repo_details_serializes_ownership() {
        let details = ManagerRepoDetails {
            repo_path: "/repo".to_string(),
            repo_name: "repo".to_string(),
            branch: "main".to_string(),
            uncommitted_files: 0,
            total_commits: 1,
            initial_commit_date: "just now".to_string(),
            last_commit_date: "just now".to_string(),
            remotes: vec![],
            branches: vec![],
            tags: vec![],
            commits: vec![],
            owned_by_authed_user: Some(true),
            is_org: None,
            is_private: None,
        };
        let json = serde_json::to_string(&details).unwrap();
        let round_trip: ManagerRepoDetails = serde_json::from_str(&json).unwrap();
        assert_eq!(round_trip.owned_by_authed_user, Some(true));
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

    #[test]
    fn test_select_manager_repo_sets_selection() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let state = AppState {
            recent_repos: vec![
                RecentRepo {
                    path: "/repo1".to_string(),
                    last_opened: now,
                },
                RecentRepo {
                    path: "/repo2".to_string(),
                    last_opened: now,
                },
            ],
            ..Default::default()
        };
        let result = reducer(
            &state,
            &AppAction::SelectManagerRepo(Some("/repo1".to_string())),
        );
        assert_eq!(result.manager_selected_repo, Some("/repo1".to_string()));
        assert!(result.manager_details.is_none());
    }

    #[test]
    fn test_select_manager_repo_none_clears_selection() {
        let details = ManagerRepoDetails {
            repo_path: "/repo1".to_string(),
            repo_name: "repo1".to_string(),
            branch: "main".to_string(),
            uncommitted_files: 0,
            total_commits: 10,
            initial_commit_date: "1 year ago".to_string(),
            last_commit_date: "1 day ago".to_string(),
            remotes: vec![],
            branches: vec![],
            tags: vec![],
            commits: vec![],
            owned_by_authed_user: Some(true),
            is_org: None,
            is_private: None,
        };
        let state = AppState {
            manager_selected_repo: Some("/repo1".to_string()),
            manager_details: Some(details),
            ..Default::default()
        };
        let result = reducer(&state, &AppAction::SelectManagerRepo(None));
        assert!(result.manager_selected_repo.is_none());
        assert!(result.manager_details.is_none());
    }

    #[test]
    fn test_set_manager_details_populates_details() {
        let details = ManagerRepoDetails {
            repo_path: "/repo1".to_string(),
            repo_name: "repo1".to_string(),
            branch: "main".to_string(),
            uncommitted_files: 3,
            total_commits: 42,
            initial_commit_date: "6 months ago".to_string(),
            last_commit_date: "2 hours ago".to_string(),
            remotes: vec![ManagerRemote {
                name: "origin".to_string(),
                url: "https://github.com/user/repo".to_string(),
                is_github: true,
            }],
            branches: vec![],
            tags: vec![],
            commits: vec![],
            owned_by_authed_user: Some(true),
            is_org: None,
            is_private: None,
        };
        let state = AppState {
            manager_selected_repo: Some("/repo1".to_string()),
            ..Default::default()
        };
        let result = reducer(&state, &AppAction::SetManagerDetails(Some(details.clone())));
        assert_eq!(result.manager_details, Some(details));
    }

    #[test]
    fn test_set_repo_ownership_updates_lookup_false() {
        let state = AppState::default();
        let result = reducer(
            &state,
            &AppAction::SetRepoOwnership {
                path: "/repo1".to_string(),
                owned: Some(false),
            },
        );
        assert_eq!(result.repo_ownership_for("/repo1"), Some(false));
    }

    #[test]
    fn test_remove_recent_repo_removes_from_list() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let state = AppState {
            recent_repos: vec![
                RecentRepo {
                    path: "/repo1".to_string(),
                    last_opened: now,
                },
                RecentRepo {
                    path: "/repo2".to_string(),
                    last_opened: now,
                },
                RecentRepo {
                    path: "/repo3".to_string(),
                    last_opened: now,
                },
            ],
            manager_selected_repo: Some("/repo2".to_string()),
            ..Default::default()
        };
        let result = reducer(&state, &AppAction::RemoveRecentRepo("/repo2".to_string()));
        assert_eq!(
            result
                .recent_repos
                .iter()
                .map(|r| r.path.clone())
                .collect::<Vec<_>>(),
            vec!["/repo1".to_string(), "/repo3".to_string()]
        );
        assert!(result.manager_selected_repo.is_none());
        assert!(result.manager_details.is_none());
    }

    #[test]
    fn test_remove_recent_repo_preserves_other_selection() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let details = ManagerRepoDetails {
            repo_path: "/repo1".to_string(),
            repo_name: "repo1".to_string(),
            branch: "main".to_string(),
            uncommitted_files: 0,
            total_commits: 10,
            initial_commit_date: "1 year ago".to_string(),
            last_commit_date: "1 day ago".to_string(),
            remotes: vec![],
            branches: vec![],
            tags: vec![],
            commits: vec![],
            owned_by_authed_user: Some(true),
            is_org: None,
            is_private: None,
        };
        let state = AppState {
            recent_repos: vec![
                RecentRepo {
                    path: "/repo1".to_string(),
                    last_opened: now,
                },
                RecentRepo {
                    path: "/repo2".to_string(),
                    last_opened: now,
                },
            ],
            manager_selected_repo: Some("/repo1".to_string()),
            manager_details: Some(details.clone()),
            ..Default::default()
        };
        let result = reducer(&state, &AppAction::RemoveRecentRepo("/repo2".to_string()));
        assert_eq!(result.recent_repos.len(), 1);
        assert_eq!(result.recent_repos[0].path, "/repo1");
        assert_eq!(result.manager_selected_repo, Some("/repo1".to_string()));
        assert_eq!(result.manager_details, Some(details));
    }

    #[test]
    fn test_manager_details_serialization_round_trip() {
        let details = ManagerRepoDetails {
            repo_path: "/home/user/my-project".to_string(),
            repo_name: "my-project".to_string(),
            branch: "develop".to_string(),
            uncommitted_files: 5,
            total_commits: 128,
            initial_commit_date: "2 years ago".to_string(),
            last_commit_date: "3 minutes ago".to_string(),
            remotes: vec![
                ManagerRemote {
                    name: "origin".to_string(),
                    url: "https://github.com/user/my-project".to_string(),
                    is_github: true,
                },
                ManagerRemote {
                    name: "upstream".to_string(),
                    url: "https://gitlab.com/org/my-project".to_string(),
                    is_github: false,
                },
            ],
            branches: vec![ManagerBranch {
                name: "main".to_string(),
                last_message: "fix: resolve merge conflict".to_string(),
                author: "Alice".to_string(),
                relative_date: "1 hour ago".to_string(),
            }],
            tags: vec![ManagerTag {
                name: "v1.0.0".to_string(),
                author: "Bob".to_string(),
                relative_date: "1 week ago".to_string(),
            }],
            commits: vec![ManagerCommit {
                message: "feat: add new feature".to_string(),
                author: "Charlie".to_string(),
                relative_date: "just now".to_string(),
            }],
            owned_by_authed_user: Some(false),
            is_org: None,
            is_private: None,
        };

        let serialized = serde_json::to_string(&details).unwrap();
        let deserialized: ManagerRepoDetails = serde_json::from_str(&serialized).unwrap();
        assert_eq!(details, deserialized);
    }
}
