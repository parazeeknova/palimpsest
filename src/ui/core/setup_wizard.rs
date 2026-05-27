use eframe::egui;
use egui_phosphor::regular::{
    ARROW_LEFT, ARROW_RIGHT, ARROW_SQUARE_OUT, CHECK_CIRCLE, COPY, ENVELOPE, GITHUB_LOGO, KEY,
    LOCK, SHIELD_CHECK, USER, WARNING_CIRCLE,
};

use crate::auth::git_identity::{GhCliStatus, GitIdentity, GpgKeyInfo, SshKeyInfo};
use crate::auth::github_oauth::GitHubUser;

#[derive(Default)]
pub struct SetupWizardState {
    pub step: WizardStep,
    pub git_name: String,
    pub git_email: String,
    pub identity_detected: bool,
    pub identity: Option<GitIdentity>,
    pub gh_cli_status: Option<GhCliStatus>,
    pub ssh_keys: Vec<SshKeyInfo>,
    pub gpg_keys: Vec<GpgKeyInfo>,
    pub device_code_response: Option<DeviceFlowUiState>,
    pub auth_polling: bool,
    pub auth_error: Option<String>,
    pub github_user: Option<GitHubUser>,
    pub detection_started: bool,
}

#[derive(Default, PartialEq, Clone, Debug)]
pub enum WizardStep {
    #[default]
    GitIdentity,
    SshGpgKeys,
    GitHubAuth,
    Done,
}

#[derive(Clone)]
pub struct DeviceFlowUiState {
    pub user_code: String,
    pub verification_uri: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum WizardAction {
    None,
    StartDetection,
    StartDeviceFlow,
    OpenVerificationUrl(String),
    Complete { git_name: String, git_email: String },
    Skip,
}

const STEP_COUNT: usize = 4;

pub fn show(ui: &mut egui::Ui, state: &mut SetupWizardState) -> WizardAction {
    let mut action = WizardAction::None;

    egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(24, 16))
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 6.0);

            ui.horizontal(|ui| {
                // Left Column: Logo (shown for every step)
                ui.allocate_ui(egui::vec2(140.0, ui.available_height()), |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(24.0);
                        let logo = egui::Image::new(egui::include_image!("../../assets/logo.svg"))
                            .fit_to_exact_size(egui::vec2(128.0, 128.0))
                            .tint(egui::Color32::from_white_alpha(120));
                        ui.add(logo);
                    });
                });

                ui.add_space(16.0);

                // Right Column: Active Step Content + Navigation
                ui.allocate_ui(
                    egui::vec2(ui.available_width(), ui.available_height()),
                    |ui| {
                        ui.vertical(|ui| {
                            match state.step.clone() {
                                WizardStep::GitIdentity => {
                                    action = render_git_identity_step(ui, state);
                                }
                                WizardStep::SshGpgKeys => {
                                    action = render_ssh_gpg_step(ui, state);
                                }
                                WizardStep::GitHubAuth => {
                                    action = render_github_auth_step(ui, state);
                                }
                                WizardStep::Done => {
                                    action = render_done_step(ui, state);
                                }
                            }

                            ui.add_space(8.0);
                            let nav_action = render_step_indicator(ui, state);
                            if action == WizardAction::None {
                                action = nav_action;
                            }
                        });
                    },
                );
            });
        });

    action
}

fn lerp_color(from: egui::Color32, to: egui::Color32, t: f32) -> egui::Color32 {
    let r = from.r() as f32 + (to.r() as f32 - from.r() as f32) * t;
    let g = from.g() as f32 + (to.g() as f32 - from.g() as f32) * t;
    let b = from.b() as f32 + (to.b() as f32 - from.b() as f32) * t;
    let a = from.a() as f32 + (to.a() as f32 - from.a() as f32) * t;
    egui::Color32::from_rgba_premultiplied(r as u8, g as u8, b as u8, a as u8)
}

