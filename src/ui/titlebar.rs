use eframe::egui;
use egui_phosphor::regular::{
    ARROW_CLOCKWISE, ARROW_DOWN, ARROW_LEFT, ARROW_RIGHT, ARROW_UP, ARROW_UP_RIGHT, BROWSERS,
    CHART_BAR, CLOCK, CLOUD_ARROW_DOWN, DATABASE, FOLDER, FOLDER_OPEN, FOLDER_PLUS, GEAR_SIX,
    GIT_BRANCH, GIT_FORK, GIT_PULL_REQUEST, GITHUB_LOGO, GLOBE_SIMPLE, GRID_FOUR, KEY, LIST,
    MAGNIFYING_GLASS, MINUS, PLUS, POWER, SCISSORS, SQUARE, STACK, TAG, TERMINAL_WINDOW, TIMER,
    USER_CIRCLE, WRENCH, X,
};

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

pub enum OpenAction {
    None,
    PickFolder,
    SelectRecent(usize),
    InitRepo,
    CloneRepo,
    NewTab,
    QuickLaunch,
    CloseTab,
    ConfigureSsh,
    Accounts,
    CheckUpdates,
    Preferences,
    Exit,
    Refresh,
    Fetch,
    Pull,
    Push,
    SaveStash,
    NewBranch,
    NewTag,
    NewWorktree,
    GitFlow,
    GitLfs,
    ApplyPatch,
    Bisect,
    OpenInFileExplorer,
    OpenInConsole,
    RepositoryStatistics,
    RepositoryTreemap,
    PerformanceBenchmark,
    RepositorySettings,
    NextTab,
    PrevTab,
}

use crate::state::RecentRepo;
use crate::ui::repo_manager::format_relative_time;

