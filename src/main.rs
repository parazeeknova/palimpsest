use eframe::egui;
use std::sync::Arc;

use palimpsest::git::GitRepo;
use palimpsest::logger::LogBuffer;
use palimpsest::state::{AppAction, AppStore, CommitAction};
use palimpsest::ui::command_palette::{PaletteResult, QuickLaunchAction};
use palimpsest::ui::repo_manager_sidebar;
use palimpsest::ui::{
    body, command_palette, commit_panel, repo_manager_body, sidebar, tabbar, titlebar, toolbar,
};

fn main() -> eframe::Result {
    let log_buffer = Arc::new(LogBuffer::new(1000));
    palimpsest::logger::init(log_buffer.clone());

    tracing::info!("Palimpsest starting up");

    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Palimpsest")
            .with_inner_size([960.0, 720.0]),
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
}

impl PalimpsestApp {
    fn new(cc: &eframe::CreationContext<'_>, log_buffer: Arc<LogBuffer>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let session = palimpsest::state::AppSession::load();
        let store = palimpsest::state::create_store(session);

        tracing::info!("Application initialized");

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
        palimpsest::state::AppSession::from_state(&self.store.get_state()).save();
    }

    fn restore_active_repo_from_state(&mut self) {
        let Some(path) = self.store.get_state().current_repo.clone() else {
            return;
        };

        if self.git_repo.is_none() {
            self.open_repo(&path);
        }
    }

    fn open_repo(&mut self, path: &str) {
        tracing::info!(repo = %path, "Opening repository");
        match GitRepo::open(path) {
            Ok(repo) => {
                tracing::info!(repo_name = ?repo.repo_name(), "Repository opened successfully");
                self.git_repo = Some(repo);
                self.store.dispatch(AppAction::OpenRepo(path.to_string()));
                self.refresh_git_data();
                self.persist_session();
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to open repository");
                self.store
                    .dispatch(AppAction::SetRepoError(Some(e.to_string())));
            }
        }
    }

    fn activate_tab(&mut self, index: usize) {
        self.store.dispatch(AppAction::ActivateTab(index));
        if let Some(path) = self.store.get_state().current_repo.clone() {
            self.git_repo = None;
            self.open_repo(&path);
            self.persist_session();
        }
    }

    fn close_tab(&mut self, index: usize) {
        self.store.dispatch(AppAction::CloseTab(index));
        self.git_repo = None;
        if let Some(path) = self.store.get_state().current_repo.clone() {
            self.open_repo(&path);
        } else {
            self.persist_session();
        }
    }

