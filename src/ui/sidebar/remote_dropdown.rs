use eframe::egui;
use egui_phosphor::regular::{
    ARROW_COUNTER_CLOCKWISE, EYE, EYE_SLASH, GIT_PULL_REQUEST, GLOBE, LINK, PENCIL_SIMPLE, TRASH,
};

pub fn show(
    _ui: &mut egui::Ui,
    remote_name: &str,
    current_branch_name: &str,
    dropdown_resp: &egui::Response,
) {
    egui::Popup::menu(dropdown_resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(260.0);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);

            let menu_item = |ui: &mut egui::Ui, icon: &str, text: &str| -> egui::Response {
                ui.selectable_label(
                    false,
                    egui::RichText::new(format!("{}  {}", icon, text)).size(11.0),
                )
            };

            // Group 1
            if menu_item(
                ui,
                ARROW_COUNTER_CLOCKWISE,
                &format!("Fetch {}", remote_name),
            )
            .clicked()
            {
                ui.close();
            }

            ui.separator();

            // Group 2
            if menu_item(ui, PENCIL_SIMPLE, &format!("Edit {}", remote_name)).clicked() {
                ui.close();
            }
            if menu_item(ui, TRASH, &format!("Remove {}", remote_name)).clicked() {
                ui.close();
            }

            ui.separator();

            // Group 3
            if menu_item(
                ui,
                GIT_PULL_REQUEST,
                &format!(
                    "Start a pull request to {} from {}/{}",
                    remote_name, remote_name, current_branch_name
                ),
            )
            .clicked()
            {
                ui.close();
            }
            if menu_item(ui, GLOBE, &format!("View {} on GitHub.com", remote_name)).clicked() {
                ui.close();
            }

            ui.separator();

            // Group 4
            if menu_item(ui, EYE_SLASH, "Hide").clicked() {
                ui.close();
            }
            if menu_item(ui, EYE, "Solo").clicked() {
                ui.close();
            }

            ui.separator();

            // Group 5
            if menu_item(ui, LINK, &format!("Copy link to remote: {}", remote_name)).clicked() {
                ui.close();
            }
        });
}