use super::profile_panel;

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    _frame: &mut eframe::Frame,
    menu_open: &mut bool,
    search_query: &mut String,
    repo_name: Option<&str>,
    recent_repos: &[RecentRepo],
    show_window_buttons: &mut bool,
    debug_open: &mut bool,
    profile_panel_state: &mut profile_panel::ProfilePanelState,
    github_user: Option<&crate::state::GitHubUserProfile>,
    git_identity: Option<&crate::state::CachedGitIdentity>,
    auth_status: &crate::state::AuthStatus,
) -> (OpenAction, profile_panel::ProfileAction) {
    let mut action = OpenAction::None;
    let mut profile_action = profile_panel::ProfileAction::None;
    let available_width = ui.available_width();
    let height = 28.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(available_width, height),
        egui::Sense::click_and_drag(),
    );

    let visuals = ui.visuals().widgets.inactive;
    ui.painter().rect_filled(rect, 0.0, visuals.bg_fill);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], visuals.bg_stroke);

    if response.dragged() {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
    }

    let logo = egui::Image::new(egui::include_image!("../assets/logo.svg"))
        .fit_to_exact_size(egui::vec2(16.0, 16.0));

    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(egui::vec2(8.0, 2.0)))
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
            ui.spacing_mut().button_padding = egui::vec2(5.0, 1.0);

            ui.add(logo);

            if ui
                .add(egui::Button::new(egui::RichText::new(LIST).size(12.0)).frame(false))
                .clicked()
            {
                *menu_open = !*menu_open;
            }

            ui.label(egui::RichText::new(ARROW_LEFT).size(12.0));
            ui.label(egui::RichText::new(ARROW_RIGHT).size(12.0));

            if *menu_open {
                ui.horizontal(|ui| {
                    let file_resp = ui.menu_button(egui::RichText::new("File").size(12.0), |ui| {
                        ui.set_max_width(240.0);

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!(
                                        "{}  Init New Repository...",
                                        FOLDER_PLUS
                                    ))
                                    .size(12.0),
                                )
                                .shortcut_text("Ctrl+Shift+N")
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = OpenAction::InitRepo;
                            ui.close();
                        }

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("{}  Clone...", CLOUD_ARROW_DOWN))
                                        .size(12.0),
                                )
                                .shortcut_text("Ctrl+N")
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = OpenAction::CloneRepo;
                            ui.close();
                        }

                        ui.separator();

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("{}  New Tab", PLUS)).size(12.0),
                                )
                                .shortcut_text("Ctrl+T")
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = OpenAction::NewTab;
                            ui.close();
                        }

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!(
                                        "{}  Open Repository...",
                                        FOLDER_OPEN
                                    ))
                                    .size(12.0),
                                )
                                .shortcut_text("Ctrl+O")
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = OpenAction::PickFolder;
                            ui.close();
                        }

                        if !recent_repos.is_empty() {
                            ui.menu_button(
                                egui::RichText::new(format!("{}  Recents", CLOCK)).size(12.0),
                                |ui| {
                                    ui.set_max_width(220.0);
                                    ui.label(
                                        egui::RichText::new("Recent repositories")
                                            .size(11.0)
                                            .color(egui::Color32::from_rgb(140, 140, 140)),
                                    );
                                    ui.separator();
                                    for (i, repo) in recent_repos.iter().enumerate() {
                                        let name = repo_display_name(&repo.path);
                                        let time_ago =
                                            format_relative_time(repo.last_opened as i64);
                                        if ui
                                            .button(
                                                egui::RichText::new(format!(
                                                    "{}  {}  {}",
                                                    FOLDER, name, time_ago
                                                ))
                                                .size(12.0),
                                            )
                                            .clicked()
                                        {
                                            action = OpenAction::SelectRecent(i);
                                            ui.close();
                                        }
                                    }
                                },
                            );
                        }

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!(
                                        "{}  Quick Launch...",
                                        MAGNIFYING_GLASS
                                    ))
                                    .size(12.0),
                                )
                                .shortcut_text("Ctrl+P")
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = OpenAction::QuickLaunch;
                            ui.close();
                        }

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("{}  Close Tab", X)).size(12.0),
                                )
                                .shortcut_text("Ctrl+W")
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = OpenAction::CloseTab;
                            ui.close();
                        }

                        ui.separator();

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("{}  Configure SSH Keys...", KEY))
                                        .size(12.0),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = OpenAction::ConfigureSsh;
                            ui.close();
                        }

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("{}  Accounts...", USER_CIRCLE))
                                        .size(12.0),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = OpenAction::Accounts;
                            ui.close();
                        }

                        ui.separator();

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!(
                                        "{}  Check for Updates...",
                                        ARROW_CLOCKWISE
                                    ))
                                    .size(12.0),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = OpenAction::CheckUpdates;
                            ui.close();
                        }

                        ui.separator();

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("{}  Preferences...", GEAR_SIX))
                                        .size(12.0),
                                )
                                .shortcut_text("Ctrl+,")
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = OpenAction::Preferences;
                            ui.close();
                        }

                        ui.separator();

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(format!("{}  Exit", POWER)).size(12.0),
                                )
                                .frame(false),
                            )
                            .clicked()
                        {
                            action = OpenAction::Exit;
                            ui.close();
                        }
                    });
                    if file_resp.response.hovered() || file_resp.inner.is_some() {
                        ui.painter().rect_filled(
                            file_resp.response.rect,
                            2.0,
                            egui::Color32::from_white_alpha(30),
                        );
                    }

                    if repo_name.is_some() {
                        let repo_resp =
                            ui.menu_button(egui::RichText::new("Repository").size(12.0), |ui| {
                                ui.set_max_width(260.0);

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Refresh",
                                                ARROW_CLOCKWISE
                                            ))
                                            .size(12.0),
                                        )
                                        .shortcut_text("F5")
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::Refresh;
                                    ui.close();
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Fetch...",
                                                ARROW_DOWN
                                            ))
                                            .size(12.0),
                                        )
                                        .shortcut_text("Ctrl+Shift+F")
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::Fetch;
                                    ui.close();
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Pull...",
                                                GIT_PULL_REQUEST
                                            ))
                                            .size(12.0),
                                        )
                                        .shortcut_text("Ctrl+Shift+U")
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::Pull;
                                    ui.close();
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!("{}  Push...", ARROW_UP))
                                                .size(12.0),
                                        )
                                        .shortcut_text("Ctrl+Shift+P")
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::Push;
                                    ui.close();
                                }

                                ui.separator();

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Save Stash...",
                                                STACK
                                            ))
                                            .size(12.0),
                                        )
                                        .shortcut_text("Ctrl+Shift+H")
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::SaveStash;
                                    ui.close();
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  New Branch...",
                                                GIT_BRANCH
                                            ))
                                            .size(12.0),
                                        )
                                        .shortcut_text("Ctrl+Shift+B")
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::NewBranch;
                                    ui.close();
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!("{}  New Tag...", TAG))
                                                .size(12.0),
                                        )
                                        .shortcut_text("Ctrl+Shift+T")
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::NewTag;
                                    ui.close();
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  New Worktree...",
                                                BROWSERS
                                            ))
                                            .size(12.0),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::NewWorktree;
                                    ui.close();
                                }

                                ui.separator();

                                ui.menu_button(
                                    egui::RichText::new(format!("{}  Git Flow", GIT_FORK))
                                        .size(12.0),
                                    |ui| {
                                        ui.set_max_width(120.0);
                                        if ui
                                            .button(egui::RichText::new("Initialize").size(12.0))
                                            .clicked()
                                        {
                                            action = OpenAction::GitFlow;
                                            ui.close();
                                        }
                                        if ui
                                            .button(egui::RichText::new("Feature").size(12.0))
                                            .clicked()
                                        {
                                            action = OpenAction::GitFlow;
                                            ui.close();
                                        }
                                        if ui
                                            .button(egui::RichText::new("Release").size(12.0))
                                            .clicked()
                                        {
                                            action = OpenAction::GitFlow;
                                            ui.close();
                                        }
                                        if ui
                                            .button(egui::RichText::new("Hotfix").size(12.0))
                                            .clicked()
                                        {
                                            action = OpenAction::GitFlow;
                                            ui.close();
                                        }
                                    },
                                );

                                ui.menu_button(
                                    egui::RichText::new(format!("{}  Git LFS", DATABASE))
                                        .size(12.0),
                                    |ui| {
                                        ui.set_max_width(160.0);
                                        if ui
                                            .button(egui::RichText::new("Track...").size(12.0))
                                            .clicked()
                                        {
                                            action = OpenAction::GitLfs;
                                            ui.close();
                                        }
                                        if ui
                                            .button(egui::RichText::new("Untrack...").size(12.0))
                                            .clicked()
                                        {
                                            action = OpenAction::GitLfs;
                                            ui.close();
                                        }
                                        if ui
                                            .button(
                                                egui::RichText::new("List Tracked Files")
                                                    .size(12.0),
                                            )
                                            .clicked()
                                        {
                                            action = OpenAction::GitLfs;
                                            ui.close();
                                        }
                                        if ui
                                            .button(egui::RichText::new("Status").size(12.0))
                                            .clicked()
                                        {
                                            action = OpenAction::GitLfs;
                                            ui.close();
                                        }
                                    },
                                );

                                ui.separator();

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Apply Patch...",
                                                WRENCH
                                            ))
                                            .size(12.0),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::ApplyPatch;
                                    ui.close();
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!("{}  Bisect", SCISSORS))
                                                .size(12.0),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::Bisect;
                                    ui.close();
                                }

                                ui.separator();

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Open In File Explorer",
                                                FOLDER
                                            ))
                                            .size(12.0),
                                        )
                                        .shortcut_text("Ctrl+Alt+O")
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::OpenInFileExplorer;
                                    ui.close();
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Open In Console",
                                                TERMINAL_WINDOW
                                            ))
                                            .size(12.0),
                                        )
                                        .shortcut_text("Ctrl+Alt+T")
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::OpenInConsole;
                                    ui.close();
                                }

                                ui.separator();

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Repository Statistics...",
                                                CHART_BAR
                                            ))
                                            .size(12.0),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::RepositoryStatistics;
                                    ui.close();
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Repository Treemap...",
                                                GRID_FOUR
                                            ))
                                            .size(12.0),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::RepositoryTreemap;
                                    ui.close();
                                }

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Performance Benchmark...",
                                                TIMER
                                            ))
                                            .size(12.0),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::PerformanceBenchmark;
                                    ui.close();
                                }

                                ui.separator();

                                if ui
                                    .add(
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "{}  Settings for This Repository...",
                                                GEAR_SIX
                                            ))
                                            .size(12.0),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    action = OpenAction::RepositorySettings;
                                    ui.close();
                                }
                            });
                        if repo_resp.response.hovered() || repo_resp.inner.is_some() {
                            ui.painter().rect_filled(
                                repo_resp.response.rect,
                                2.0,
                                egui::Color32::from_white_alpha(30),
                            );
                        }
                    }

                    let view_resp = ui.menu_button(egui::RichText::new("View").size(12.0), |ui| {
                        ui.set_max_width(300.0);

                        let item = |ui: &mut egui::Ui, label: &str, shortcut: Option<&str>| {
                            let response = ui.add_enabled(
                                false,
                                egui::Button::new(egui::RichText::new(label).size(12.0)),
                            );
                            if let Some(shortcut) = shortcut {
                                ui.painter().text(
                                    egui::pos2(
                                        response.rect.right() - 6.0,
                                        response.rect.center().y,
                                    ),
                                    egui::Align2::RIGHT_CENTER,
                                    shortcut,
                                    egui::FontId::proportional(11.0),
                                    egui::Color32::from_rgb(150, 150, 150),
                                );
                            }
                        };

                        item(ui, "Show Uncommitted Changes", Some("Ctrl+1"));
                        item(ui, "Show All Commits", Some("Ctrl+2"));
                        ui.separator();
                        item(ui, "Hide Commit Details", Some("Ctrl+Shift+D"));
                        item(ui, "Show HEAD", Some("Ctrl+0"));
                        ui.separator();
                        item(ui, "Hide Tags", None);
                        item(ui, "Hide Stashes in Commit List", None);
                        item(ui, "Show Lost Commits (Reflog)", Some("Ctrl+Shift+."));
                        item(ui, "Collapse All Merges (Show First Parent)", None);
                        item(ui, "Filter by Active Branch", Some("Ctrl+Shift+A"));
                    });
                    if view_resp.response.hovered() || view_resp.inner.is_some() {
                        ui.painter().rect_filled(
                            view_resp.response.rect,
                            2.0,
                            egui::Color32::from_white_alpha(30),
                        );
                    }

                    let window_resp =
                        ui.menu_button(egui::RichText::new("Window").size(12.0), |ui| {
                            ui.set_max_width(220.0);
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(format!(
                                            "{}  Select Next Tab",
                                            ARROW_RIGHT
                                        ))
                                        .size(12.0),
                                    )
                                    .shortcut_text("Ctrl+Tab")
                                    .frame(false),
                                )
                                .clicked()
                            {
                                action = OpenAction::NextTab;
                                ui.close();
                            }

                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(format!(
                                            "{}  Select Previous Tab",
                                            ARROW_LEFT
                                        ))
                                        .size(12.0),
                                    )
                                    .shortcut_text("Ctrl+Shift+Tab")
                                    .frame(false),
                                )
                                .clicked()
                            {
                                action = OpenAction::PrevTab;
                                ui.close();
                            }

                            ui.separator();

                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new(TERMINAL_WINDOW).size(12.0));
                                ui.label("Show window buttons");
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.checkbox(show_window_buttons, "");
                                    },
                                );
                            });
                        });
                    if window_resp.response.hovered() || window_resp.inner.is_some() {
                        ui.painter().rect_filled(
                            window_resp.response.rect,
                            2.0,
                            egui::Color32::from_white_alpha(30),
                        );
                    }

                    let debug_resp =
                        ui.menu_button(egui::RichText::new("Debug").size(12.0), |ui| {
                            ui.set_max_width(200.0);
                            if ui
                                .button(
                                    egui::RichText::new(format!("{}  Open Logs", TERMINAL_WINDOW))
                                        .size(12.0),
                                )
                                .clicked()
                            {
                                *debug_open = true;
                                ui.close();
                            }
                        });
                    if debug_resp.response.hovered() || debug_resp.inner.is_some() {
                        ui.painter().rect_filled(
                            debug_resp.response.rect,
                            2.0,
                            egui::Color32::from_white_alpha(30),
                        );
                    }

                    let help_resp = ui.menu_button(egui::RichText::new("Help").size(12.0), |ui| {
                        ui.set_max_width(220.0);
                        let github_resp = ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(GITHUB_LOGO).size(12.0));
                            ui.label(egui::RichText::new("Visit GitHub").size(12.0));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(egui::RichText::new(ARROW_UP_RIGHT).size(10.0));
                                },
                            );
                        });
                        if github_resp.response.clicked() {
                            open_url("https://github.com/parazeeknova/palimpsest");
                            ui.close();
                        }

                        let author_resp = ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(GLOBE_SIMPLE).size(12.0));
                            ui.label(egui::RichText::new("Visit Author").size(12.0));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(egui::RichText::new(ARROW_UP_RIGHT).size(10.0));
                                },
                            );
                        });
                        if author_resp.response.clicked() {
                            open_url("https://przknv.cc");
                            ui.close();
                        }
                    });
                    if help_resp.response.hovered() || help_resp.inner.is_some() {
                        ui.painter().rect_filled(
                            help_resp.response.rect,
                            2.0,
                            egui::Color32::from_white_alpha(30),
                        );
                    }
                });
            }

            let search_width = (rect.width() * 0.25).clamp(140.0, 240.0);
            let group_width = search_width + 60.0;
            let spacer = ((ui.available_width() - group_width).max(0.0)) * 0.5;
            ui.add_space(spacer);

            let hint = match repo_name {
                Some(name) => format!("Search anything in {}...", name),
                None => "Search anything...".to_string(),
            };

            ui.allocate_ui_with_layout(
                egui::vec2(group_width, 20.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
                    ui.add_sized(
                        [14.0, 20.0],
                        egui::Label::new(egui::RichText::new(MAGNIFYING_GLASS).size(12.0)),
                    );
                    let bg_fill = visuals.bg_fill;
                    let edit_response = ui.add_sized(
                        [search_width - 20.0, 18.0],
                        egui::TextEdit::singleline(search_query)
                            .hint_text(hint)
                            .frame(egui::Frame::NONE)
                            .background_color(bg_fill),
                    );
                    ui.painter().line_segment(
                        [
                            edit_response.rect.left_bottom(),
                            edit_response.rect.right_bottom(),
                        ],
                        egui::Stroke::new(1.0_f32, visuals.bg_stroke.color),
                    );
                },
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if *show_window_buttons {
                    if ui.button(egui::RichText::new(X).size(12.0)).clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button(egui::RichText::new(SQUARE).size(12.0)).clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                    }
                    if ui.button(egui::RichText::new(MINUS).size(12.0)).clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                }

                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(GEAR_SIX).size(14.0))
                            .min_size(egui::vec2(18.0, 18.0)),
                    )
                    .clicked()
                {
                    ui.close();
                }

                let user_button_response = if let Some(user) = github_user {
                    let avatar = egui::Image::new(&user.avatar_url)
                        .fit_to_exact_size(egui::vec2(18.0, 18.0))
                        .corner_radius(egui::CornerRadius::same(9))
                        .sense(egui::Sense::click());
                    ui.add(avatar)
                } else {
                    ui.add(
                        egui::Button::new(egui::RichText::new(USER_CIRCLE).size(14.0))
                            .min_size(egui::vec2(18.0, 18.0)),
                    )
                };
                if user_button_response.clicked() {
                    profile_panel_state.open = !profile_panel_state.open;
                }
                profile_action = profile_panel::show(
                    ui,
                    &user_button_response,
                    profile_panel_state,
                    github_user,
                    git_identity,
                    auth_status,
                );
            });
        },
    );

    (action, profile_action)
}

fn repo_display_name(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}
