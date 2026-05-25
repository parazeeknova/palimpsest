use eframe::egui;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};

use palimpsest::git::GitRepo;
use palimpsest::git::live::{RepoLiveEvent, RepoLocalSnapshot, RepoOwnership, RepoRemoteSnapshot};
use palimpsest::logger::LogBuffer;
use palimpsest::state::{AppAction, AppStore, BranchAction, CommitAction, StashAction};
use palimpsest::ui::command_palette::{PaletteResult, QuickLaunchAction};
use palimpsest::ui::repo_manager::RepoOwnershipFilterLabel;
use palimpsest::ui::repo_manager_sidebar;
use palimpsest::ui::{
    body, command_palette, commit_panel, profile_panel, repo_manager_body, setup_wizard, sidebar,
    tabbar, titlebar, toolbar,
};

fn primary_modifiers() -> egui::Modifiers {
    if cfg!(target_os = "macos") {
        egui::Modifiers::COMMAND
    } else {
        egui::Modifiers::CTRL
    }
}

fn main() -> eframe::Result {
    let log_buffer = Arc::new(LogBuffer::new(1000));
    palimpsest::logger::init(log_buffer.clone());

    tracing::info!("Palimpsest starting up");

    let creds = palimpsest::auth::credentials::load_credentials();
    let size = if creds.setup_completed {
        [960.0, 720.0]
    } else {
        [500.0, 450.0]
    };

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Palimpsest")
            .with_inner_size(size),
        ..Default::default()
    };

    eframe::run_native(
        "Palimpsest",
        native_options,
        Box::new(|cc| Ok(Box::new(PalimpsestApp::new(cc, log_buffer)))),
    )
}

struct PalimpsestApp {
    store: Arc<AppStore>,
    log_buffer: Arc<LogBuffer>,
    titlebar_menu_open: bool,
    search_query: String,
    debug_open: bool,
    show_command_palette: bool,
    git_repo: Option<GitRepo>,
    body_state: body::State,
    commit_panel_state: commit_panel::State,
    command_palette_state: command_palette::State,
    manager_body_state: repo_manager_body::State,
    manager_sidebar_state: repo_manager_sidebar::SidebarState,
    sidebar_state: sidebar::SidebarState,
    show_create_branch_dialog: bool,
    new_branch_name: String,
    profile_panel_state: profile_panel::ProfilePanelState,
    setup_wizard_state: setup_wizard::SetupWizardState,
    last_fetched_repo: Option<String>,
    repo_live_generation: u64,
    repo_live_states: HashMap<String, RepoLiveState>,
    repo_live_trackers: HashMap<String, RepoTrackerHandle>,
    repo_live_tx: Sender<RepoLiveEvent>,
    repo_live_rx: Receiver<RepoLiveEvent>,
    egui_ctx: egui::Context,
    authed_github_login: Option<String>,
    current_repo_owned_by_authed_user: Option<bool>,
    manager_repo_filter: repo_manager_sidebar::RepoOwnershipFilter,
    repo_metadata_fetches: std::collections::HashSet<String>,
    avatar_fetches: std::collections::HashSet<String>,
    show_clone_dialog: bool,
    clone_url: String,
    clone_dir: String,
    pending_open_repo: Arc<std::sync::Mutex<Option<String>>>,
    show_setup_wizard_dialog: bool,
    show_preferences_dialog: bool,
    show_update_dialog: bool,
    show_create_tag_dialog: bool,
    new_tag_name: String,
    show_save_stash_dialog: bool,
    new_stash_message: String,
}

struct RepoLiveState {
    generation: u64,
    local: Option<RepoLocalSnapshot>,
    remote: Option<RepoRemoteSnapshot>,
}

struct RepoTrackerHandle {
    stop: Arc<AtomicBool>,
}