fn render_pill_button(
    ui: &mut egui::Ui,
    text: &str,
    is_primary: bool,
    fixed_width: f32,
) -> egui::Response {
    let height = 24.0_f32;
    let button_size = egui::vec2(fixed_width, height);

    // Allocate space
    let (rect, response) = ui.allocate_exact_size(button_size, egui::Sense::click());

    // Get animation factors
    let hover_t = ui.ctx().animate_bool(response.id, response.hovered());

    // Smooth active state using pointer down while hovered
    let active = response.hovered() && ui.input(|i| i.pointer.primary_down());
    let active_t = ui.ctx().animate_bool(response.id.with("active"), active);

    // Expand drawn rect slightly on hover, shrink on click
    let expansion = 1.0_f32 * hover_t - 1.0_f32 * active_t;
    let draw_rect = rect.expand(expansion);

    // Color interpolation
    let bg_color = if is_primary {
        let inactive = egui::Color32::from_rgb(38, 38, 38);
        let hover = egui::Color32::from_rgb(52, 52, 52);
        let active = egui::Color32::from_rgb(26, 26, 26);

        let c = lerp_color(inactive, hover, hover_t);
        lerp_color(c, active, active_t)
    } else {
        let inactive = egui::Color32::from_rgb(32, 32, 32);
        let hover = egui::Color32::from_rgb(45, 45, 45);
        let active = egui::Color32::from_rgb(20, 20, 20);

        let c = lerp_color(inactive, hover, hover_t);
        lerp_color(c, active, active_t)
    };

    let stroke_color = if is_primary {
        let inactive = egui::Color32::from_rgb(60, 60, 60);
        let hover = egui::Color32::from_rgb(100, 100, 100);
        let active = egui::Color32::from_rgb(48, 48, 48);

        let c = lerp_color(inactive, hover, hover_t);
        lerp_color(c, active, active_t)
    } else {
        let inactive = egui::Color32::from_rgb(50, 50, 50);
        let hover = egui::Color32::from_rgb(80, 80, 80);
        let active = egui::Color32::from_rgb(40, 40, 40);

        let c = lerp_color(inactive, hover, hover_t);
        lerp_color(c, active, active_t)
    };

    let text_color = if is_primary {
        egui::Color32::WHITE
    } else {
        let inactive = egui::Color32::from_rgb(180, 180, 180);
        let hover = egui::Color32::from_rgb(255, 255, 255);
        let active = egui::Color32::from_rgb(140, 140, 140);

        let c = lerp_color(inactive, hover, hover_t);
        lerp_color(c, active, active_t)
    };

    // Draw background (pill-shaped)
    let corner_radius = draw_rect.height() / 2.0;
    ui.painter().rect(
        draw_rect,
        egui::CornerRadius::same(corner_radius as u8),
        bg_color,
        egui::Stroke::new(1.0_f32, stroke_color),
        egui::StrokeKind::Inside,
    );

    let font_id = egui::FontId::new(11.0, egui::FontFamily::Proportional);
    let text_galley = ui
        .painter()
        .layout_no_wrap(text.to_string(), font_id, text_color);

    // Center text in rect
    let text_pos = draw_rect.min
        + egui::vec2(
            (draw_rect.width() - text_galley.size().x) / 2.0,
            (draw_rect.height() - text_galley.size().y) / 2.0,
        );

    ui.painter()
        .galley(text_pos, text_galley, egui::Color32::PLACEHOLDER);

    response
}