    fn refresh_git_data(&mut self) {
        if let Some(repo) = &self.git_repo {
            let mut errors = Vec::new();

            let commits = match repo.commits(200) {
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

            self.store.dispatch(AppAction::RefreshGitData {
                commits,
                branches,
                remotes,
                tags,
                status,
            });

            if errors.is_empty() {
                self.store.dispatch(AppAction::SetRepoError(None));
            } else {
                self.store
                    .dispatch(AppAction::SetRepoError(Some(errors.join("; "))));
            }
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
        }
    }

    fn handle_quick_launch_action(&mut self, action: QuickLaunchAction) {
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
                std::process::exit(0);
            }
            QuickLaunchAction::OpenLogs => {
                self.debug_open = true;
            }
            QuickLaunchAction::Fetch => {
                if let Some(repo) = &self.git_repo {
                    match repo.fetch() {
                        Ok(()) => self.refresh_git_data(),
                        Err(e) => tracing::error!(error = %e, "Fetch failed"),
                    }
                }
            }
            QuickLaunchAction::Pull => {
                if let Some(repo) = &self.git_repo {
                    match repo.pull() {
                        Ok(()) => self.refresh_git_data(),
                        Err(e) => tracing::error!(error = %e, "Pull failed"),
                    }
                }
            }
            QuickLaunchAction::Push => {
                if let Some(repo) = &self.git_repo {
                    match repo.push() {
                        Ok(()) => self.refresh_git_data(),
                        Err(e) => tracing::error!(error = %e, "Push failed"),
                    }
                }
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
                tracing::info!("Create branch requested (not yet implemented)");
            }
        }
    }

    fn fetch_manager_details(&mut self, path: &str) {
        use palimpsest::state::{
            ManagerBranch, ManagerCommit, ManagerRemote, ManagerRepoDetails, ManagerTag,
        };
        use palimpsest::ui::repo_manager::format_relative_time;

        match GitRepo::open(path) {
            Ok(repo) => {
                let repo_name = repo.repo_name().unwrap_or_else(|| path.to_string());
                let branch = repo.head_branch().unwrap_or_else(|_| "HEAD".to_string());

                let status = repo.status().ok();
                let uncommitted = status
                    .as_ref()
                    .map_or(0, |s| s.staged_count + s.unstaged_count);

                let commits = repo.commits(20).unwrap_or_default();
                let total_commits = commits.len();

                let initial_date = commits.last().map_or("unknown".to_string(), |c| {
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

                let remotes = repo.remotes().unwrap_or_default();
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

                let tags = repo.tags().unwrap_or_default();
                let manager_tags: Vec<ManagerTag> = tags
                    .iter()
                    .take(5)
                    .map(|t| {
                        let target_commit = commits.iter().find(|c| {
                            c.hash.starts_with(&t.target_hash) || c.short_hash == t.target_hash
                        });
                        ManagerTag {
                            name: t.name.clone(),
                            author: target_commit.map(|c| c.author.clone()).unwrap_or_default(),
                            relative_date: target_commit
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
                };

                self.store
                    .dispatch(AppAction::SetManagerDetails(Some(details)));
            }
            Err(e) => {
                tracing::warn!(path = %path, error = %e, "Repo not found, removing from recents");
                self.store
                    .dispatch(AppAction::RemoveRecentRepo(path.to_string()));
                self.store.dispatch(AppAction::SelectManagerRepo(None));
            }
        }
    }
}

impl eframe::App for PalimpsestApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let background = ui.visuals().widgets.inactive.bg_fill;
        ui.painter().rect_filled(ui.max_rect(), 0.0, background);

        let state = self.store.get_state();
        let repo_name = self.repo_name();
        let repo_name_ref = repo_name.as_deref();
        let mut show_window_buttons = self.store.get_state().show_window_buttons;

        let open_repo = titlebar::show(
            ui,
            frame,
            &mut self.titlebar_menu_open,
            &mut self.search_query,
            repo_name_ref,
            &state.recent_repos,
            &mut show_window_buttons,
            &mut self.debug_open,
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
                if let Some(path) = self.store.get_state().recent_repos.get(index).cloned() {
                    tracing::info!(repo = %path, "Selecting recent repository");
                    self.open_repo(&path);
                }
            }
            titlebar::OpenAction::None => {}
        }

        let state = self.store.get_state();
        let repo_name = self.repo_name();
        let current_branch = state.cached_status.as_ref().map(|s| s.branch.as_str());
        let quick_launch_clicked = toolbar::show(ui, repo_name.as_deref(), current_branch);

        if quick_launch_clicked || command_palette::check_shortcut(ui.ctx()) {
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
                    self.handle_quick_launch_action(a);
                }
                PaletteResult::Closed => {
                    self.show_command_palette = false;
                }
                PaletteResult::StillOpen => {}
            }
        }

        if !self.show_command_palette {
            let ctx = ui.ctx();
            let open_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::O);
            let exit_shortcut = egui::KeyboardShortcut::new(egui::Modifiers::COMMAND, egui::Key::Q);
            let logs_shortcut = egui::KeyboardShortcut::new(
                egui::Modifiers::COMMAND.plus(egui::Modifiers::SHIFT),
                egui::Key::L,
            );

            if ctx.input_mut(|i| i.consume_shortcut(&open_shortcut)) {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    let path = path.to_string_lossy().to_string();
                    self.open_repo(&path);
                }
            }
            if ctx.input_mut(|i| i.consume_shortcut(&exit_shortcut)) {
                tracing::info!("Exiting app via shortcut");
                std::process::exit(0);
            }
            if ctx.input_mut(|i| i.consume_shortcut(&logs_shortcut)) {
                self.debug_open = true;
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
                    |ui| repo_manager_sidebar::show(ui, &state),
                )
                .inner
            {
                match action {
                    repo_manager_sidebar::ManagerSidebarAction::SelectRepo(path) => {
                        self.store
                            .dispatch(AppAction::SelectManagerRepo(Some(path.clone())));
                        self.fetch_manager_details(&path);
                    }
                }
            }

            if let Some(open_path) = ui
                .scope_builder(
                    egui::UiBuilder::new()
                        .id_salt("manager_body")
                        .max_rect(body_rect)
                        .layout(egui::Layout::top_down(egui::Align::Min)),
                    |ui| repo_manager_body::show(ui, &mut self.manager_body_state, &state),
                )
                .inner
            {
                self.open_repo(&open_path);
            }
        } else {
            let repo_name = self.repo_name();
            ui.scope_builder(
                egui::UiBuilder::new()
                    .id_salt("app_sidebar")
                    .max_rect(sidebar_rect)
                    .layout(egui::Layout::top_down(egui::Align::Min)),
                |ui| sidebar::show_cached(ui, repo_name.as_deref(), &state),
            );
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

        if self.debug_open {
            show_debug_window(ui.ctx(), &self.log_buffer, &mut self.debug_open);
        }
    }
}

impl Drop for PalimpsestApp {
    fn drop(&mut self) {
        self.persist_session();
    }
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