impl PalimpsestApp {
    fn new(cc: &eframe::CreationContext<'_>, log_buffer: Arc<LogBuffer>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let session = palimpsest::state::AppSession::load();
        let store = palimpsest::state::create_store(session.clone());

        // Load credentials from secure storage and populate the store
        let creds = palimpsest::auth::credentials::load_credentials();
        store.dispatch(AppAction::SetSetupCompleted(creds.setup_completed));
        if !creds.setup_completed {
            cc.egui_ctx
                .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(500.0, 450.0)));
        }
        if let Some(user) = creds.github_user.clone() {
            store.dispatch(AppAction::SetGitHubUser(Some(
                palimpsest::state::GitHubUserProfile {
                    login: user.login,
                    name: user.name,
                    email: user.email,
                    avatar_url: user.avatar_url,
                    html_url: user.html_url,
                    bio: user.bio,
                },
            )));
            store.dispatch(AppAction::SetAuthStatus(
                palimpsest::state::AuthStatus::Connected,
            ));
        } else {
            store.dispatch(AppAction::SetAuthStatus(
                palimpsest::state::AuthStatus::Disconnected,
            ));
        }
        let git_id = if creds.git_name.is_some() || creds.git_email.is_some() {
            Some(palimpsest::state::CachedGitIdentity {
                name: creds.git_name.clone(),
                email: creds.git_email.clone(),
                signing_key: None,
                gpg_sign_commits: false,
                ssh_key_count: 0,
                gpg_key_count: 0,
            })
        } else {
            None
        };
        store.dispatch(AppAction::SetGitIdentity(git_id));

        tracing::info!("Application initialized");

        let (repo_live_tx, repo_live_rx) = mpsc::channel();

        let mut app = Self {
            store,
            log_buffer,
            titlebar_menu_open: false,
            search_query: String::new(),
            debug_open: false,
            show_command_palette: false,
            git_repo: None,
            body_state: body::State::default(),
            commit_panel_state: commit_panel::State::default(),
            command_palette_state: command_palette::State::default(),
            manager_body_state: repo_manager_body::State::default(),
            manager_sidebar_state: repo_manager_sidebar::SidebarState::default(),
            sidebar_state: sidebar::SidebarState::default(),
            show_create_branch_dialog: false,
            new_branch_name: String::new(),
            profile_panel_state: profile_panel::ProfilePanelState::default(),
            setup_wizard_state: setup_wizard::SetupWizardState::default(),
            last_fetched_repo: None,
            repo_live_generation: 0,
            repo_live_states: HashMap::new(),
            repo_live_trackers: HashMap::new(),
            repo_live_tx,
            repo_live_rx,
            egui_ctx: cc.egui_ctx.clone(),
            authed_github_login: creds.github_user.clone().map(|u| u.login),
            current_repo_owned_by_authed_user: None,
            manager_repo_filter: match session.manager_repo_filter.as_str() {
                "owned" => repo_manager_sidebar::RepoOwnershipFilter::Owned,
                "external" => repo_manager_sidebar::RepoOwnershipFilter::External,
                _ => repo_manager_sidebar::RepoOwnershipFilter::All,
            },
            repo_metadata_fetches: std::collections::HashSet::new(),
            avatar_fetches: std::collections::HashSet::new(),
            show_clone_dialog: false,
            clone_url: String::new(),
            clone_dir: String::new(),
            pending_open_repo: Arc::new(std::sync::Mutex::new(None)),
            show_setup_wizard_dialog: false,
            show_preferences_dialog: false,
            show_update_dialog: false,
            show_create_tag_dialog: false,
            new_tag_name: String::new(),
            show_save_stash_dialog: false,
            new_stash_message: String::new(),
        };

        app.restore_active_repo_from_state();
        app.persist_session();
        app
    }

    fn repo_name(&self) -> Option<String> {
        self.git_repo
            .as_ref()
            .and_then(|r| r.repo_name())
            .or_else(|| {
                self.store.get_state().current_repo.as_deref().map(|p| {
                    std::path::Path::new(p)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or(p)
                        .to_string()
                })
            })
    }

    fn persist_session(&self) {
        let mut session = palimpsest::state::AppSession::from_state(&self.store.get_state());
        session.manager_repo_filter = match self.manager_repo_filter {
            repo_manager_sidebar::RepoOwnershipFilter::All => "all".to_string(),
            repo_manager_sidebar::RepoOwnershipFilter::Owned => "owned".to_string(),
            repo_manager_sidebar::RepoOwnershipFilter::External => "external".to_string(),
        };
        session.save();
    }

    fn next_repo_live_generation(&mut self) -> u64 {
        self.repo_live_generation = self.repo_live_generation.saturating_add(1);
        self.repo_live_generation
    }

    fn ensure_repo_tracker(&mut self, path: &str, ctx: &egui::Context) {
        if self.repo_live_trackers.contains_key(path) {
            return;
        }

        let generation = self.next_repo_live_generation();
        let stop = Arc::new(AtomicBool::new(false));
        self.repo_live_states
            .entry(path.to_string())
            .and_modify(|entry| {
                entry.generation = generation;
            })
            .or_insert(RepoLiveState {
                generation,
                local: None,
                remote: None,
            });

        palimpsest::git::live::spawn_repo_tracker(
            path.to_string(),
            generation,
            self.repo_live_tx.clone(),
            stop.clone(),
            ctx.clone(),
            self.authed_github_login.clone(),
        );

        self.repo_live_trackers
            .insert(path.to_string(), RepoTrackerHandle { stop });
    }

    fn ensure_ownership_probes(&mut self) {
        let state = self.store.get_state();
        let github_login = self.authed_github_login.clone();

        for repo in &state.recent_repos {
            if state.repo_ownership_for(&repo.path).is_some() {
                continue;
            }

            if self.repo_live_states.contains_key(&repo.path) {
                continue;
            }

            let generation = self.next_repo_live_generation();
            let stop = Arc::new(AtomicBool::new(false));
            self.repo_live_states.insert(
                repo.path.clone(),
                RepoLiveState {
                    generation,
                    local: None,
                    remote: None,
                },
            );

            palimpsest::git::live::spawn_repo_ownership_probe(
                repo.path.clone(),
                generation,
                self.repo_live_tx.clone(),
                stop,
                github_login.clone(),
            );
        }
    }

    fn stop_repo_tracker(&mut self, path: &str) {
        if let Some(handle) = self.repo_live_trackers.remove(path) {
            handle.stop.store(true, Ordering::Relaxed);
        }
    }

    fn process_repo_live_updates(&mut self, ctx: &egui::Context) {
        let mut changed = false;

        while let Ok(event) = self.repo_live_rx.try_recv() {
            match event {
                RepoLiveEvent::Local {
                    path,
                    generation,
                    snapshot,
                } => {
                    let Some(entry) = self.repo_live_states.get_mut(&path) else {
                        continue;
                    };
                    if entry.generation != generation {
                        continue;
                    }
                    entry.local = Some(snapshot.clone());
                    if self.store.get_state().current_repo.as_deref() == Some(&path) {
                        self.store.dispatch(AppAction::RefreshGitData {
                            commits: snapshot.commits.clone(),
                            branches: snapshot.branches.clone(),
                            remotes: snapshot.remotes.clone(),
                            tags: snapshot.tags.clone(),
                            stashes: snapshot.stashes.clone(),
                            status: snapshot.status.clone(),
                        });
                        self.store
                            .dispatch(AppAction::SetRepoError(snapshot.repo_error.clone()));
                        changed = true;
                    }
                }
                RepoLiveEvent::Remote {
                    path,
                    generation,
                    snapshot,
                } => {
                    let Some(entry) = self.repo_live_states.get_mut(&path) else {
                        continue;
                    };
                    if entry.generation != generation {
                        continue;
                    }
                    entry.remote = Some(snapshot.clone());
                    if self.store.get_state().current_repo.as_deref() == Some(&path) {
                        self.current_repo_owned_by_authed_user =
                            Some(matches!(snapshot.ownership, RepoOwnership::Owned));
                        if snapshot.ownership == RepoOwnership::External {
                            self.store.dispatch(AppAction::SetGitHubData {
                                pull_requests: Vec::new(),
                                action_runs: Vec::new(),
                                releases: Vec::new(),
                                packages: Vec::new(),
                            });
                            self.store.dispatch(AppAction::SetGitHubError(Some(
                                "GitHub repo is not owned by the authenticated user".to_string(),
                            )));
                            changed = true;
                            continue;
                        }
                        self.store.dispatch(AppAction::SetGitHubData {
                            pull_requests: snapshot.pull_requests.clone(),
                            action_runs: snapshot.action_runs.clone(),
                            releases: snapshot.releases.clone(),
                            packages: snapshot.packages.clone(),
                        });
                        self.store
                            .dispatch(AppAction::SetGitHubError(snapshot.github_error.clone()));
                        changed = true;
                    }
                }
                RepoLiveEvent::Ownership {
                    path,
                    generation,
                    ownership,
                } => {
                    let Some(entry) = self.repo_live_states.get_mut(&path) else {
                        continue;
                    };
                    if entry.generation != generation {
                        continue;
                    }
                    self.store.dispatch(AppAction::SetRepoOwnership {
                        path: path.clone(),
                        owned: ownership,
                    });
                    if self.store.get_state().current_repo.as_deref() == Some(&path) {
                        self.current_repo_owned_by_authed_user = ownership;
                        changed = true;
                    }
                }
            }
        }

        if changed {
            ctx.request_repaint();
        }
    }

    fn sync_active_repo_from_cache(&mut self, path: &str) {
        let Some(entry) = self.repo_live_states.get(path) else {
            return;
        };

        if let Some(local) = &entry.local {
            self.store.dispatch(AppAction::RefreshGitData {
                commits: local.commits.clone(),
                branches: local.branches.clone(),
                remotes: local.remotes.clone(),
                tags: local.tags.clone(),
                stashes: local.stashes.clone(),
                status: local.status.clone(),
            });
            self.store
                .dispatch(AppAction::SetRepoError(local.repo_error.clone()));
        }

        if let Some(remote) = &entry.remote {
            if remote.ownership != RepoOwnership::External {
                self.current_repo_owned_by_authed_user =
                    Some(matches!(remote.ownership, RepoOwnership::Owned));
                self.store.dispatch(AppAction::SetGitHubData {
                    pull_requests: remote.pull_requests.clone(),
                    action_runs: remote.action_runs.clone(),
                    releases: remote.releases.clone(),
                    packages: remote.packages.clone(),
                });
                self.store
                    .dispatch(AppAction::SetGitHubError(remote.github_error.clone()));
            }
        }
    }

    fn restore_active_repo_from_state(&mut self) {
        let state = self.store.get_state();
        let ctx = self.egui_ctx.clone();
        for path in state.open_tabs.iter() {
            self.ensure_repo_tracker(path, &ctx);
        }
        if let Some(path) = state.current_repo.clone() {
            if self.git_repo.is_none() {
                self.open_repo(&path);
                if self.git_repo.is_none() {
                    if let Some(index) = state.open_tabs.iter().position(|p| p == &path) {
                        self.store.dispatch(AppAction::CloseTab(index));
                    }
                    self.store
                        .dispatch(AppAction::RemoveRecentRepo(path.clone()));

                    let next_state = self.store.get_state();
                    if let Some(fallback_path) = next_state.current_repo.clone() {
                        self.open_repo(&fallback_path);
                    } else if !next_state.recent_repos.is_empty() {
                        let first_recent = next_state.recent_repos[0].path.clone();
                        self.store
                            .dispatch(AppAction::SelectManagerRepo(Some(first_recent.clone())));
                        self.fetch_manager_details(&first_recent);
                    } else {
                        self.store.dispatch(AppAction::SelectManagerRepo(None));
                    }
                    self.persist_session();
                }
            }
        } else if !state.recent_repos.is_empty() {
            let first_recent = state.recent_repos[0].path.clone();
            self.store
                .dispatch(AppAction::SelectManagerRepo(Some(first_recent.clone())));
            self.fetch_manager_details(&first_recent);
        }
    }

    fn open_repo(&mut self, path: &str) {
        tracing::info!(repo = %path, "Opening repository");
        match GitRepo::open(path) {
            Ok(repo) => {
                tracing::info!(repo_name = ?repo.repo_name(), "Repository opened successfully");
                self.git_repo = Some(repo);
                let ctx = self.egui_ctx.clone();
                self.ensure_repo_tracker(path, &ctx);
                self.store.dispatch(AppAction::OpenRepo(path.to_string()));
                self.refresh_git_data();
                self.persist_session();
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to open repository");
                self.store
                    .dispatch(AppAction::SetRepoError(Some(e.to_string())));

                let state = self.store.get_state();
                if let Some(index) = state.open_tabs.iter().position(|p| p == path) {
                    self.store.dispatch(AppAction::CloseTab(index));
                }
                self.store
                    .dispatch(AppAction::RemoveRecentRepo(path.to_string()));

                let next_state = self.store.get_state();
                if let Some(next_path) = next_state.current_repo.clone() {
                    if next_path != path {
                        self.open_repo(&next_path);
                    }
                } else if !next_state.recent_repos.is_empty() {
                    let first_recent = next_state.recent_repos[0].path.clone();
                    self.store
                        .dispatch(AppAction::SelectManagerRepo(Some(first_recent.clone())));
                    self.fetch_manager_details(&first_recent);
                } else {
                    self.store.dispatch(AppAction::SelectManagerRepo(None));
                }
                self.persist_session();
            }
        }
    }

    fn activate_tab(&mut self, index: usize) {
        self.store.dispatch(AppAction::ActivateTab(index));
        if let Some(path) = self.store.get_state().current_repo.clone() {
            self.git_repo = None;
            self.open_repo(&path);
            self.sync_active_repo_from_cache(&path);
            self.persist_session();
        }
    }

    fn close_tab(&mut self, index: usize) {
        let closed_path = self.store.get_state().open_tabs.get(index).cloned();
        self.store.dispatch(AppAction::CloseTab(index));
        if let Some(path) = closed_path {
            self.stop_repo_tracker(&path);
        }
        self.git_repo = None;
        if let Some(path) = self.store.get_state().current_repo.clone() {
            self.open_repo(&path);
        } else {
            self.persist_session();
        }
    }

    fn refresh_git_data(&mut self) {
        if let Some(repo) = &self.git_repo {
            let snapshot = palimpsest::git::live::collect_local_snapshot(
                repo,
                self.authed_github_login.as_deref(),
            );
            self.store.dispatch(AppAction::RefreshGitData {
                commits: snapshot.commits.clone(),
                branches: snapshot.branches.clone(),
                remotes: snapshot.remotes.clone(),
                tags: snapshot.tags.clone(),
                stashes: snapshot.stashes.clone(),
                status: snapshot.status.clone(),
            });
            self.store
                .dispatch(AppAction::SetRepoError(snapshot.repo_error.clone()));
        }
    }

    fn handle_commit_action(&mut self, action: CommitAction) {
        let Some(repo) = &self.git_repo else {
            return;
        };

        match action {
            CommitAction::StageFile(ref path) => match repo.stage_file(path) {
                Ok(()) => self.refresh_git_data(),
                Err(e) => tracing::error!(path = %path, error = %e, "Failed to stage file"),
            },
            CommitAction::UnstageFile(ref path) => match repo.unstage_file(path) {
                Ok(()) => self.refresh_git_data(),
                Err(e) => tracing::error!(path = %path, error = %e, "Failed to unstage file"),
            },
            CommitAction::DiscardFile(ref path) => match repo.discard_file(path) {
                Ok(()) => self.refresh_git_data(),
                Err(e) => tracing::error!(path = %path, error = %e, "Failed to discard file"),
            },
            CommitAction::StageAll => match repo.stage_all() {
                Ok(()) => self.refresh_git_data(),
                Err(e) => tracing::error!(error = %e, "Failed to stage all files"),
            },
            CommitAction::DiscardAll => match repo.discard_all() {
                Ok(()) => self.refresh_git_data(),
                Err(e) => tracing::error!(error = %e, "Failed to discard all changes"),
            },
            CommitAction::Commit { message, amend } => {
                let message_to_commit = if self.commit_panel_state.sign_off {
                    if let Ok(sig) = repo.signature() {
                        let name = sig.name().unwrap_or("");
                        let email = sig.email().unwrap_or("");
                        format!("{}\n\nSigned-off-by: {} <{}>", message, name, email)
                    } else {
                        message
                    }
                } else {
                    message
                };

                match repo.commit(&message_to_commit, amend) {
                    Ok(()) => {
                        self.refresh_git_data();
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to commit");
                        self.store.dispatch(AppAction::SetRepoError(Some(format!(
                            "Commit failed: {}",
                            e
                        ))));
                    }
                }
            }
            CommitAction::UnstageAll => match repo.unstage_all() {
                Ok(()) => self.refresh_git_data(),
                Err(e) => tracing::error!(error = %e, "Failed to unstage all files"),
            },
        }
    }

    fn handle_stash_action(&mut self, action: StashAction, ctx: &egui::Context) {
        if self.git_repo.is_none() {
            return;
        }

        let msg = match &action {
            StashAction::Save(_) => "Stashing changes...",
            StashAction::Pop(_) => "Popping stash...",
            StashAction::Apply(_) => "Applying stash...",
            StashAction::Drop(_) => "Dropping stash...",
        };
        self.store
            .dispatch(AppAction::SetRepoError(Some(msg.to_string())));

        let ctx = ctx.clone();
        self.run_background_git_job(ctx, move |repo| match action {
            StashAction::Save(msg) => repo.stash_save(msg.as_deref()),
            StashAction::Pop(idx) => repo.stash_pop(idx),
            StashAction::Apply(idx) => repo.stash_apply(idx),
            StashAction::Drop(idx) => repo.stash_drop(idx),
        });
    }

    fn handle_branch_action(&mut self, action: BranchAction, ctx: &egui::Context) {
        if self.git_repo.is_none() {
            return;
        }

        let msg = match &action {
            BranchAction::Create(name) => format!("Creating branch {}...", name),
            BranchAction::Checkout(name) => format!("Checking out branch {}...", name),
            BranchAction::Delete(name) => format!("Deleting branch {}...", name),
            BranchAction::CreateAndCheckout(name) => {
                format!("Creating and checking out branch {}...", name)
            }
        };
        self.store
            .dispatch(AppAction::SetRepoError(Some(msg.to_string())));

        let ctx = ctx.clone();
        self.run_background_git_job(ctx, move |repo| match action {
            BranchAction::Create(name) => repo.create_branch(&name),
            BranchAction::Checkout(name) => repo.checkout_branch(&name),
            BranchAction::Delete(name) => repo.delete_branch(&name),
            BranchAction::CreateAndCheckout(name) => {
                repo.create_branch(&name)?;
                repo.checkout_branch(&name)
            }
        });
    }

    fn handle_quick_launch_action(&mut self, action: QuickLaunchAction, ctx: &egui::Context) {
        match action {
            QuickLaunchAction::OpenRepository => {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    let path = path.to_string_lossy().to_string();
                    self.open_repo(&path);
                }
            }
            QuickLaunchAction::ExitApp => {
                tracing::info!("Exiting app via quick launch");
                self.persist_session();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            QuickLaunchAction::OpenLogs => {
                self.debug_open = true;
            }
            QuickLaunchAction::Fetch => {
                self.store
                    .dispatch(AppAction::SetRepoError(Some("Fetching...".to_string())));
                self.run_background_git_job(ctx.clone(), |repo| repo.fetch());
            }
            QuickLaunchAction::Pull => {
                self.store
                    .dispatch(AppAction::SetRepoError(Some("Pulling...".to_string())));
                self.run_background_git_job(ctx.clone(), |repo| repo.pull());
            }
            QuickLaunchAction::Push => {
                self.store
                    .dispatch(AppAction::SetRepoError(Some("Pushing...".to_string())));
                self.run_background_git_job(ctx.clone(), |repo| repo.push());
            }
            QuickLaunchAction::StageAll => {
                if let Some(repo) = &self.git_repo {
                    match repo.stage_all() {
                        Ok(()) => self.refresh_git_data(),
                        Err(e) => tracing::error!(error = %e, "Failed to stage all"),
                    }
                }
            }
            QuickLaunchAction::DiscardAll => {
                if let Some(repo) = &self.git_repo {
                    match repo.discard_all() {
                        Ok(()) => self.refresh_git_data(),
                        Err(e) => tracing::error!(error = %e, "Failed to discard all"),
                    }
                }
            }
            QuickLaunchAction::CreateBranch => {
                self.show_create_branch_dialog = true;
                self.new_branch_name.clear();
            }
        }
    }

    fn run_background_git_job<F>(&self, ctx: egui::Context, f: F)
    where
        F: FnOnce(&GitRepo) -> Result<(), palimpsest::git::error::GitError> + Send + 'static,
    {
        let store = self.store.clone();
        let current_repo_path = match store.get_state().current_repo.clone() {
            Some(path) => path,
            None => return,
        };

        std::thread::spawn(move || {
            let repo = match GitRepo::open(&current_repo_path) {
                Ok(r) => r,
                Err(e) => {
                    if store.get_state().current_repo.as_ref() == Some(&current_repo_path) {
                        store.dispatch(AppAction::SetRepoError(Some(format!(
                            "Failed to open repo: {}",
                            e
                        ))));
                        ctx.request_repaint();
                    }
                    return;
                }
            };

            if let Err(e) = f(&repo) {
                if store.get_state().current_repo.as_ref() == Some(&current_repo_path) {
                    store.dispatch(AppAction::SetRepoError(Some(e.to_string())));
                    ctx.request_repaint();
                }
                return;
            }

            let mut errors = Vec::new();
            let commits = match repo.commits(Some(200)) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(e.to_string());
                    Vec::new()
                }
            };
            let branches = match repo.branches() {
                Ok(b) => b,
                Err(e) => {
                    errors.push(e.to_string());
                    Vec::new()
                }
            };
            let remotes = match repo.remotes() {
                Ok(r) => r,
                Err(e) => {
                    errors.push(e.to_string());
                    Vec::new()
                }
            };
            let tags = match repo.tags() {
                Ok(t) => t,
                Err(e) => {
                    errors.push(e.to_string());
                    Vec::new()
                }
            };
            let stashes = match repo.stashes() {
                Ok(s) => s,
                Err(e) => {
                    errors.push(e.to_string());
                    Vec::new()
                }
            };
            let status = match repo.status() {
                Ok(s) => s,
                Err(e) => {
                    errors.push(e.to_string());
                    palimpsest::git::models::RepoStatus {
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
            };

            if store.get_state().current_repo.as_ref() == Some(&current_repo_path) {
                store.dispatch(AppAction::RefreshGitData {
                    commits,
                    branches,
                    remotes,
                    tags,
                    stashes,
                    status,
                });

                if errors.is_empty() {
                    store.dispatch(AppAction::SetRepoError(None));
                } else {
                    store.dispatch(AppAction::SetRepoError(Some(errors.join("; "))));
                }

                ctx.request_repaint();
            }
        });
    }

    fn fetch_manager_details(&mut self, path: &str) {
        if let Some(details) = &self.store.get_state().manager_details {
            if details.repo_path == path {
                if details.is_org.is_none() || details.is_private.is_none() {
                    self.trigger_github_metadata_fetch(details.clone());
                }
                return;
            }
        }
        if let Some(cached) = self
            .store
            .get_state()
            .manager_details_cache
            .iter()
            .find(|(k, _)| k == path)
            .map(|(_, v)| v.clone())
        {
            self.store
                .dispatch(AppAction::SetManagerDetails(Some(cached.clone())));
            if cached.is_org.is_none() || cached.is_private.is_none() {
                self.trigger_github_metadata_fetch(cached);
            }
            return;
        }
        use palimpsest::state::{
            ManagerBranch, ManagerCommit, ManagerRemote, ManagerRepoDetails, ManagerTag,
        };
        use palimpsest::ui::repo_manager::{format_relative_time, parse_tag_version};

        match GitRepo::open(path) {
            Ok(repo) => {
                let repo_name = repo.repo_name().unwrap_or_else(|| path.to_string());
                let branch = repo.head_branch().unwrap_or_else(|_| "HEAD".to_string());

                let status = repo.status().ok();
                let uncommitted = status
                    .as_ref()
                    .map_or(0, |s| s.staged_count + s.unstaged_count);

                let commits = repo.commits(Some(20)).unwrap_or_default();
                let (total_commits, oldest_commit) = repo.history_stats().unwrap_or((0, None));

                let initial_date = oldest_commit.map_or("unknown".to_string(), |c| {
                    format_relative_time(
                        c.timestamp
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0),
                    )
                });
                let last_date = commits.first().map_or("unknown".to_string(), |c| {
                    format_relative_time(
                        c.timestamp
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_secs() as i64)
                            .unwrap_or(0),
                    )
                });

                let state = self.store.get_state();
                let remotes = repo.remotes().unwrap_or_default();
                let owned_by_authed_user = palimpsest::git::live::classify_repo_ownership(
                    &remotes,
                    state.github_user.as_ref().map(|user| user.login.as_str()),
                );
                let manager_remotes: Vec<ManagerRemote> = remotes
                    .iter()
                    .map(|r| {
                        let is_github = palimpsest::ui::repo_manager::is_github_url(&r.url);
                        ManagerRemote {
                            name: r.name.clone(),
                            url: r.url.clone(),
                            is_github,
                        }
                    })
                    .collect();

                let branches = repo.branches().unwrap_or_default();
                let manager_branches: Vec<ManagerBranch> = branches
                    .iter()
                    .take(5)
                    .map(|b| {
                        let tip_commit = commits.iter().find(|c| {
                            c.hash.starts_with(&b.tip_hash) || c.short_hash == b.tip_hash
                        });
                        ManagerBranch {
                            name: b.name.clone(),
                            last_message: tip_commit
                                .map(|c| c.message.lines().next().unwrap_or("").to_string())
                                .unwrap_or_default(),
                            author: tip_commit.map(|c| c.author.clone()).unwrap_or_default(),
                            relative_date: tip_commit
                                .map(|c| {
                                    format_relative_time(
                                        c.timestamp
                                            .duration_since(std::time::UNIX_EPOCH)
                                            .map(|d| d.as_secs() as i64)
                                            .unwrap_or(0),
                                    )
                                })
                                .unwrap_or_default(),
                        }
                    })
                    .collect();

                let mut tags = repo.tags().unwrap_or_default();
                tags.sort_by(|a, b| {
                    let va = parse_tag_version(&a.name);
                    let vb = parse_tag_version(&b.name);
                    vb.cmp(&va)
                });
                let manager_tags: Vec<ManagerTag> = tags
                    .iter()
                    .take(5)
                    .map(|t| ManagerTag {
                        name: t.name.clone(),
                        author: t.author.clone(),
                        relative_date: format_relative_time(
                            t.timestamp
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs() as i64)
                                .unwrap_or(0),
                        ),
                    })
                    .collect();

                let manager_commits: Vec<ManagerCommit> = commits
                    .iter()
                    .take(5)
                    .map(|c| ManagerCommit {
                        message: c.message.lines().next().unwrap_or("").to_string(),
                        author: c.author.clone(),
                        relative_date: format_relative_time(
                            c.timestamp
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| d.as_secs() as i64)
                                .unwrap_or(0),
                        ),
                    })
                    .collect();

                let details = ManagerRepoDetails {
                    repo_path: path.to_string(),
                    repo_name,
                    branch,
                    uncommitted_files: uncommitted,
                    total_commits,
                    initial_commit_date: initial_date,
                    last_commit_date: last_date,
                    remotes: manager_remotes,
                    branches: manager_branches,
                    tags: manager_tags,
                    commits: manager_commits,
                    owned_by_authed_user,
                    is_org: None,
                    is_private: None,
                };

                self.store
                    .dispatch(AppAction::SetManagerDetails(Some(details.clone())));
                self.trigger_github_metadata_fetch(details);
            }
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "Repo not found, removing from recents");
                self.store
                    .dispatch(AppAction::RemoveRecentRepo(path.to_string()));
                self.store.dispatch(AppAction::SelectManagerRepo(None));
            }
        }
    }

    fn trigger_github_metadata_fetch(&mut self, details: palimpsest::state::ManagerRepoDetails) {
        if self.repo_metadata_fetches.contains(&details.repo_path) {
            return;
        }

        let creds = palimpsest::auth::credentials::load_credentials();
        let Some(token) = creds.github_token.clone() else {
            return;
        };

        let mut gh_remote = None;
        for remote in &details.remotes {
            if remote.is_github {
                if let Some((owner, repo)) = parse_github_remote(&remote.url) {
                    gh_remote = Some((owner, repo));
                    break;
                }
            }
        }

        let Some((owner, repo)) = gh_remote else {
            return;
        };

        self.repo_metadata_fetches.insert(details.repo_path.clone());

        let store = self.store.clone();
        let details_clone = details.clone();

        tracing::debug!("Triggering background GitHub metadata fetch for {owner}/{repo}");

        std::thread::spawn(move || {
            match palimpsest::auth::github_api::get_repo_metadata(&token, &owner, &repo) {
                Ok(meta) => {
                    tracing::info!(
                        "Successfully fetched GitHub metadata for {owner}/{repo}: is_org={}, is_private={}",
                        meta.is_org,
                        meta.is_private
                    );
                    let mut updated = details_clone;
                    updated.is_org = Some(meta.is_org);
                    updated.is_private = Some(meta.is_private);
                    store.dispatch(AppAction::SetManagerDetails(Some(updated)));
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch repo metadata for {owner}/{repo}: {e}");
                    let mut updated = details_clone;
                    updated.is_org = None;
                    updated.is_private = None;
                    store.dispatch(AppAction::SetManagerDetails(Some(updated)));
                }
            }
        });
    }

    fn get_or_fetch_avatar(
        &mut self,
        author_name: &str,
        author_email: Option<&str>,
    ) -> Option<String> {
        let name = author_name.trim().to_string();
        if name.is_empty() {
            return None;
        }

        // 1. Check state in-memory cache
        let state = self.store.get_state();
        if let Some(path) = state.avatar_cache.get(&name) {
            return Some(path.clone());
        }

        // 2. Check disk cache
        let hash = sha256_hash(&name);
        if let Some(dir) = avatars_dir() {
            for ext in &["png", "svg"] {
                let path = dir.join(format!("{hash}.{ext}"));
                if path.exists() {
                    let path_str = path.to_string_lossy().to_string();
                    self.store.dispatch(AppAction::SetAvatarPath {
                        key: name.clone(),
                        path: path_str.clone(),
                    });
                    return Some(path_str);
                }
            }
        }

        // Special handling for bot accounts with local SVG assets embedded at compile time
        let bot_asset_bytes: Option<&[u8]> =
            if name == "github-actions[bot]" || name == "github-actions" {
                Some(include_bytes!("assets/GitHub_Invertocat_White.svg"))
            } else if name == "copilot-swe-agent[bot]" {
                Some(include_bytes!("assets/Copilot_Icon_White.svg"))
            } else {
                None
            };

        if let Some(bytes) = bot_asset_bytes {
            if let Some(dir) = avatars_dir() {
                let dest_path = dir.join(format!("{hash}.svg"));
                if let Some(parent) = dest_path.parent() {
                    let _ = std::fs::create_dir_all(parent);
                }
                if std::fs::write(&dest_path, bytes).is_ok() {
                    let path_str = dest_path.to_string_lossy().to_string();
                    self.store.dispatch(AppAction::SetAvatarPath {
                        key: name.clone(),
                        path: path_str.clone(),
                    });
                    return Some(path_str);
                }
            }
        }

        // 3. Trigger background fetch if not already in progress
        self.trigger_avatar_fetch(&name, author_email);
        None
    }

    fn trigger_avatar_fetch(&mut self, author_name: &str, author_email: Option<&str>) {
        if self.avatar_fetches.contains(author_name) {
            return;
        }

        self.avatar_fetches.insert(author_name.to_string());

        let creds = palimpsest::auth::credentials::load_credentials();
        let token = creds.github_token.clone();

        let name = author_name.to_string();
        let email = author_email.map(|s| s.to_string());
        let store = self.store.clone();
        let egui_ctx = self.egui_ctx.clone();

        std::thread::spawn(move || {
            let hash = sha256_hash(&name);
            let Some(dir) = avatars_dir() else {
                return;
            };
            let dest_path = dir.join(format!("{hash}.png"));

            tracing::debug!(
                "Triggering background GitHub avatar search for '{}' (email: {:?})",
                name,
                email
            );

            let mut avatar_url = match palimpsest::auth::github_api::fetch_avatar_url(
                token.as_deref(),
                email.as_deref(),
                &name,
            ) {
                Ok(Some(url)) => {
                    tracing::info!("Resolved GitHub avatar URL for '{}': {}", name, url);
                    Some(url)
                }
                Ok(None) => {
                    tracing::debug!("GitHub search returned no users for '{}'", name);
                    None
                }
                Err(e) => {
                    tracing::warn!("Failed to search GitHub avatar for '{}': {}", name, e);
                    None
                }
            };

            // 2. If GitHub search returns nothing, fall back to Github identicons
            if avatar_url.is_none() {
                let url = format!("https://github.com/identicons/{}.png", hash);
                tracing::debug!(
                    "Author '{}' not found on GitHub, using identicon fallback: {}",
                    name,
                    url
                );
                avatar_url = Some(url);
            }

            // 3. Download the avatar image
            if let Some(url) = avatar_url {
                tracing::debug!("Downloading avatar for '{}' from {}", name, url);
                match palimpsest::auth::github_api::download_avatar_image(&url, &dest_path) {
                    Ok(_) => {
                        let path_str = dest_path.to_string_lossy().to_string();
                        tracing::info!(
                            "Saved downloaded avatar for '{}' to disk cache: {}",
                            name,
                            path_str
                        );
                        store.dispatch(AppAction::SetAvatarPath {
                            key: name.clone(),
                            path: path_str,
                        });
                        egui_ctx.request_repaint();
                    }
                    Err(e) => {
                        tracing::warn!("Failed to download avatar for {}: {}", name, e);
                    }
                }
            }
        });
    }

    fn ensure_avatar_fetches(&mut self) {
        let state = self.store.get_state();

        for commit in &state.cached_commits {
            self.get_or_fetch_avatar(&commit.author, None);
        }

        if let Some(details) = &state.manager_details {
            for commit in &details.commits {
                self.get_or_fetch_avatar(&commit.author, None);
            }
            for branch in &details.branches {
                self.get_or_fetch_avatar(&branch.author, None);
            }
            for tag in &details.tags {
                self.get_or_fetch_avatar(&tag.author, None);
            }
        }
    }
}

