use eframe::egui;

pub enum PassphrasePromptAction {
    Submit { passphrase: String, remember: bool },
    Cancel,
    None,
}

pub struct PassphrasePromptState {
    pub open: bool,
    pub key_path: String,
    pub passphrase: String,
    pub remember: bool,
}

impl Default for PassphrasePromptState {
    fn default() -> Self {
        Self {
            open: false,
            key_path: String::new(),
            passphrase: String::new(),
            remember: true,
        }
    }
}

pub fn show(ui: &mut egui::Ui, state: &mut PassphrasePromptState) -> PassphrasePromptAction {
    if !state.open {
        return PassphrasePromptAction::None;
    }

    let mut action = PassphrasePromptAction::None;
    let mut is_open = state.open;

    egui::Window::new("SSH Passphrase")
        .collapsible(false)
        .resizable(false)
        .title_bar(false)
        .fixed_size(egui::vec2(720.0, 34.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .open(&mut is_open)
        .show(ui.ctx(), |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(format!(
                        "Enter passphrase for SSH key '{}':",
                        state.key_path
                    ))
                    .size(13.0),
                );

                let password_edit = ui.add_sized(
                    [220.0, 24.0],
                    egui::TextEdit::singleline(&mut state.passphrase)
                        .password(true)
                        .hint_text("Password"),
                );

                if password_edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    action = PassphrasePromptAction::Submit {
                        passphrase: state.passphrase.clone(),
                        remember: state.remember,
                    };
                }

                ui.checkbox(&mut state.remember, "Remember");

                if ui
                    .add_sized([60.0, 22.0], egui::Button::new("Submit"))
                    .clicked()
                {
                    action = PassphrasePromptAction::Submit {
                        passphrase: state.passphrase.clone(),
                        remember: state.remember,
                    };
                }

                if ui
                    .add_sized([60.0, 22.0], egui::Button::new("Cancel"))
                    .clicked()
                {
                    action = PassphrasePromptAction::Cancel;
                }
            });
        });

    if !is_open {
        action = PassphrasePromptAction::Cancel;
    }

    state.open = is_open;
    action
}