fn render_step_indicator(ui: &mut egui::Ui, state: &mut SetupWizardState) -> WizardAction {
    let mut action = WizardAction::None;
    let active_index = match state.step {
        WizardStep::GitIdentity => 0,
        WizardStep::SshGpgKeys => 1,
        WizardStep::GitHubAuth => 2,
        WizardStep::Done => 3,
    };

    let dash_width = 24.0_f32;
    let dash_height = 4.0_f32;
    let spacing = 6.0_f32;

    let has_back = active_index > 0 && active_index <= 3;
    let has_next = active_index < 3;
    let has_finish_step = active_index == 3;
    let has_skip = active_index == 2 && state.github_user.is_none();

    let row_width = ui.available_width();

    let back_width = 76.0_f32;
    let next_width = 76.0_f32;
    let skip_width = 60.0_f32;
    let finish_width = 76.0_f32;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

        // 1. Back button (left-aligned)
        if has_back {
            let clicked =
                render_pill_button(ui, &format!("{} Back", ARROW_LEFT), false, back_width)
                    .clicked();

            if clicked {
                tracing::info!("Setup wizard: clicked Back (from_step: {:?})", state.step);
                match state.step {
                    WizardStep::SshGpgKeys => state.step = WizardStep::GitIdentity,
                    WizardStep::GitHubAuth => state.step = WizardStep::SshGpgKeys,
                    WizardStep::Done => state.step = WizardStep::GitHubAuth,
                    _ => {}
                }
            }
        } else {
            // Draw a spacer of back_width so that the capsule centering is preserved!
            ui.allocate_space(egui::vec2(back_width, 24.0));
        }

        // 2. Centering spacer for Capsule
        let capsule_width =
            (STEP_COUNT as f32 * dash_width) + ((STEP_COUNT - 1) as f32 * spacing) + 16.0_f32;
        let left_spacer = (row_width / 2.0) - (capsule_width / 2.0) - back_width - 8.0;
        if left_spacer > 0.0 {
            ui.add_space(left_spacer);
        }

        // 3. Middle: Capsule Indicator
        egui::Frame::NONE
            .fill(egui::Color32::from_rgb(32, 32, 32))
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(spacing, 0.0);
                    for step_index in 0..STEP_COUNT {
                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(dash_width, dash_height),
                            egui::Sense::hover(),
                        );

                        let color = if step_index == active_index {
                            egui::Color32::from_rgb(200, 200, 200)
                        } else if step_index < active_index {
                            if step_index == 2 && state.github_user.is_none() {
                                egui::Color32::from_rgb(220, 80, 80) // Red for skipped/not signed in
                            } else {
                                egui::Color32::from_rgb(80, 180, 80) // Green for completed
                            }
                        } else {
                            egui::Color32::from_rgb(60, 60, 60)
                        };

                        ui.painter()
                            .rect_filled(rect, egui::CornerRadius::same(2), color);
                    }
                });
            });

        // 4. Right spacer: push Next/Skip/Finish buttons to the far right!
        let right_buttons_width = if has_skip {
            skip_width + next_width + 8.0
        } else if has_finish_step {
            finish_width
        } else {
            next_width
        };
        let right_spacer = ui.available_width() - right_buttons_width;
        if right_spacer > 0.0 {
            ui.add_space(right_spacer);
        }

        // 5. Right side: Skip, Next and Finish buttons
        if has_skip {
            let clicked = render_pill_button(ui, "Skip", false, skip_width).clicked();

            if clicked {
                tracing::info!("Setup wizard: skipped GitHub authentication");
                action = WizardAction::Skip;
            }
        }

        if has_next {
            let button_text = if active_index == 2 && state.github_user.is_some() {
                "Finish"
            } else {
                "Next"
            };

            let clicked = render_pill_button(
                ui,
                &format!("{} {}", button_text, ARROW_RIGHT),
                true,
                next_width,
            )
            .clicked();

            if clicked {
                tracing::info!("Setup wizard: clicked Next (from_step: {:?})", state.step);
                match state.step {
                    WizardStep::GitIdentity => state.step = WizardStep::SshGpgKeys,
                    WizardStep::SshGpgKeys => state.step = WizardStep::GitHubAuth,
                    WizardStep::GitHubAuth => {
                        state.step = WizardStep::Done;
                    }
                    _ => {}
                }
            }
        }

        if has_finish_step {
            let clicked =
                render_pill_button(ui, &format!("Finish {}", ARROW_RIGHT), true, finish_width)
                    .clicked();

            if clicked {
                tracing::info!("Setup wizard: setup completed via Finish");
                action = WizardAction::Complete {
                    git_name: state.git_name.clone(),
                    git_email: state.git_email.clone(),
                };
            }
        }
    });

    action
}