fn sha256_hash(input: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn avatars_dir() -> Option<std::path::PathBuf> {
    directories::ProjectDirs::from("io", "parazeeknova", "Palimpsest")
        .map(|dirs| dirs.data_dir().join("avatars"))
}

impl eframe::App for PalimpsestApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let pending = {
            let mut guard = self.pending_open_repo.lock().unwrap();
            guard.take()
        };
        if let Some(path) = pending {
            self.open_repo(&path);
        }

        self.process_repo_live_updates(ui.ctx());

        let state = self.store.get_state();
        let current_login = state.github_user.as_ref().map(|user| user.login.clone());
        if self.authed_github_login != current_login {
            self.authed_github_login = current_login;
            self.repo_live_states.clear();
            for (_, handle) in self.repo_live_trackers.drain() {
                handle.stop.store(true, Ordering::Relaxed);
            }
            for tab_path in &state.open_tabs {
                self.ensure_repo_tracker(tab_path, ui.ctx());
            }
        }

        self.ensure_ownership_probes();
        self.ensure_avatar_fetches();

        let background = ui.visuals().widgets.inactive.bg_fill;
        ui.painter().rect_filled(ui.max_rect(), 0.0, background);

        // Render Setup Wizard modal if setup is not completed or if triggered manually from menu
        let force_show_wizard = !state.setup_completed || self.show_setup_wizard_dialog;
        if force_show_wizard {
            if let Some(ref user) = state.github_user {
                self.setup_wizard_state.github_user =
                    Some(palimpsest::auth::github_oauth::GitHubUser {
                        login: user.login.clone(),
                        name: user.name.clone(),
                        email: user.email.clone(),
                        avatar_url: user.avatar_url.clone(),
                        html_url: user.html_url.clone(),
                        bio: user.bio.clone(),
                    });
                self.setup_wizard_state.auth_polling = false;
            } else if let palimpsest::state::AuthStatus::Failed(ref err) = state.auth_status {
                self.setup_wizard_state.auth_error = Some(err.clone());
                self.setup_wizard_state.auth_polling = false;
            }

            let wizard_action = setup_wizard::show(ui, &mut self.setup_wizard_state);
            match wizard_action {
                setup_wizard::WizardAction::StartDetection => {
                    let identity = palimpsest::auth::git_identity::detect_git_config();
                    let gh_cli = palimpsest::auth::git_identity::detect_gh_cli_auth();
                    let ssh_keys = palimpsest::auth::git_identity::detect_ssh_keys();
                    let gpg_keys = palimpsest::auth::git_identity::detect_gpg_keys();

                    self.setup_wizard_state.git_name = identity.name.clone().unwrap_or_default();
                    self.setup_wizard_state.git_email = identity.email.clone().unwrap_or_default();
                    self.setup_wizard_state.identity = Some(identity);
                    self.setup_wizard_state.gh_cli_status = gh_cli;
                    self.setup_wizard_state.ssh_keys = ssh_keys;
                    self.setup_wizard_state.gpg_keys = gpg_keys;
                    self.setup_wizard_state.detection_started = true;
                }
                setup_wizard::WizardAction::StartDeviceFlow => {
                    let client_id = palimpsest::auth::github_oauth::GITHUB_CLIENT_ID;
                    match palimpsest::auth::github_oauth::request_device_code(client_id) {
                        Ok(res) => {
                            self.setup_wizard_state.device_code_response =
                                Some(setup_wizard::DeviceFlowUiState {
                                    user_code: res.user_code.clone(),
                                    verification_uri: res.verification_uri.clone(),
                                });
                            self.setup_wizard_state.auth_polling = true;
                            self.setup_wizard_state.auth_error = None;

                            let store = self.store.clone();
                            let ctx = ui.ctx().clone();
                            let device_code = res.device_code;
                            let interval = res.interval;
                            let expires_in = res.expires_in;

                            std::thread::spawn(move || {
                                let start = std::time::Instant::now();
                                loop {
                                    if start.elapsed().as_secs() > expires_in {
                                        store.dispatch(AppAction::SetAuthStatus(
                                            palimpsest::state::AuthStatus::Failed(
                                                "Device code expired".to_string(),
                                            ),
                                        ));
                                        break;
                                    }
                                    std::thread::sleep(std::time::Duration::from_secs(
                                        interval.max(5),
                                    ));
                                    match palimpsest::auth::github_oauth::poll_for_token(
                                        client_id,
                                        &device_code,
                                    ) {
                                        Ok(token_res) => {
                                            match palimpsest::auth::github_oauth::fetch_user_profile(
                                                &token_res.access_token,
                                            ) {
                                                Ok(user) => {
                                                    let mut creds = palimpsest::auth::credentials::load_credentials();
                                                    creds.github_token =
                                                        Some(token_res.access_token.clone());
                                                    creds.github_user = Some(user.clone());
                                                    if let Err(e) = palimpsest::auth::credentials::save_credentials(&creds) {
                                                        tracing::warn!("Failed to save credentials: {}", e);
                                                    }

                                                    store.dispatch(AppAction::SetGitHubUser(Some(
                                                        palimpsest::state::GitHubUserProfile {
                                                            login: user.login,
                                                            name: user.name,
                                                            email: user.email,
                                                            avatar_url: user.avatar_url,
                                                            html_url: user.html_url,
                                                            bio: user.bio,
                                                        },
                                                    )));
                                                    store.dispatch(AppAction::SetAuthStatus(
                                                        palimpsest::state::AuthStatus::Connected,
                                                    ));
                                                    ctx.request_repaint();
                                                    break;
                                                }
                                                Err(e) => {
                                                    store.dispatch(AppAction::SetAuthStatus(
                                                        palimpsest::state::AuthStatus::Failed(
                                                            format!(
                                                                "Failed to fetch profile: {}",
                                                                e
                                                            ),
                                                        ),
                                                    ));
                                                    ctx.request_repaint();
                                                    break;
                                                }
                                            }
                                        }
                                        Err(palimpsest::auth::github_oauth::AuthError::Pending) => {
                                        }
                                        Err(e) => {
                                            store.dispatch(AppAction::SetAuthStatus(
                                                palimpsest::state::AuthStatus::Failed(format!(
                                                    "Connection failed: {}",
                                                    e
                                                )),
                                            ));
                                            ctx.request_repaint();
                                            break;
                                        }
                                    }
                                }
                            });
                        }
                        Err(e) => {
                            self.setup_wizard_state.auth_error =
                                Some(format!("Failed to connect: {}", e));
                        }
                    }
                }
                setup_wizard::WizardAction::OpenVerificationUrl(verification_url) => {
                    open_url(&verification_url);
                }
                setup_wizard::WizardAction::Complete {
                    git_name,
                    git_email,
                } => {
                    let mut creds = palimpsest::auth::credentials::load_credentials();
                    creds.git_name = Some(git_name.clone());
                    creds.git_email = Some(git_email.clone());
                    creds.setup_completed = true;
                    if let Err(e) = palimpsest::auth::credentials::save_credentials(&creds) {
                        tracing::warn!("Failed to save credentials: {}", e);
                    }
                    self.store.dispatch(AppAction::SetSetupCompleted(true));
                    self.show_setup_wizard_dialog = false;
                    let cached_id = palimpsest::state::CachedGitIdentity {
                        name: Some(git_name),
                        email: Some(git_email),
                        signing_key: None,
                        gpg_sign_commits: false,
                        ssh_key_count: self.setup_wizard_state.ssh_keys.len(),
                        gpg_key_count: self.setup_wizard_state.gpg_keys.len(),
                    };
                    self.store
                        .dispatch(AppAction::SetGitIdentity(Some(cached_id)));
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            960.0, 720.0,
                        )));
                }
                setup_wizard::WizardAction::Skip => {
                    let mut creds = palimpsest::auth::credentials::load_credentials();
                    creds.setup_completed = true;
                    let _ = palimpsest::auth::credentials::save_credentials(&creds);
                    self.store.dispatch(AppAction::SetSetupCompleted(true));
                    self.show_setup_wizard_dialog = false;
                    ui.ctx()
                        .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                            960.0, 720.0,
                        )));
                }
                setup_wizard::WizardAction::None => {}
            }
            return;
        }

        // Auto fetch remote GitHub data if repository changes
        if state.current_repo != self.last_fetched_repo {
            self.last_fetched_repo = state.current_repo.clone();
            self.refresh_github_remote_data(ui.ctx().clone());
        }

        let repo_name = self.repo_name();
        let repo_name_ref = repo_name.as_deref();
        let mut show_window_buttons = self.store.get_state().show_window_buttons;

        let (open_repo, profile_action) = titlebar::show(
            ui,
            frame,
            &mut self.titlebar_menu_open,
            &mut self.search_query,
            repo_name_ref,
            &state.recent_repos,
            &mut show_window_buttons,
            &mut self.debug_open,
            &mut self.profile_panel_state,
            state.github_user.as_ref(),
            state.git_identity.as_ref(),
            &state.auth_status,
        );

        if show_window_buttons != self.store.get_state().show_window_buttons {
            tracing::info!(
                "Window buttons visibility toggled to {}",
                show_window_buttons
            );
            self.store
                .dispatch(AppAction::ToggleWindowButtons(show_window_buttons));
            self.persist_session();
        }

        match open_repo {
            titlebar::OpenAction::PickFolder => {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    let path = path.to_string_lossy().to_string();
                    self.open_repo(&path);
                }
            }
            titlebar::OpenAction::SelectRecent(index) => {
                if let Some(repo) = self.store.get_state().recent_repos.get(index) {
                    tracing::info!(repo = %repo.path, "Selecting recent repository");
                    self.open_repo(&repo.path);
                }
            }
            titlebar::OpenAction::InitRepo => {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    match git2::Repository::init(&path) {
                        Ok(_) => {
                            let path_str = path.to_string_lossy().to_string();
                            self.open_repo(&path_str);
                        }
                        Err(e) => {
                            self.store.dispatch(AppAction::SetRepoError(Some(format!(
                                "Failed to initialize repository: {}",
                                e
                            ))));
                        }
                    }
                }
            }
            titlebar::OpenAction::CloneRepo => {
                self.show_clone_dialog = true;
            }
            titlebar::OpenAction::NewTab => {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    let path = path.to_string_lossy().to_string();
                    self.open_repo(&path);
                }
            }
            titlebar::OpenAction::QuickLaunch => {
                self.show_command_palette = true;
            }
            titlebar::OpenAction::CloseTab => {
                if let Some(index) = self.store.get_state().active_tab {
                    self.close_tab(index);
                }
            }
            titlebar::OpenAction::ConfigureSsh => {
                self.setup_wizard_state.step = setup_wizard::WizardStep::SshGpgKeys;
                self.show_setup_wizard_dialog = true;
            }
            titlebar::OpenAction::Accounts => {
                self.setup_wizard_state.step = setup_wizard::WizardStep::GitHubAuth;
                self.show_setup_wizard_dialog = true;
            }
            titlebar::OpenAction::CheckUpdates => {
                self.show_update_dialog = true;
            }
            titlebar::OpenAction::Preferences => {
                self.show_preferences_dialog = true;
            }
            titlebar::OpenAction::Exit => {
                self.persist_session();
                ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
            }
            titlebar::OpenAction::NextTab => {
                let state = self.store.get_state();
                if !state.open_tabs.is_empty() {
                    if let Some(index) = state.active_tab {
                        let next_index = (index + 1) % state.open_tabs.len();
                        self.activate_tab(next_index);
                    }
                }
            }
            titlebar::OpenAction::PrevTab => {
                let state = self.store.get_state();
                if !state.open_tabs.is_empty() {
                    if let Some(index) = state.active_tab {
                        let prev_index =
                            (index + state.open_tabs.len() - 1) % state.open_tabs.len();
                        self.activate_tab(prev_index);
                    }
                }
            }
            titlebar::OpenAction::Refresh => {
                self.refresh_git_data();
                self.refresh_github_remote_data(ui.ctx().clone());
            }
            titlebar::OpenAction::Fetch => {
                let ctx = ui.ctx().clone();
                self.handle_quick_launch_action(QuickLaunchAction::Fetch, &ctx);
            }
            titlebar::OpenAction::Pull => {
                let ctx = ui.ctx().clone();
                self.handle_quick_launch_action(QuickLaunchAction::Pull, &ctx);
            }
            titlebar::OpenAction::Push => {
                let ctx = ui.ctx().clone();
                self.handle_quick_launch_action(QuickLaunchAction::Push, &ctx);
            }
            titlebar::OpenAction::SaveStash => {
                self.show_save_stash_dialog = true;
                self.new_stash_message.clear();
            }
            titlebar::OpenAction::NewBranch => {
                let ctx = ui.ctx().clone();
                self.handle_quick_launch_action(QuickLaunchAction::CreateBranch, &ctx);
            }
            titlebar::OpenAction::NewTag => {
                self.show_create_tag_dialog = true;
                self.new_tag_name.clear();
            }
            titlebar::OpenAction::NewWorktree => {
                self.store.dispatch(AppAction::SetRepoError(Some(
                    "New Worktree functionality coming soon".to_string(),
                )));
            }
            titlebar::OpenAction::GitFlow => {
                self.store.dispatch(AppAction::SetRepoError(Some(
                    "Git Flow integration coming soon".to_string(),
                )));
            }
            titlebar::OpenAction::GitLfs => {
                self.store.dispatch(AppAction::SetRepoError(Some(
                    "Git LFS integration coming soon".to_string(),
                )));
            }
            titlebar::OpenAction::ApplyPatch => {
                self.store.dispatch(AppAction::SetRepoError(Some(
                    "Apply Patch functionality coming soon".to_string(),
                )));
            }
            titlebar::OpenAction::Bisect => {
                self.store.dispatch(AppAction::SetRepoError(Some(
                    "Bisect functionality coming soon".to_string(),
                )));
            }
            titlebar::OpenAction::OpenInFileExplorer => {
                if let Some(path) = self.store.get_state().current_repo.clone() {
                    open_url(&path);
                }
            }
            titlebar::OpenAction::OpenInConsole => {
                if let Some(path) = self.store.get_state().current_repo.clone() {
                    self.open_in_console(&path);
                }
            }
            titlebar::OpenAction::RepositoryStatistics => {
                self.store.dispatch(AppAction::SetRepoError(Some(
                    "Repository Statistics coming soon".to_string(),
                )));
            }
            titlebar::OpenAction::RepositoryTreemap => {
                self.store.dispatch(AppAction::SetRepoError(Some(
                    "Repository Treemap coming soon".to_string(),
                )));
            }
            titlebar::OpenAction::PerformanceBenchmark => {
                self.store.dispatch(AppAction::SetRepoError(Some(
                    "Performance Benchmark coming soon".to_string(),
                )));
            }
            titlebar::OpenAction::RepositorySettings => {
                self.store.dispatch(AppAction::SetRepoError(Some(
                    "Repository Settings coming soon".to_string(),
                )));
            }
            titlebar::OpenAction::None => {}
        }

        match profile_action {
            profile_panel::ProfileAction::SignOut => {
                let mut creds = palimpsest::auth::credentials::load_credentials();
                palimpsest::auth::credentials::clear_github_auth(&mut creds, true);
                self.store.dispatch(AppAction::SetGitHubUser(None));
                self.store.dispatch(AppAction::SetAuthStatus(
                    palimpsest::state::AuthStatus::Disconnected,
                ));
                self.store.dispatch(AppAction::SetGitHubData {
                    pull_requests: Vec::new(),
                    action_runs: Vec::new(),
                    releases: Vec::new(),
                    packages: Vec::new(),
                });
            }
            profile_panel::ProfileAction::RerunSetup => {
                self.store.dispatch(AppAction::SetSetupCompleted(false));
                self.setup_wizard_state = setup_wizard::SetupWizardState::default();
                ui.ctx()
                    .send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(500.0, 450.0)));
            }
            profile_panel::ProfileAction::OpenGitHubProfile => {
                if let Some(user) = &state.github_user {
                    open_url(&user.html_url);
                }
            }
            profile_panel::ProfileAction::None => {}
        }

        let state = self.store.get_state();
        let repo_name = self.repo_name();
        let current_branch = state.cached_status.as_ref().map(|s| s.branch.as_str());
        let toolbar_action = toolbar::show(
            ui,
            repo_name.as_deref(),
            current_branch,
            &state,
            self.current_repo_owned_by_authed_user,
        );
        let ctx = ui.ctx().clone();

        match toolbar_action {
            toolbar::ToolbarAction::QuickLaunch => {
                self.show_command_palette = true;
            }
            toolbar::ToolbarAction::Fetch => {
                self.handle_quick_launch_action(QuickLaunchAction::Fetch, &ctx);
                self.refresh_github_remote_data(ctx.clone());
            }
            toolbar::ToolbarAction::Pull => {
                self.handle_quick_launch_action(QuickLaunchAction::Pull, &ctx);
            }
            toolbar::ToolbarAction::Push => {
                self.handle_quick_launch_action(QuickLaunchAction::Push, &ctx);
            }
            toolbar::ToolbarAction::StashSave => {
                self.handle_stash_action(StashAction::Save(None), &ctx);
            }
            toolbar::ToolbarAction::StashApply => {
                self.handle_stash_action(StashAction::Apply(0), &ctx);
            }
            toolbar::ToolbarAction::StashPop => {
                self.handle_stash_action(StashAction::Pop(0), &ctx);
            }
            toolbar::ToolbarAction::NewBranch => {
                self.show_create_branch_dialog = true;
                self.new_branch_name.clear();
            }
            toolbar::ToolbarAction::None => {}
        }

        if command_palette::check_shortcut(ui.ctx()) {
            self.show_command_palette = true;
        }

        if self.show_command_palette {
            let ctx = ui.ctx().clone();
            match command_palette::show(
                &ctx,
                &mut self.command_palette_state,
                self.git_repo.is_some(),
            ) {
                PaletteResult::Action(a) => {
                    self.show_command_palette = false;
                    let ctx = ui.ctx().clone();
                    self.handle_quick_launch_action(a, &ctx);
                }
                PaletteResult::Closed => {
                    self.show_command_palette = false;
                }
                PaletteResult::StillOpen => {}
            }
        }

        if !self.show_command_palette {
            let ctx = ui.ctx();
            let command = primary_modifiers();
            let open_shortcut = egui::KeyboardShortcut::new(command, egui::Key::O);
            let exit_shortcut = egui::KeyboardShortcut::new(command, egui::Key::Q);
            let logs_shortcut =
                egui::KeyboardShortcut::new(command.plus(egui::Modifiers::SHIFT), egui::Key::L);
            let init_shortcut =
                egui::KeyboardShortcut::new(command.plus(egui::Modifiers::SHIFT), egui::Key::N);
            let clone_shortcut = egui::KeyboardShortcut::new(command, egui::Key::N);
            let tab_shortcut = egui::KeyboardShortcut::new(command, egui::Key::T);
            let quick_shortcut = egui::KeyboardShortcut::new(command, egui::Key::P);
            let close_shortcut = egui::KeyboardShortcut::new(command, egui::Key::W);
            let prefs_shortcut = egui::KeyboardShortcut::new(command, egui::Key::Comma);

            let refresh_shortcut =
                egui::KeyboardShortcut::new(egui::Modifiers::NONE, egui::Key::F5);
            let fetch_shortcut =
                egui::KeyboardShortcut::new(command.plus(egui::Modifiers::SHIFT), egui::Key::F);
            let pull_shortcut =
                egui::KeyboardShortcut::new(command.plus(egui::Modifiers::SHIFT), egui::Key::L);
            let push_shortcut =
                egui::KeyboardShortcut::new(command.plus(egui::Modifiers::SHIFT), egui::Key::P);
            let save_stash_shortcut =
                egui::KeyboardShortcut::new(command.plus(egui::Modifiers::SHIFT), egui::Key::H);
            let new_branch_shortcut =
                egui::KeyboardShortcut::new(command.plus(egui::Modifiers::SHIFT), egui::Key::B);
            let new_tag_shortcut =
                egui::KeyboardShortcut::new(command.plus(egui::Modifiers::SHIFT), egui::Key::T);
            let open_explorer_shortcut =
                egui::KeyboardShortcut::new(command.plus(egui::Modifiers::ALT), egui::Key::O);
            let open_console_shortcut =
                egui::KeyboardShortcut::new(command.plus(egui::Modifiers::ALT), egui::Key::T);
            let next_tab_shortcut =
                egui::KeyboardShortcut::new(egui::Modifiers::CTRL, egui::Key::Tab);
            let prev_tab_shortcut = egui::KeyboardShortcut::new(
                egui::Modifiers::CTRL.plus(egui::Modifiers::SHIFT),
                egui::Key::Tab,
            );

            if ctx.input_mut(|i| i.consume_shortcut(&open_shortcut)) {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    let path = path.to_string_lossy().to_string();
                    self.open_repo(&path);
                }
            }
            if ctx.input_mut(|i| i.consume_shortcut(&init_shortcut)) {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    match git2::Repository::init(&path) {
                        Ok(_) => {
                            let path_str = path.to_string_lossy().to_string();
                            self.open_repo(&path_str);
                        }
                        Err(e) => {
                            self.store.dispatch(AppAction::SetRepoError(Some(format!(
                                "Failed to initialize repository: {}",
                                e
                            ))));
                        }
                    }
                }
            }
            if ctx.input_mut(|i| i.consume_shortcut(&clone_shortcut)) {
                self.show_clone_dialog = true;
            }
            if ctx.input_mut(|i| i.consume_shortcut(&tab_shortcut)) {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    let path = path.to_string_lossy().to_string();
                    self.open_repo(&path);
                }
            }
            if ctx.input_mut(|i| i.consume_shortcut(&quick_shortcut)) {
                self.show_command_palette = true;
            }
            if ctx.input_mut(|i| i.consume_shortcut(&close_shortcut)) {
                if let Some(index) = self.store.get_state().active_tab {
                    self.close_tab(index);
                }
            }
            if ctx.input_mut(|i| i.consume_shortcut(&prefs_shortcut)) {
                self.show_preferences_dialog = true;
            }
            if ctx.input_mut(|i| i.consume_shortcut(&exit_shortcut)) {
                tracing::info!("Exiting app via shortcut");
                self.persist_session();
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            if ctx.input_mut(|i| i.consume_shortcut(&logs_shortcut)) {
                self.debug_open = true;
            }
            if ctx.input_mut(|i| i.consume_shortcut(&next_tab_shortcut)) {
                let state = self.store.get_state();
                if !state.open_tabs.is_empty() {
                    if let Some(index) = state.active_tab {
                        let next_index = (index + 1) % state.open_tabs.len();
                        self.activate_tab(next_index);
                    }
                }
            }
            if ctx.input_mut(|i| i.consume_shortcut(&prev_tab_shortcut)) {
                let state = self.store.get_state();
                if !state.open_tabs.is_empty() {
                    if let Some(index) = state.active_tab {
                        let prev_index =
                            (index + state.open_tabs.len() - 1) % state.open_tabs.len();
                        self.activate_tab(prev_index);
                    }
                }
            }

            if self.git_repo.is_some() {
                if ctx.input_mut(|i| i.consume_shortcut(&refresh_shortcut)) {
                    self.refresh_git_data();
                    self.refresh_github_remote_data(ctx.clone());
                }
                if ctx.input_mut(|i| i.consume_shortcut(&fetch_shortcut)) {
                    let ctx_cloned = ctx.clone();
                    self.handle_quick_launch_action(QuickLaunchAction::Fetch, &ctx_cloned);
                }
                if ctx.input_mut(|i| i.consume_shortcut(&pull_shortcut)) {
                    let ctx_cloned = ctx.clone();
                    self.handle_quick_launch_action(QuickLaunchAction::Pull, &ctx_cloned);
                }
                if ctx.input_mut(|i| i.consume_shortcut(&push_shortcut)) {
                    let ctx_cloned = ctx.clone();
                    self.handle_quick_launch_action(QuickLaunchAction::Push, &ctx_cloned);
                }
                if ctx.input_mut(|i| i.consume_shortcut(&save_stash_shortcut)) {
                    self.show_save_stash_dialog = true;
                    self.new_stash_message.clear();
                }
                if ctx.input_mut(|i| i.consume_shortcut(&new_branch_shortcut)) {
                    let ctx_cloned = ctx.clone();
                    self.handle_quick_launch_action(QuickLaunchAction::CreateBranch, &ctx_cloned);
                }
                if ctx.input_mut(|i| i.consume_shortcut(&new_tag_shortcut)) {
                    self.show_create_tag_dialog = true;
                    self.new_tag_name.clear();
                }
                if ctx.input_mut(|i| i.consume_shortcut(&open_explorer_shortcut)) {
                    if let Some(path) = self.store.get_state().current_repo.clone() {
                        open_url(&path);
                    }
                }
                if ctx.input_mut(|i| i.consume_shortcut(&open_console_shortcut)) {
                    if let Some(path) = self.store.get_state().current_repo.clone() {
                        self.open_in_console(&path);
                    }
                }
            }
        }

        if let Some(action) = tabbar::show(ui, &state) {
            match action {
                tabbar::TabAction::Activate(index) => self.activate_tab(index),
                tabbar::TabAction::Close(index) => self.close_tab(index),
                tabbar::TabAction::Open => {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        let path = path.to_string_lossy().to_string();
                        self.open_repo(&path);
                    }
                }
            }
        }

        let show_manager = state.current_repo.is_none() && !state.recent_repos.is_empty();

        let content_rect = ui.available_rect_before_wrap();
        let (content_rect, _) = ui.allocate_exact_size(content_rect.size(), egui::Sense::hover());
        let sidebar_rect = egui::Rect::from_min_size(
            content_rect.left_top(),
            egui::vec2(sidebar::SIDEBAR_WIDTH, content_rect.height()),
        );
        let body_rect = egui::Rect::from_min_max(
            egui::pos2(sidebar_rect.right(), content_rect.top()),
            content_rect.right_bottom(),
        );

        if show_manager {
            if let Some(action) = ui
                .scope_builder(
                    egui::UiBuilder::new()
                        .id_salt("manager_sidebar")
                        .max_rect(sidebar_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        repo_manager_sidebar::show(
                            ui,
                            &mut self.manager_sidebar_state,
                            &state,
                            self.manager_repo_filter,
                        )
                    },
                )
                .inner
            {
                match action {
                    repo_manager_sidebar::ManagerSidebarAction::SelectRepo(path) => {
                        self.store
                            .dispatch(AppAction::SelectManagerRepo(Some(path.clone())));
                        self.fetch_manager_details(&path);
                    }
                    repo_manager_sidebar::ManagerSidebarAction::SetFilter(filter) => {
                        self.manager_repo_filter = filter;
                        tracing::info!(filter = ?self.manager_repo_filter, "Updated manager repo filter");
                        self.persist_session();
                    }
                }
            }

            if let Some(open_path) = ui
                .scope_builder(
                    egui::UiBuilder::new()
                        .id_salt("manager_body")
                        .max_rect(body_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        repo_manager_body::show(
                            ui,
                            &mut self.manager_body_state,
                            &state,
                            match self.manager_repo_filter {
                                repo_manager_sidebar::RepoOwnershipFilter::All => {
                                    RepoOwnershipFilterLabel::All
                                }
                                repo_manager_sidebar::RepoOwnershipFilter::Owned => {
                                    RepoOwnershipFilterLabel::Owned
                                }
                                repo_manager_sidebar::RepoOwnershipFilter::External => {
                                    RepoOwnershipFilterLabel::External
                                }
                            },
                        )
                    },
                )
                .inner
            {
                self.open_repo(&open_path);
            }
        } else {
            let repo_name = self.repo_name();
            let sidebar_action = ui
                .scope_builder(
                    egui::UiBuilder::new()
                        .id_salt("app_sidebar")
                        .max_rect(sidebar_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| {
                        sidebar::show_cached(
                            ui,
                            &mut self.sidebar_state,
                            repo_name.as_deref(),
                            &state,
                        )
                    },
                )
                .inner;

            if let Some(action) = sidebar_action {
                let ctx = ui.ctx().clone();
                match action {
                    sidebar::SidebarAction::CheckoutBranch(name) => {
                        self.handle_branch_action(BranchAction::Checkout(name), &ctx);
                    }
                    sidebar::SidebarAction::DeleteBranch(name) => {
                        self.handle_branch_action(BranchAction::Delete(name), &ctx);
                    }
                    sidebar::SidebarAction::StashApply(index) => {
                        self.handle_stash_action(StashAction::Apply(index), &ctx);
                    }
                    sidebar::SidebarAction::StashPop(index) => {
                        self.handle_stash_action(StashAction::Pop(index), &ctx);
                    }
                    sidebar::SidebarAction::StashDrop(index) => {
                        self.handle_stash_action(StashAction::Drop(index), &ctx);
                    }
                    sidebar::SidebarAction::OpenUrl(url) => {
                        open_url(&url);
                    }
                }
            }
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("app_body")
                    .max_rect(body_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| {
                    body::show_cached(
                        ui,
                        &mut self.body_state,
                        &mut self.commit_panel_state,
                        &state,
                        self.git_repo.as_ref(),
                    )
                },
            );

            if let Some(action) = self.commit_panel_state.pending_action.take() {
                self.handle_commit_action(action);
            }
        }

        if let Some(ref error) = state.repo_error {
            show_error_banner(ui, error);
        }

        if self.show_create_branch_dialog {
            let mut close_dialog = false;
            let mut create_branch = false;

            egui::Window::new("Create New Branch")
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .collapsible(false)
                .resizable(false)
                .default_width(300.0)
                .show(ui.ctx(), |ui| {
                    ui.vertical(|ui| {
                        ui.label("Branch name:");
                        let text_input = ui.text_edit_singleline(&mut self.new_branch_name);
                        text_input.request_focus();

                        if text_input.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))
                        {
                            create_branch = true;
                        }

                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            let create_btn = ui.add_enabled(
                                !self.new_branch_name.trim().is_empty(),
                                egui::Button::new("Create"),
                            );
                            if create_btn.clicked() {
                                create_branch = true;
                            }
                            if ui.button("Cancel").clicked()
                                || ui.input(|i| i.key_pressed(egui::Key::Escape))
                            {
                                close_dialog = true;
                            }
                        });
                    });
                });

            if create_branch {
                let name = self.new_branch_name.trim().to_string();
                if !name.is_empty() {
                    let ctx = ui.ctx().clone();
                    self.handle_branch_action(BranchAction::CreateAndCheckout(name), &ctx);
                }
                close_dialog = true;
            }

            if close_dialog {
                self.show_create_branch_dialog = false;
                self.new_branch_name.clear();
            }
        }

        if self.debug_open {
            show_debug_window(ui.ctx(), &self.log_buffer, &mut self.debug_open);
        }

        if self.show_clone_dialog {
            let mut close_dialog = false;
            let mut start_clone = false;

            let mut is_open = self.show_clone_dialog;
            egui::Window::new("Clone Repository")
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .collapsible(false)
                .resizable(false)
                .default_width(480.0)
                .open(&mut is_open)
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        // Left Column: Logo
                        ui.allocate_ui_with_layout(
                            egui::vec2(80.0, 140.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.add_space(12.0);
                                let logo =
                                    egui::Image::new(egui::include_image!("assets/logo.svg"))
                                        .fit_to_exact_size(egui::vec2(48.0, 48.0));
                                ui.add(logo);
                            },
                        );

                        // Right Column: Form contents
                        ui.allocate_ui_with_layout(
                            egui::vec2(360.0, 140.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.label(egui::RichText::new("Repository URL:").size(12.0));
                                ui.add_sized(
                                    [ui.available_width(), 26.0],
                                    egui::TextEdit::singleline(&mut self.clone_url)
                                        .hint_text("https://github.com/user/repo.git")
                                        .margin(egui::Margin::symmetric(8, 6)),
                                );
                                ui.add_space(8.0);

                                ui.label(egui::RichText::new("Destination Directory:").size(12.0));
                                ui.horizontal(|ui| {
                                    let input_width = ui.available_width() - 80.0;
                                    ui.add_sized(
                                        [input_width, 26.0],
                                        egui::TextEdit::singleline(&mut self.clone_dir)
                                            .hint_text("/path/to/local/directory")
                                            .margin(egui::Margin::symmetric(8, 6)),
                                    );
                                    if ui
                                        .add_sized([72.0, 26.0], egui::Button::new("Browse..."))
                                        .clicked()
                                    {
                                        if let Some(path) = rfd::FileDialog::new().pick_folder() {
                                            self.clone_dir = path.to_string_lossy().to_string();
                                        }
                                    }
                                });

                                ui.add_space(14.0);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let clone_btn = ui.add_enabled(
                                            !self.clone_url.trim().is_empty()
                                                && !self.clone_dir.trim().is_empty(),
                                            egui::Button::new("Clone"),
                                        );
                                        if clone_btn.clicked() {
                                            start_clone = true;
                                        }
                                        if ui.button("Cancel").clicked() {
                                            close_dialog = true;
                                        }
                                    },
                                );
                            },
                        );
                    });

                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close_dialog = true;
                    }
                });

            if start_clone {
                let url = self.clone_url.trim().to_string();
                let path = self.clone_dir.trim().to_string();
                let store = self.store.clone();
                let pending_open = self.pending_open_repo.clone();

                std::thread::spawn(move || {
                    store.dispatch(AppAction::SetRepoError(Some(
                        "Cloning repository...".to_string(),
                    )));
                    match git2::Repository::clone(&url, &path) {
                        Ok(_) => {
                            store.dispatch(AppAction::SetRepoError(None));
                            let mut guard = pending_open.lock().unwrap();
                            *guard = Some(path);
                        }
                        Err(e) => {
                            store.dispatch(AppAction::SetRepoError(Some(format!(
                                "Failed to clone repository: {}",
                                e
                            ))));
                        }
                    }
                });
                close_dialog = true;
            }

            if !is_open || close_dialog {
                self.show_clone_dialog = false;
                self.clone_url.clear();
                self.clone_dir.clear();
            }
        }

        if self.show_create_tag_dialog {
            let mut close_dialog = false;
            let mut create_tag = false;

            let mut is_open = self.show_create_tag_dialog;
            egui::Window::new("Create Tag")
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .collapsible(false)
                .resizable(false)
                .default_width(400.0)
                .open(&mut is_open)
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        // Left Column: Logo
                        ui.allocate_ui_with_layout(
                            egui::vec2(80.0, 100.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.add_space(10.0);
                                let logo =
                                    egui::Image::new(egui::include_image!("assets/logo.svg"))
                                        .fit_to_exact_size(egui::vec2(48.0, 48.0));
                                ui.add(logo);
                            },
                        );

                        // Right Column: Form contents
                        ui.allocate_ui_with_layout(
                            egui::vec2(280.0, 100.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.label(egui::RichText::new("Tag Name:").size(12.0));
                                let text_input = ui.add_sized(
                                    [ui.available_width(), 26.0],
                                    egui::TextEdit::singleline(&mut self.new_tag_name)
                                        .hint_text("v1.0.0")
                                        .margin(egui::Margin::symmetric(8, 6)),
                                );
                                text_input.request_focus();

                                if text_input.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    create_tag = true;
                                }

                                ui.add_space(12.0);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let create_btn = ui.add_enabled(
                                            !self.new_tag_name.trim().is_empty(),
                                            egui::Button::new("Create"),
                                        );
                                        if create_btn.clicked() {
                                            create_tag = true;
                                        }
                                        if ui.button("Cancel").clicked() {
                                            close_dialog = true;
                                        }
                                    },
                                );
                            },
                        );
                    });

                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close_dialog = true;
                    }
                });

            if create_tag {
                let name = self.new_tag_name.trim().to_string();
                if !name.is_empty() {
                    if let Some(repo) = &self.git_repo {
                        match repo.tag_lightweight(&name) {
                            Ok(_) => {
                                self.refresh_git_data();
                            }
                            Err(e) => {
                                self.store.dispatch(AppAction::SetRepoError(Some(format!(
                                    "Failed to create tag: {}",
                                    e
                                ))));
                            }
                        }
                    }
                }
                close_dialog = true;
            }

            if !is_open || close_dialog {
                self.show_create_tag_dialog = false;
                self.new_tag_name.clear();
            }
        }

        if self.show_save_stash_dialog {
            let mut close_dialog = false;
            let mut save_stash = false;

            let mut is_open = self.show_save_stash_dialog;
            egui::Window::new("Save Stash")
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .collapsible(false)
                .resizable(false)
                .default_width(400.0)
                .open(&mut is_open)
                .show(ui.ctx(), |ui| {
                    ui.horizontal(|ui| {
                        // Left Column: Logo
                        ui.allocate_ui_with_layout(
                            egui::vec2(80.0, 100.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.add_space(10.0);
                                let logo =
                                    egui::Image::new(egui::include_image!("assets/logo.svg"))
                                        .fit_to_exact_size(egui::vec2(48.0, 48.0));
                                ui.add(logo);
                            },
                        );

                        // Right Column: Form contents
                        ui.allocate_ui_with_layout(
                            egui::vec2(280.0, 100.0),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| {
                                ui.label(
                                    egui::RichText::new("Stash Message (Optional):").size(12.0),
                                );
                                let text_input = ui.add_sized(
                                    [ui.available_width(), 26.0],
                                    egui::TextEdit::singleline(&mut self.new_stash_message)
                                        .hint_text("WIP on stash")
                                        .margin(egui::Margin::symmetric(8, 6)),
                                );
                                text_input.request_focus();

                                if text_input.lost_focus()
                                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                                {
                                    save_stash = true;
                                }

                                ui.add_space(12.0);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("Save").clicked() {
                                            save_stash = true;
                                        }
                                        if ui.button("Cancel").clicked() {
                                            close_dialog = true;
                                        }
                                    },
                                );
                            },
                        );
                    });

                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        close_dialog = true;
                    }
                });

            if save_stash {
                let message = self.new_stash_message.trim().to_string();
                let msg_opt = if message.is_empty() {
                    None
                } else {
                    Some(message)
                };
                if let Some(repo) = &self.git_repo {
                    match repo.stash_save(msg_opt.as_deref()) {
                        Ok(_) => {
                            self.refresh_git_data();
                        }
                        Err(e) => {
                            self.store.dispatch(AppAction::SetRepoError(Some(format!(
                                "Failed to save stash: {}",
                                e
                            ))));
                        }
                    }
                }
                close_dialog = true;
            }

            if !is_open || close_dialog {
                self.show_save_stash_dialog = false;
                self.new_stash_message.clear();
            }
        }

        if self.show_preferences_dialog {
            let mut close_dialog = false;

            egui::Window::new("Preferences")
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .collapsible(false)
                .resizable(false)
                .default_width(320.0)
                .show(ui.ctx(), |ui| {
                    ui.vertical(|ui| {
                        let state = self.store.get_state();

                        let mut show_buttons = state.show_window_buttons;
                        if ui
                            .checkbox(&mut show_buttons, "Show window buttons")
                            .changed()
                        {
                            self.store
                                .dispatch(AppAction::ToggleWindowButtons(show_buttons));
                            self.persist_session();
                        }

                        ui.add_space(8.0);

                        if let Some(ref identity) = state.git_identity {
                            ui.group(|ui| {
                                ui.label(egui::RichText::new("Git Identity").strong());
                                if let Some(ref name) = identity.name {
                                    ui.label(format!("Name: {}", name));
                                }
                                if let Some(ref email) = identity.email {
                                    ui.label(format!("Email: {}", email));
                                }
                            });
                        }

                        ui.add_space(12.0);
                        if ui.button("Close").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            close_dialog = true;
                        }
                    });
                });

            if close_dialog {
                self.show_preferences_dialog = false;
            }
        }

        if self.show_update_dialog {
            let mut close_dialog = false;

            egui::Window::new("Check for Updates")
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .collapsible(false)
                .resizable(false)
                .default_width(280.0)
                .show(ui.ctx(), |ui| {
                    ui.vertical(|ui| {
                        ui.label(format!("Palimpsest v{}", env!("CARGO_PKG_VERSION")));
                        ui.add_space(8.0);
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Palimpsest is up to date!");
                        });
                        ui.add_space(12.0);
                        if ui.button("OK").clicked()
                            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            close_dialog = true;
                        }
                    });
                });

            if close_dialog {
                self.show_update_dialog = false;
            }
        }
    }
}

