use eframe::egui;
use std::sync::Arc;

use palimpsest::git::GitRepo;
use palimpsest::logger::LogBuffer;
use palimpsest::state::{AppAction, AppStore};
use palimpsest::ui::{body, commit_panel, sidebar, tabbar, titlebar, toolbar};

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
    git_repo: Option<GitRepo>,
    body_state: body::State,
    commit_panel_state: commit_panel::State,
}

impl PalimpsestApp {
    fn new(cc: &eframe::CreationContext<'_>, log_buffer: Arc<LogBuffer>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let store = palimpsest::state::create_store();

        tracing::info!("Application initialized");

        Self {
            store,
            log_buffer,
            titlebar_menu_open: false,
            search_query: String::new(),
            debug_open: false,
            git_repo: None,
            body_state: body::State::default(),
            commit_panel_state: commit_panel::State::default(),
        }
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

    fn open_repo(&mut self, path: &str) {
        tracing::info!(repo = %path, "Opening repository");
        match GitRepo::open(path) {
            Ok(repo) => {
                tracing::info!(repo_name = ?repo.repo_name(), "Repository opened successfully");
                self.git_repo = Some(repo);
                self.store.dispatch(AppAction::OpenRepo(path.to_string()));
                self.refresh_git_data();
            }
            Err(e) => {
                tracing::error!(error = %e, "Failed to open repository");
                self.git_repo = None;
                self.store
                    .dispatch(AppAction::SetRepoError(Some(e.to_string())));
            }
        }
    }

    fn refresh_git_data(&mut self) {
        if let Some(repo) = &self.git_repo {
            let commits = repo.commits(200).unwrap_or_default();
            let branches = repo.branches().unwrap_or_default();
            let remotes = repo.remotes().unwrap_or_default();
            let tags = repo.tags().unwrap_or_default();
            let status = repo
                .status()
                .unwrap_or(palimpsest::git::models::RepoStatus {
                    branch: "HEAD".to_string(),
                    staged_count: 0,
                    unstaged_count: 0,
                    staged_files: Vec::new(),
                    additions: 0,
                    deletions: 0,
                    files_changed: 0,
                });

            self.store.dispatch(AppAction::RefreshGitData {
                commits,
                branches,
                remotes,
                tags,
                status,
            });
            self.store.dispatch(AppAction::SetRepoError(None));
        }
    }
}

impl eframe::App for PalimpsestApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let background = ui.visuals().widgets.inactive.bg_fill;
        ui.painter().rect_filled(ui.max_rect(), 0.0, background);

        let repo_name = self.repo_name();
        let repo_name_ref = repo_name.as_deref();
        let mut show_window_buttons = self.store.get_state().show_window_buttons;

        let open_repo = titlebar::show(
            ui,
            frame,
            &mut self.titlebar_menu_open,
            &mut self.search_query,
            repo_name_ref,
            &self.store.get_state().recent_repos,
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

        if self.git_repo.is_some() && self.store.get_state().needs_refresh() {
            self.refresh_git_data();
        }

        let state = self.store.get_state();
        let repo_name = self.repo_name();
        toolbar::show(ui, repo_name.as_deref(), self.git_repo.as_ref());
        tabbar::show(ui, repo_name.as_deref());

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

        if let Some(ref error) = state.repo_error {
            show_error_banner(ui, error);
        }

        if self.debug_open {
            show_debug_window(ui.ctx(), &self.log_buffer, &mut self.debug_open);
        }
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