fn render_git_identity_step(ui: &mut egui::Ui, state: &mut SetupWizardState) -> WizardAction {
    let mut action = WizardAction::None;

    if !state.detection_started {
        action = WizardAction::StartDetection;
    }

    ui.vertical(|ui| {
        ui.add_space(12.0);
        ui.label(
            egui::RichText::new("👋 Welcome to Palimpsest")
                .size(20.0)
                .strong(),
        );
        ui.label(
            egui::RichText::new("Let's set up your Git identity")
                .size(13.0)
                .color(egui::Color32::from_rgb(165, 165, 165)),
        );

        ui.add_space(16.0);

        ui.scope(|ui| {
            let visuals = ui.visuals_mut();
            visuals.extreme_bg_color = egui::Color32::from_rgb(38, 38, 38);
            visuals.widgets.inactive.bg_stroke =
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(60, 60, 60));
            visuals.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
            visuals.widgets.active.bg_stroke =
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(28, 145, 220));
            visuals.widgets.active.corner_radius = egui::CornerRadius::same(6);
            visuals.widgets.hovered.bg_stroke =
                egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(80, 80, 80));
            visuals.widgets.hovered.corner_radius = egui::CornerRadius::same(6);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(USER).size(14.0));
                ui.label(egui::RichText::new("Name").size(12.0));
            });
            ui.add_sized(
                [ui.available_width(), 26.0],
                egui::TextEdit::singleline(&mut state.git_name)
                    .hint_text("Your name (e.g. Jane Doe)")
                    .margin(egui::Margin::symmetric(8, 6)),
            );

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(ENVELOPE).size(14.0));
                ui.label(egui::RichText::new("Email").size(12.0));
            });
            ui.add_sized(
                [ui.available_width(), 26.0],
                egui::TextEdit::singleline(&mut state.git_email)
                    .hint_text("your@email.com")
                    .margin(egui::Margin::symmetric(8, 6)),
            );
        });

        ui.add_space(10.0);

        if let Some(ref cli_status) = state.gh_cli_status {
            ui.horizontal(|ui| {
                if cli_status.logged_in {
                    ui.label(
                        egui::RichText::new(CHECK_CIRCLE)
                            .size(14.0)
                            .color(egui::Color32::from_rgb(80, 180, 80)),
                    );
                    let status_text = match &cli_status.username {
                        Some(username) => {
                            format!("gh CLI authenticated as {}", username)
                        }
                        None => "gh CLI authenticated".to_string(),
                    };
                    ui.label(egui::RichText::new(status_text).size(12.0));
                } else {
                    ui.label(
                        egui::RichText::new(WARNING_CIRCLE)
                            .size(14.0)
                            .color(egui::Color32::from_rgb(200, 80, 80)),
                    );
                    ui.label(egui::RichText::new("gh CLI not authenticated").size(12.0));
                }
            });
        }

        ui.add_space(12.0);
    });

    action
}

fn render_ssh_gpg_step(ui: &mut egui::Ui, state: &mut SetupWizardState) -> WizardAction {
    ui.add_space(12.0);
    ui.label(
        egui::RichText::new("🔒 Security Configuration")
            .size(20.0)
            .strong(),
    );
    ui.label(
        egui::RichText::new("Detected keys on your system")
            .size(13.0)
            .color(egui::Color32::from_rgb(165, 165, 165)),
    );

    ui.add_space(16.0);

    // SSH Keys section
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(KEY).size(14.0));
        ui.label(egui::RichText::new("SSH Keys").size(13.0).strong());
    });

    if state.ssh_keys.is_empty() {
        ui.label(
            egui::RichText::new("  No SSH keys detected")
                .size(12.0)
                .color(egui::Color32::from_rgb(140, 140, 140)),
        );
    } else {
        for ssh_key in &state.ssh_keys {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new(CHECK_CIRCLE)
                        .size(12.0)
                        .color(egui::Color32::from_rgb(80, 180, 80)),
                );
                let key_label = format!("{} ({})", ssh_key.path, ssh_key.key_type);
                ui.label(egui::RichText::new(key_label).size(12.0));
            });
        }
    }

    ui.add_space(10.0);

    // GPG Keys section
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(SHIELD_CHECK).size(14.0));
        ui.label(egui::RichText::new("GPG Keys").size(13.0).strong());
    });

    if state.gpg_keys.is_empty() {
        ui.label(
            egui::RichText::new("  No GPG keys detected")
                .size(12.0)
                .color(egui::Color32::from_rgb(140, 140, 140)),
        );
    } else {
        for gpg_key in &state.gpg_keys {
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                ui.label(
                    egui::RichText::new(LOCK)
                        .size(12.0)
                        .color(egui::Color32::from_rgb(80, 180, 80)),
                );
                let key_label = format!("{} ({})", gpg_key.key_id, gpg_key.uid);
                ui.label(egui::RichText::new(key_label).size(12.0));
            });
        }
    }

    ui.add_space(20.0);

    WizardAction::None
}