impl Drop for PalimpsestApp {
    fn drop(&mut self) {
        let paths: Vec<String> = self.repo_live_trackers.keys().cloned().collect();
        for path in paths {
            self.stop_repo_tracker(&path);
        }
    }
}

impl PalimpsestApp {
    fn open_in_console(&self, path: &str) {
        #[cfg(target_os = "macos")]
        let _ = std::process::Command::new("open")
            .args(["-a", "Terminal", path])
            .spawn();

        #[cfg(target_os = "windows")]
        let _ = std::process::Command::new("cmd")
            .args(["/c", "start", "cmd"])
            .current_dir(path)
            .spawn();

        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            if std::process::Command::new("x-terminal-emulator")
                .current_dir(path)
                .spawn()
                .is_err()
                && std::process::Command::new("gnome-terminal")
                    .arg(format!("--working-directory={}", path))
                    .spawn()
                    .is_err()
            {
                let _ = std::process::Command::new("xterm")
                    .current_dir(path)
                    .spawn();
            }
        }
    }

    fn refresh_github_remote_data(&self, ctx: egui::Context) {
        let state = self.store.get_state();
        if state.github_loading {
            return;
        }

        let creds = palimpsest::auth::credentials::load_credentials();
        let Some(token) = creds.github_token.clone() else {
            return;
        };

        let mut gh_remote = None;
        for remote in &state.cached_remotes {
            if let Some((owner, repo)) = parse_github_remote(&remote.url) {
                gh_remote = Some((owner, repo));
                break;
            }
        }

        let Some((owner, repo)) = gh_remote else {
            // Clear GitHub remote data from state if there is no GitHub remote
            self.store.dispatch(AppAction::SetGitHubData {
                pull_requests: Vec::new(),
                action_runs: Vec::new(),
                releases: Vec::new(),
                packages: Vec::new(),
            });
            return;
        };

        self.store.dispatch(AppAction::SetGitHubLoading(true));

        let store = self.store.clone();
        std::thread::spawn(move || {
            tracing::info!(owner = %owner, repo = %repo, "Fetching GitHub remote data in background");

            let pulls = palimpsest::auth::github_api::list_pull_requests(&token, &owner, &repo);
            let actions = palimpsest::auth::github_api::list_action_runs(&token, &owner, &repo);
            let releases = palimpsest::auth::github_api::list_releases(&token, &owner, &repo);

            let is_org =
                match palimpsest::auth::github_api::get_repo_owner_type(&token, &owner, &repo) {
                    Ok(owner_type) => owner_type.to_lowercase() == "organization",
                    Err(_) => false,
                };

            let packages = palimpsest::auth::github_api::list_packages(&token, &owner, is_org);

            let mut errors = Vec::new();

            let pr_list = match pulls {
                Ok(p) => p,
                Err(e) => {
                    errors.push(format!("PRs: {}", e));
                    Vec::new()
                }
            };

            let action_list = match actions {
                Ok(a) => a,
                Err(e) => {
                    errors.push(format!("Actions: {}", e));
                    Vec::new()
                }
            };

            let release_list = match releases {
                Ok(r) => r,
                Err(e) => {
                    errors.push(format!("Releases: {}", e));
                    Vec::new()
                }
            };

            let package_list = match packages {
                Ok(pkg) => pkg,
                Err(e) => {
                    errors.push(format!("Packages: {}", e));
                    Vec::new()
                }
            };

            let mapped_prs = pr_list
                .into_iter()
                .map(|pr| palimpsest::state::GitHubPullRequest {
                    number: pr.number,
                    title: pr.title,
                    state: pr.state,
                    user_login: pr.user_login,
                    html_url: pr.html_url,
                    head_ref: pr.head_ref,
                    base_ref: pr.base_ref,
                    draft: pr.draft,
                })
                .collect::<Vec<_>>();

            let mapped_actions = action_list
                .into_iter()
                .map(|run| palimpsest::state::GitHubActionRun {
                    id: run.id,
                    name: run.name,
                    status: run.status,
                    conclusion: run.conclusion,
                    html_url: run.html_url,
                    head_branch: run.head_branch,
                })
                .collect::<Vec<_>>();

            let mapped_releases = release_list
                .into_iter()
                .map(|rel| palimpsest::state::GitHubRelease {
                    tag_name: rel.tag_name,
                    name: rel.name,
                    html_url: rel.html_url,
                    draft: rel.draft,
                    prerelease: rel.prerelease,
                    body: rel.body,
                })
                .collect::<Vec<_>>();

            let mapped_packages = package_list
                .into_iter()
                .map(|pkg| palimpsest::state::GitHubPackage {
                    name: pkg.name,
                    package_type: pkg.package_type,
                    html_url: pkg.html_url,
                })
                .collect::<Vec<_>>();

            store.dispatch(AppAction::SetGitHubData {
                pull_requests: mapped_prs,
                action_runs: mapped_actions,
                releases: mapped_releases,
                packages: mapped_packages,
            });

            if !errors.is_empty() {
                store.dispatch(AppAction::SetGitHubError(Some(errors.join(", "))));
            } else {
                store.dispatch(AppAction::SetGitHubError(None));
            }

            store.dispatch(AppAction::SetGitHubLoading(false));
            ctx.request_repaint();
        });
    }
}

