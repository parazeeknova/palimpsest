use crate::state::CachedBranch;
use eframe::egui;
use egui_phosphor::regular::{
    ARROW_COUNTER_CLOCKWISE, ARROW_DOWN, ARROW_LEFT, ARROW_UP, BOOKMARK, CLOUD_ARROW_UP, COPY, EYE,
    EYE_SLASH, FILE_TEXT, GIT_PULL_REQUEST, INFO, LINK, PENCIL_SIMPLE, PLUS, PUSH_PIN, SPARKLE,
    TAG, TRASH,
};

pub fn show(_ui: &mut egui::Ui, branch: &CachedBranch, dropdown_resp: &egui::Response) {
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
            if menu_item(ui, ARROW_DOWN, "Pull (fast-forward if possible)").clicked() {
                ui.close();
            }
            if menu_item(ui, ARROW_UP, "Push").clicked() {
                ui.close();
            }
            if menu_item(ui, CLOUD_ARROW_UP, "Set Upstream").clicked() {
                ui.close();
            }

            ui.separator();

            // Group 2
            if menu_item(ui, PLUS, "Create branch here").clicked() {
                ui.close();
            }
            if menu_item(
                ui,
                ARROW_COUNTER_CLOCKWISE,
                &format!("Reset {} to this commit", branch.name),
            )
            .clicked()
            {
                ui.close();
            }
            if menu_item(ui, PENCIL_SIMPLE, "Edit commit message").clicked() {
                ui.close();
            }
            if menu_item(ui, ARROW_LEFT, "Revert commit").clicked() {
                ui.close();
            }

            ui.separator();

            // Group 3
            if menu_item(ui, SPARKLE, "Recompose commit with AI (Preview)").clicked() {
                ui.close();
            }
            if menu_item(ui, TRASH, "Drop commit").clicked() {
                ui.close();
            }
            if menu_item(ui, ARROW_DOWN, "Move commit down").clicked() {
                ui.close();
            }

            ui.separator();

            // Group 4
            if menu_item(
                ui,
                GIT_PULL_REQUEST,
                &format!("Start a pull request to origin from origin/{}", branch.name),
            )
            .clicked()
            {
                ui.close();
            }
            if menu_item(ui, INFO, "Explain Branch Changes (Preview)").clicked() {
                ui.close();
            }

            ui.separator();

            // Group 5
            if menu_item(ui, FILE_TEXT, "Apply patch").clicked() {
                ui.close();
            }
            if menu_item(ui, PENCIL_SIMPLE, &format!("Rename {}", branch.name)).clicked() {
                ui.close();
            }
            if menu_item(ui, TRASH, &format!("Delete {}", branch.name)).clicked() {
                ui.close();
            }

            ui.separator();

            // Group 6
            if menu_item(ui, COPY, "Copy branch name").clicked() {
                ui.close();
            }
            if menu_item(ui, COPY, "Copy commit sha").clicked() {
                ui.close();
            }
            if menu_item(
                ui,
                LINK,
                &format!("Copy link to branch: origin/{}", branch.name),
            )
            .clicked()
            {
                ui.close();
            }
            if menu_item(ui, LINK, "Copy link to this commit on remote: origin").clicked() {
                ui.close();
            }

            ui.separator();

            // Group 7
            if menu_item(ui, EYE_SLASH, "Hide").clicked() {
                ui.close();
            }
            if menu_item(ui, PUSH_PIN, "Pin to Left").clicked() {
                ui.close();
            }
            if menu_item(ui, EYE, "Solo").clicked() {
                ui.close();
            }

            ui.separator();

            // Group 8
            if menu_item(ui, TAG, "Create tag here").clicked() {
                ui.close();
            }
            if menu_item(ui, BOOKMARK, "Create annotated tag here").clicked() {
                ui.close();
            }
        });
}