fn render_github_auth_step(ui: &mut egui::Ui, state: &mut SetupWizardState) -> WizardAction {
    let mut action = WizardAction::None;

    ui.add_space(12.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(GITHUB_LOGO).size(22.0));
        ui.label(egui::RichText::new("Connect to GitHub").size(20.0).strong());
    });

    ui.add_space(12.0);

    // 1. Connection Status / Actions (rendered vertically)
    ui.vertical(|ui| {
        if let Some(ref github_user) = state.github_user {
            // Successfully connected
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(CHECK_CIRCLE)
                        .size(16.0)
                        .color(egui::Color32::from_rgb(80, 180, 80)),
                );
                ui.label(
                    egui::RichText::new(format!("Connected as {}", github_user.login))
                        .size(14.0)
                        .color(egui::Color32::from_rgb(80, 180, 80)),
                );
            });
        } else if let Some(ref error_message) = state.auth_error {
            // Error state
            ui.label(
                egui::RichText::new(format!("{} {}", WARNING_CIRCLE, error_message))
                    .size(12.0)
                    .color(egui::Color32::from_rgb(220, 80, 80)),
            );
            ui.add_space(8.0);

            let clicked =
                render_pill_button(ui, &format!("{} Try Again", GITHUB_LOGO), true, 120.0_f32)
                    .clicked();

            if clicked {
                tracing::info!("Setup wizard: retrying GitHub authorization");
                state.auth_error = None;
                action = WizardAction::StartDeviceFlow;
            }
        } else if let Some(ref device_flow_state) = state.device_code_response.clone() {
            // Device flow active — show code and verification URL
            ui.label(
                egui::RichText::new("Enter the code below at GitHub:")
                    .size(13.0)
                    .color(egui::Color32::from_rgb(165, 165, 165)),
            );

            ui.add_space(12.0);

            // Code layout: show code and a copy button
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&device_flow_state.user_code)
                        .size(28.0)
                        .strong()
                        .monospace(),
                );
                ui.add_space(8.0);

                let copy_clicked = render_pill_button(ui, COPY, false, 30.0_f32).clicked();

                if copy_clicked {
                    tracing::info!("Setup wizard: copied device verification code to clipboard");
                    ui.ctx().copy_text(device_flow_state.user_code.clone());
                }
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Open:").size(12.0));
                let link_clicked = ui
                    .link(egui::RichText::new(&device_flow_state.verification_uri).size(12.0))
                    .clicked();

                ui.add_space(4.0);
                let icon_clicked =
                    render_pill_button(ui, ARROW_SQUARE_OUT, false, 30.0_f32).clicked();

                if link_clicked || icon_clicked {
                    tracing::info!("Setup wizard: opening GitHub verification URL");
                    action = WizardAction::OpenVerificationUrl(
                        device_flow_state.verification_uri.clone(),
                    );
                }
            });

            if state.auth_polling {
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(
                        egui::RichText::new("Waiting for authorization...")
                            .size(12.0)
                            .color(egui::Color32::from_rgb(165, 165, 165)),
                    );
                });
            }
        } else {
            // Initial state — prompt to connect
            ui.label(
                egui::RichText::new("Connect your GitHub account for remote features")
                    .size(13.0)
                    .color(egui::Color32::from_rgb(165, 165, 165)),
            );

            ui.add_space(16.0);

            let clicked = render_pill_button(
                ui,
                &format!("{}  Connect to GitHub", GITHUB_LOGO),
                true,
                170.0_f32,
            )
            .clicked();

            if clicked {
                tracing::info!(
                    "Setup wizard: clicked Connect to GitHub to start authorization flow"
                );
                action = WizardAction::StartDeviceFlow;
            }
        }
    });

    ui.add_space(20.0);

    // 2. Awesome Facts Card (rendered below status/actions)
    egui::Frame::NONE
        .fill(egui::Color32::from_rgb(32, 32, 32))
        .corner_radius(egui::CornerRadius::same(6))
        .inner_margin(egui::Margin::symmetric(12, 10))
        .stroke(egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(50, 50, 50)))
        .show(ui, |ui| {
            ui.vertical(|ui| {
                ui.label(egui::RichText::new("💡 Did you know?").strong().size(13.0));
                ui.add_space(8.0);

                // Fact 1
                ui.horizontal_top(|ui| {
                    ui.label(egui::RichText::new("•").color(egui::Color32::from_rgb(28, 145, 220)).strong());
                    ui.label(
                        egui::RichText::new("Palimpsest parses commit histories at lightning speeds by leveraging Rust's zero-cost abstractions.")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(180, 180, 180)),
                    );
                });
                ui.add_space(6.0);

                // Fact 2
                ui.horizontal_top(|ui| {
                    ui.label(egui::RichText::new("•").color(egui::Color32::from_rgb(28, 145, 220)).strong());
                    ui.label(
                        egui::RichText::new("GitHub integration enables viewing pull requests, remote branches, and online commit statuses seamlessly.")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(180, 180, 180)),
                    );
                });
                ui.add_space(6.0);

                // Fact 3
                ui.horizontal_top(|ui| {
                    ui.label(egui::RichText::new("•").color(egui::Color32::from_rgb(28, 145, 220)).strong());
                    ui.label(
                        egui::RichText::new("Your credentials are never stored in plain text. They are saved directly to your OS secure keyring.")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(180, 180, 180)),
                    );
                });
                ui.add_space(6.0);

                // Fact 4
                ui.horizontal_top(|ui| {
                    ui.label(egui::RichText::new("•").color(egui::Color32::from_rgb(28, 145, 220)).strong());
                    ui.label(
                        egui::RichText::new("The name 'Palimpsest' comes from ancient manuscripts reused over time, much like git branch revisions.")
                            .size(11.0)
                            .color(egui::Color32::from_rgb(180, 180, 180)),
                    );
                });
            });
        });

    ui.add_space(16.0);

    action
}

