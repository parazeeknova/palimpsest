use eframe::egui;
use egui_phosphor::regular::{
    ARROW_LEFT, ARROW_RIGHT, ARROW_UP_RIGHT, CLOCK, FOLDER, FOLDER_OPEN, GEAR_SIX, GITHUB_LOGO,
    GLOBE_SIMPLE, LIST, MAGNIFYING_GLASS, MINUS, POWER, SQUARE, TERMINAL_WINDOW, USER_CIRCLE, X,
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

            if ui.button(egui::RichText::new(LIST).size(12.0)).clicked() {
                *menu_open = !*menu_open;
            }

            ui.label(egui::RichText::new(ARROW_LEFT).size(12.0));
            ui.label(egui::RichText::new(ARROW_RIGHT).size(12.0));

            if *menu_open {
                ui.horizontal(|ui| {
                    let file_resp = ui.menu_button(egui::RichText::new("File").size(12.0), |ui| {
                        ui.set_max_width(200.0);
                        if ui
                            .button(
                                egui::RichText::new(format!("{}  Open repository", FOLDER_OPEN))
                                    .size(12.0),
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
                            .button(egui::RichText::new(format!("{}  Exit", POWER)).size(12.0))
                            .clicked()
                        {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                    if file_resp.response.hovered() || file_resp.inner.is_some() {
                        ui.painter().rect_filled(
                            file_resp.response.rect,
                            2.0,
                            egui::Color32::from_white_alpha(30),
                        );
                    }

                    let window_resp =
                        ui.menu_button(egui::RichText::new("Window").size(12.0), |ui| {
                            ui.set_max_width(200.0);
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
