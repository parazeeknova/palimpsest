use crate::state::{AuthStatus, CachedGitIdentity, GitHubUserProfile};
use eframe::egui;
use egui_phosphor::regular::{ARROW_UP_RIGHT, SIGN_OUT};

#[derive(Default)]
pub struct ProfilePanelState {
    pub open: bool,
    pub ssh_keys_expanded: bool,
    pub gpg_keys_expanded: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProfileAction {
    None,
    SignOut,
    RerunSetup,
    OpenGitHubProfile,
}

fn shorten_key_path(path: &str) -> String {
    if let Ok(home) = std::env::var("HOME") {
        if let Some(suffix) = path.strip_prefix(&home) {
            return format!("~{}", suffix);
        }
    }
    path.to_string()
}

#[allow(deprecated)]
pub fn show(
    ui: &mut egui::Ui,
    button_response: &egui::Response,
    state: &mut ProfilePanelState,
    github_user: Option<&GitHubUserProfile>,
    git_identity: Option<&CachedGitIdentity>,
    _auth_status: &AuthStatus,
) -> ProfileAction {
    let mut action = ProfileAction::None;

    let popup_id = ui.make_persistent_id("profile_panel_popup");

    if state.open && !ui.memory(|mem| mem.is_popup_open(popup_id)) {
        ui.memory_mut(|mem| mem.open_popup(popup_id));
    }

    if !state.open {
        return action;
    }

    // Auto-close when clicking outside
    let _response = egui::popup_below_widget(
        ui,
        popup_id,
        button_response,
        egui::PopupCloseBehavior::CloseOnClickOutside,
        |ui| {
            ui.set_min_width(280.0);
            ui.set_max_width(280.0);

            // Popup styling matches titlebar menu
            let spacing = ui.spacing().item_spacing;
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 5.0);

            if github_user.is_none() {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);
                    // Left: Logo
                    let logo = egui::Image::new(egui::include_image!("../assets/logo.svg"))
                        .fit_to_exact_size(egui::vec2(44.0, 44.0))
                        .tint(egui::Color32::from_white_alpha(160));
                    ui.add(logo);

                    // Right: Vertical layout for message and button
                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 4.0);
                        ui.label(
                            egui::RichText::new("Currently not signed in")
                                .size(13.0)
                                .strong(),
                        );
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new("Please re-run the setup wizard to connect your GitHub account and configure Git settings.")
                                    .size(11.0)
                                    .color(ui.visuals().text_color())
                            )
                            .wrap()
                        );
                        ui.add_space(4.0);
                        if ui
                            .button(
                                egui::RichText::new("Re-run Setup Wizard")
                                    .size(11.0)
                                    .strong()
                            )
                            .clicked()
                        {
                            tracing::info!("Profile panel: clicked Re-run Setup Wizard (unauthenticated user)");
                            action = ProfileAction::RerunSetup;
                            state.open = false;
                        }
                    });
                });
            } else {
                // 1. Header Section (GitHub avatar placeholder & username)
                if let Some(user) = github_user {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(10.0, 0.0);

                        let avatar = egui::Image::new(&user.avatar_url)
                            .fit_to_exact_size(egui::vec2(36.0, 36.0))
                            .corner_radius(egui::CornerRadius::same(18));
                        ui.add(avatar);

                        ui.vertical(|ui| {
                            ui.add_space(2.0);
                            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);

                            let display_name =
                                user.name.clone().unwrap_or_else(|| user.login.clone());
                            ui.label(egui::RichText::new(display_name).size(13.0).strong());

                            ui.label(
                                egui::RichText::new(format!("@{}", user.login))
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(165, 165, 165)),
                            );
                        });

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                            ui.visuals_mut().hyperlink_color =
                                egui::Color32::from_rgb(140, 140, 140);
                            ui.with_layout(egui::Layout::top_down(egui::Align::Max), |ui| {
                                ui.add_space(4.0);
                                ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);

                                ui.hyperlink_to(
                                    egui::RichText::new(format!(
                                        "view on github {}",
                                        ARROW_UP_RIGHT
                                    ))
                                    .size(9.5),
                                    &user.html_url,
                                );

                                ui.hyperlink_to(
                                    egui::RichText::new(format!("github {}", ARROW_UP_RIGHT))
                                        .size(9.5),
                                    "https://github.com",
                                );
                            });
                        });
                    });

                    if let Some(ref bio) = user.bio {
                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(bio)
                                .size(11.0)
                                .color(egui::Color32::from_rgb(140, 140, 140))
                                .italics(),
                        );
                    }
                } else {
                    ui.vertical_centered(|ui| {
                        // Not authenticated placeholder
                        let (avatar_rect, _) =
                            ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
                        let center = avatar_rect.center();
                        ui.painter().circle_filled(
                            center,
                            24.0,
                            egui::Color32::from_rgb(80, 80, 80),
                        );
                        ui.painter().text(
                            center,
                            egui::Align2::CENTER_CENTER,
                            "?",
                            egui::FontId::new(20.0, egui::FontFamily::Proportional),
                            egui::Color32::WHITE,
                        );

                        ui.add_space(4.0);
                        ui.label(egui::RichText::new("Local User").size(14.0).strong());
                        ui.label(
                            egui::RichText::new("GitHub disconnected")
                                .size(11.0)
                                .color(egui::Color32::from_rgb(140, 140, 140)),
                        );
                    });
                }

                ui.separator();

                // 2. Git Identity Section
                ui.label(
                    egui::RichText::new("Local Git Identity")
                        .size(12.0)
                        .strong(),
                );

                let name = git_identity
                    .and_then(|i| i.name.clone())
                    .unwrap_or_else(|| "Not Configured".to_string());
                let email = git_identity
                    .and_then(|i| i.email.clone())
                    .unwrap_or_else(|| "Not Configured".to_string());

                ui.label(egui::RichText::new(format!("Name: {}", name)).size(11.0));
                ui.label(egui::RichText::new(format!("Email: {}", email)).size(11.0));

                ui.separator();

                // 3. Security & Auth Section
                ui.label(egui::RichText::new("Security & Auth").size(12.0).strong());

                let (gpg_sign, signing_key, ssh_keys, gpg_keys) = if let Some(i) = git_identity {
                    (
                        i.gpg_sign_commits,
                        i.signing_key.clone(),
                        i.ssh_keys.clone(),
                        i.gpg_keys.clone(),
                    )
                } else {
                    (false, None, Vec::new(), Vec::new())
                };

                let sign_status = if gpg_sign { "Enabled" } else { "Disabled" };
                ui.label(
                    egui::RichText::new(format!("Commit Signing: {}", sign_status)).size(11.0),
                );

                if let Some(key) = signing_key {
                    ui.label(egui::RichText::new(format!("Signing Key: {}", key)).size(11.0));
                }

                let mut ssh_expanded = state.ssh_keys_expanded;
                if !ssh_keys.is_empty() {
                    let label = format!("SSH Keys ({})", ssh_keys.len());
                    let label_text = egui::RichText::new(label)
                        .size(11.0)
                        .color(ui.visuals().text_color());

                    let mut clicked = false;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                        let response =
                            ui.add(egui::Label::new(label_text).sense(egui::Sense::click()));

                        let underline_color = egui::Color32::from_rgb(120, 120, 120);
                        let rect = response.rect;
                        ui.painter().line_segment(
                            [rect.left_bottom(), rect.right_bottom()],
                            egui::Stroke::new(1.0_f32, underline_color),
                        );

                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        if response.clicked() {
                            clicked = true;
                        }
                    });

                    if clicked {
                        ssh_expanded = !ssh_expanded;
                    }

                    if ssh_expanded {
                        ui.indent("ssh_keys_list", |ui| {
                            for key in ssh_keys {
                                let path_str = shorten_key_path(&key.path);
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!(
                                            "• {} ({})",
                                            path_str, key.key_type
                                        ))
                                        .size(10.0)
                                        .color(egui::Color32::from_rgb(160, 160, 160)),
                                    )
                                    .wrap(),
                                );
                            }
                        });
                    }
                } else {
                    ui.label(egui::RichText::new("SSH Keys: None").size(11.0));
                }
                state.ssh_keys_expanded = ssh_expanded;

                let mut gpg_expanded = state.gpg_keys_expanded;
                if !gpg_keys.is_empty() {
                    let label = format!("GPG Keys ({})", gpg_keys.len());
                    let label_text = egui::RichText::new(label)
                        .size(11.0)
                        .color(ui.visuals().text_color());

                    let mut clicked = false;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
                        let response =
                            ui.add(egui::Label::new(label_text).sense(egui::Sense::click()));

                        let underline_color = egui::Color32::from_rgb(120, 120, 120);
                        let rect = response.rect;
                        ui.painter().line_segment(
                            [rect.left_bottom(), rect.right_bottom()],
                            egui::Stroke::new(1.0_f32, underline_color),
                        );

                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        if response.clicked() {
                            clicked = true;
                        }
                    });

                    if clicked {
                        gpg_expanded = !gpg_expanded;
                    }

                    if gpg_expanded {
                        ui.indent("gpg_keys_list", |ui| {
                            for key in gpg_keys {
                                ui.add(
                                    egui::Label::new(
                                        egui::RichText::new(format!(
                                            "• {} ({})",
                                            key.key_id, key.uid
                                        ))
                                        .size(10.0)
                                        .color(egui::Color32::from_rgb(160, 160, 160)),
                                    )
                                    .wrap(),
                                );
                            }
                        });
                    }
                } else {
                    ui.label(egui::RichText::new("GPG Keys: None").size(11.0));
                }
                state.gpg_keys_expanded = gpg_expanded;

                // 4. Action buttons
                if github_user.is_some() {
                    ui.columns(2, |columns| {
                        let w = columns[0].available_width();
                        if columns[0]
                            .add_sized(
                                egui::vec2(w, 24.0),
                                egui::Button::new(egui::RichText::new("Re-run Setup").size(11.0)),
                            )
                            .clicked()
                        {
                            tracing::info!("Profile panel: clicked Re-run Setup Wizard");
                            action = ProfileAction::RerunSetup;
                            state.open = false;
                        }

                        let w = columns[1].available_width();
                        if columns[1]
                            .add_sized(
                                egui::vec2(w, 24.0),
                                egui::Button::new(
                                    egui::RichText::new(format!("{} Sign Out", SIGN_OUT))
                                        .size(11.0)
                                        .color(egui::Color32::from_rgb(220, 80, 80)),
                                ),
                            )
                            .clicked()
                        {
                            tracing::info!("Profile panel: clicked Sign Out of GitHub");
                            action = ProfileAction::SignOut;
                            state.open = false;
                        }
                    });
                } else {
                    let w = ui.available_width();
                    if ui
                        .add_sized(
                            egui::vec2(w, 24.0),
                            egui::Button::new(
                                egui::RichText::new("Re-run Setup Wizard").size(12.0),
                            ),
                        )
                        .clicked()
                    {
                        tracing::info!("Profile panel: clicked Re-run Setup Wizard");
                        action = ProfileAction::RerunSetup;
                        state.open = false;
                    }
                }

                ui.add_space(4.0);
                ui.spacing_mut().item_spacing = spacing;
            }
        },
    );

    if !ui.memory(|mem| mem.is_popup_open(popup_id)) {
        state.open = false;
    }

    action
}