fn render_done_step(ui: &mut egui::Ui, state: &mut SetupWizardState) -> WizardAction {
    let action = WizardAction::None;

    ui.add_space(20.0);
    ui.vertical(|ui| {
        ui.label(egui::RichText::new("✅ All Set!").size(22.0).strong());
    });

    ui.add_space(8.0);
    ui.label(
        egui::RichText::new("Here's a summary of your configuration:")
            .size(13.0)
            .color(egui::Color32::from_rgb(165, 165, 165)),
    );

    ui.add_space(16.0);

    // Summary items
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(USER).size(14.0));
        ui.label(egui::RichText::new("Name:").size(12.0).strong());
        let display_name = if state.git_name.is_empty() {
            "(not set)".to_string()
        } else {
            state.git_name.clone()
        };
        ui.label(egui::RichText::new(display_name).size(12.0));
    });

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(ENVELOPE).size(14.0));
        ui.label(egui::RichText::new("Email:").size(12.0).strong());
        let display_email = if state.git_email.is_empty() {
            "(not set)".to_string()
        } else {
            state.git_email.clone()
        };
        ui.label(egui::RichText::new(display_email).size(12.0));
    });

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(GITHUB_LOGO).size(14.0));
        ui.label(egui::RichText::new("GitHub:").size(12.0).strong());
        if let Some(ref github_user) = state.github_user {
            ui.label(
                egui::RichText::new(format!("{} {}", CHECK_CIRCLE, github_user.login))
                    .size(12.0)
                    .color(egui::Color32::from_rgb(80, 180, 80)),
            );
        } else {
            ui.label(
                egui::RichText::new("Not connected")
                    .size(12.0)
                    .color(egui::Color32::from_rgb(140, 140, 140)),
            );
        }
    });

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(KEY).size(14.0));
        ui.label(egui::RichText::new("SSH Keys:").size(12.0).strong());
        ui.label(egui::RichText::new(format!("{}", state.ssh_keys.len())).size(12.0));
    });

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(SHIELD_CHECK).size(14.0));
        ui.label(egui::RichText::new("GPG Keys:").size(12.0).strong());
        ui.label(egui::RichText::new(format!("{}", state.gpg_keys.len())).size(12.0));
    });

    ui.add_space(16.0);
    ui.separator();
    ui.add_space(12.0);

    ui.vertical(|ui| {
        ui.label(egui::RichText::new("🚀 System Capabilities").strong().size(13.0));
        ui.add_space(6.0);

        // Core visualizer status (always enabled)
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("✓").color(egui::Color32::from_rgb(80, 180, 80)).strong());
            ui.label(egui::RichText::new("Local Git history visualizer (active)").size(12.0));
        });

        // GitHub Integration Status
        if state.github_user.is_some() {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("✓").color(egui::Color32::from_rgb(80, 180, 80)).strong());
                ui.label(egui::RichText::new("GitHub remote services enabled").size(12.0));
            });
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("  • Pull request details, remote branches, and online status checks are fully integrated.")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(140, 140, 140)),
            );
        } else {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("⚠").color(egui::Color32::from_rgb(220, 140, 40)).strong());
                ui.label(egui::RichText::new("GitHub integration is skipped").size(12.0).color(egui::Color32::from_rgb(200, 150, 100)));
            });
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new("  • You are missing: pull request metadata, remote branch history syncing, and remote actions.")
                    .size(11.0)
                    .color(egui::Color32::from_rgb(150, 130, 120)),
            );
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.add_space(12.0);
                let clicked = render_pill_button(
                    ui,
                    &format!("{} Connect to GitHub", GITHUB_LOGO),
                    true,
                    150.0_f32,
                ).clicked();
                if clicked {
                    tracing::info!("Setup wizard Done screen: redirecting to GitHub authorization screen");
                    state.step = WizardStep::GitHubAuth;
                }
            });
        }
    });

    ui.add_space(24.0);

    action
}
