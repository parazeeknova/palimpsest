use crate::state::{AuthStatus, CachedGitIdentity, GitHubUserProfile};
use eframe::egui;
use egui_phosphor::regular::{GITHUB_LOGO, KEY, LOCK, SHIELD_CHECK, SIGN_OUT, USER};

#[derive(Default)]
pub struct ProfilePanelState {
    pub open: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ProfileAction {
    None,
    SignOut,
    RerunSetup,
    OpenGitHubProfile,
}

#[allow(deprecated)]
pub fn show(
    ui: &mut egui::Ui,
    button_response: &egui::Response,
    state: &mut ProfilePanelState,
    github_user: Option<&GitHubUserProfile>,
    git_identity: Option<&CachedGitIdentity>,
    auth_status: &AuthStatus,
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
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);

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
                ui.vertical_centered(|ui| {
                    if let Some(user) = github_user {
                        let avatar = egui::Image::new(&user.avatar_url)
                            .fit_to_exact_size(egui::vec2(48.0, 48.0))
                            .corner_radius(egui::CornerRadius::same(24));
                        ui.add(avatar);

                        ui.add_space(4.0);

                        let display_name = user.name.clone().unwrap_or_else(|| user.login.clone());
                        ui.label(egui::RichText::new(display_name).size(14.0).strong());

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
                            ui.add_space(
                                (ui.available_width()
                                    - ui.painter()
                                        .layout_no_wrap(
                                            format!("@{}", user.login),
                                            egui::FontId::proportional(11.0),
                                            egui::Color32::PLACEHOLDER,
                                        )
                                        .rect
                                        .width())
                                    / 2.0,
                            );
                            ui.label(
                                egui::RichText::new(GITHUB_LOGO)
                                    .size(12.0)
                                    .color(egui::Color32::from_rgb(165, 165, 165)),
                            );
                            ui.label(
                                egui::RichText::new(format!("@{}", user.login))
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(165, 165, 165)),
                            );
                        });

                        if let Some(ref bio) = user.bio {
                            ui.add_space(2.0);
                            ui.label(
                                egui::RichText::new(bio)
                                    .size(11.0)
                                    .color(egui::Color32::from_rgb(140, 140, 140))
                                    .italics(),
                            );
                        }
                    } else {
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
                    }
                });

                ui.separator();

                // 2. Git Identity Section
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(USER)
                            .size(13.0)
                            .color(egui::Color32::from_rgb(165, 165, 165)),
                    );
                    ui.label(egui::RichText::new("Git Identity").size(12.0).strong());
                });

                let name = git_identity
                    .and_then(|i| i.name.clone())
                    .unwrap_or_else(|| "Not Configured".to_string());
                let email = git_identity
                    .and_then(|i| i.email.clone())
                    .unwrap_or_else(|| "Not Configured".to_string());

                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new(format!("Name: {}", name)).size(11.0));
                        ui.label(egui::RichText::new(format!("Email: {}", email)).size(11.0));
                    });
                });

                ui.separator();

                // 3. Auth and Keys Section
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(SHIELD_CHECK)
                            .size(13.0)
                            .color(egui::Color32::from_rgb(165, 165, 165)),
                    );
                    ui.label(egui::RichText::new("Security & Auth").size(12.0).strong());
                });

                ui.horizontal(|ui| {
                    ui.add_space(16.0);
                    ui.vertical(|ui| {
                        // Connection status dot
                        ui.horizontal(|ui| {
                            let dot_color = match auth_status {
                                AuthStatus::Connected => egui::Color32::from_rgb(80, 180, 80),
                                AuthStatus::Connecting => egui::Color32::from_rgb(220, 180, 80),
                                _ => egui::Color32::from_rgb(120, 120, 120),
                            };
                            let (dot_rect, _) =
                                ui.allocate_exact_size(egui::vec2(8.0, 8.0), egui::Sense::hover());
                            ui.painter()
                                .circle_filled(dot_rect.center(), 4.0, dot_color);
                            ui.label(
                                egui::RichText::new(format!("GitHub: {:?}", auth_status))
                                    .size(11.0),
                            );
                        });

                        let ssh_count = git_identity.map(|i| i.ssh_key_count).unwrap_or(0);
                        let gpg_count = git_identity.map(|i| i.gpg_key_count).unwrap_or(0);

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(KEY).size(11.0));
                            ui.label(
                                egui::RichText::new(format!("SSH Keys: {}", ssh_count)).size(11.0),
                            );
                        });

                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(LOCK).size(11.0));
                            ui.label(
                                egui::RichText::new(format!("GPG Keys: {}", gpg_count)).size(11.0),
                            );
                        });
                    });
                });

                ui.separator();

                // 4. Action buttons
                if github_user.is_some()
                    && ui
                        .button(egui::RichText::new("View GitHub Profile").size(12.0))
                        .clicked()
                {
                    tracing::info!("Profile panel: clicked View GitHub Profile");
                    action = ProfileAction::OpenGitHubProfile;
                    state.open = false;
                }

                if ui
                    .button(egui::RichText::new("Re-run Setup Wizard").size(12.0))
                    .clicked()
                {
                    tracing::info!("Profile panel: clicked Re-run Setup Wizard");
                    action = ProfileAction::RerunSetup;
                    state.open = false;
                }

                if github_user.is_some()
                    && ui
                        .button(
                            egui::RichText::new(format!("{} Sign Out of GitHub", SIGN_OUT))
                                .size(12.0)
                                .color(egui::Color32::from_rgb(220, 80, 80)),
                        )
                        .clicked()
                {
                    tracing::info!("Profile panel: clicked Sign Out of GitHub");
                    action = ProfileAction::SignOut;
                    state.open = false;
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