fn open_url(url: &str) {
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/c", "start", "", url])
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

fn parse_github_remote(url: &str) -> Option<(String, String)> {
    let s = url.trim();
    if s.contains("github.com") {
        let path = if s.starts_with("ssh://git@") {
            s.split("github.com/")
                .nth(1)?
                .split('/')
                .collect::<Vec<_>>()
        } else if s.starts_with("git@") {
            s.split("github.com:")
                .nth(1)?
                .split('/')
                .collect::<Vec<_>>()
        } else {
            s.split("github.com/")
                .nth(1)?
                .split('/')
                .collect::<Vec<_>>()
        };
        if path.len() >= 2 {
            let owner = path[0].to_string();
            let mut repo = path[1].to_string();
            if repo.ends_with(".git") {
                repo = repo[..repo.len() - 4].to_string();
            }
            return Some((owner, repo));
        }
    }
    None
}

fn show_error_banner(ui: &egui::Ui, error: &str) {
    let rect = ui.available_rect_before_wrap();
    let banner_height = 40.0;
    let banner_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.bottom() - banner_height),
        egui::vec2(rect.width(), banner_height),
    );
    let bg = egui::Color32::from_rgb(60, 20, 20);
    ui.painter().rect_filled(banner_rect, 0.0, bg);
    ui.painter().text(
        banner_rect.center(),
        egui::Align2::CENTER_CENTER,
        error,
        egui::FontId::proportional(12.0),
        egui::Color32::LIGHT_RED,
    );
}

fn show_debug_window(ctx: &egui::Context, log_buffer: &LogBuffer, open: &mut bool) {
    egui::Window::new("Debug Log")
        .open(open)
        .default_size([800.0, 400.0])
        .resizable(true)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Logs");
                if ui.button("Clear").clicked() {
                    log_buffer.clear();
                }
            });
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false, true])
                .stick_to_bottom(true)
                .show(ui, |ui| {
                    for entry in log_buffer.entries() {
                        let color = match entry.level.as_str() {
                            "ERROR" => egui::Color32::RED,
                            "WARN " => egui::Color32::YELLOW,
                            "INFO " => egui::Color32::GREEN,
                            "DEBUG" => egui::Color32::BLUE,
                            "TRACE" => egui::Color32::GRAY,
                            _ => egui::Color32::WHITE,
                        };
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(&entry.timestamp)
                                    .color(egui::Color32::GRAY)
                                    .size(11.0),
                            );
                            ui.label(
                                egui::RichText::new(&entry.level)
                                    .color(color)
                                    .size(11.0)
                                    .monospace(),
                            );
                            ui.label(egui::RichText::new(&entry.message).size(11.0));
                        });
                    }
                });
        });
}
