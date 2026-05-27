use crate::state::CachedBranch;
use eframe::egui;
use egui_phosphor::regular::{
    ARROW_COUNTER_CLOCKWISE, ARROW_DOWN, ARROW_LEFT, ARROW_UP, BOOKMARK, COPY, EYE, EYE_SLASH,
    GIT_BRANCH, GIT_PULL_REQUEST, INFO, LINK, PENCIL_SIMPLE, PLUS, PUSH_PIN, SPARKLE, TAG, TRASH,
    TREE_VIEW,
};

pub fn show(
    _ui: &mut egui::Ui,
    branch: &CachedBranch,
    current_branch_name: &str,
    dropdown_resp: &egui::Response,
) {
    egui::Popup::menu(dropdown_resp)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(|ui| {
            ui.set_min_width(280.0);
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 2.0);

            let menu_item = |ui: &mut egui::Ui, icon: &str, text: &str| -> egui::Response {
                ui.selectable_label(
                    false,
                    egui::RichText::new(format!("{}  {}", icon, text)).size(11.0),
                )
            };

            let short_sha = if branch.tip_hash.len() >= 7 {
                &branch.tip_hash[..7]
            } else {
                &branch.tip_hash
            };

            // Group 1
            if menu_item(
                ui,
                ARROW_DOWN,
                &format!("Fast-forward {} to {}", branch.name, current_branch_name),
            )
            .clicked()
            {
                ui.close();
            }
            if menu_item(
                ui,
                GIT_BRANCH,
                &format!("Merge {} into {}", current_branch_name, branch.name),
            )
            .clicked()
            {
                ui.close();
            }
            if menu_item(
                ui,
                ARROW_UP,
                &format!("Rebase {} onto {}", current_branch_name, branch.name),
            )
            .clicked()
            {
                ui.close();
            }
            if menu_item(
                ui,
                ARROW_UP,
                &format!(
                    "Interactive Rebase {} onto {}",
                    current_branch_name, branch.name
                ),
            )
            .clicked()
            {
                ui.close();
            }

            ui.separator();

            // Group 2
            if menu_item(ui, GIT_BRANCH, &format!("Checkout {}", branch.name)).clicked() {
                ui.close();
            }

            ui.separator();

            // Group 3
            if menu_item(
                ui,
                TREE_VIEW,
                &format!("Create worktree from {}", branch.name),
            )
            .clicked()
            {
                ui.close();
            }

            ui.separator();

            // Group 4
            if menu_item(ui, PLUS, "Create branch here").clicked() {
                ui.close();
            }
            if menu_item(ui, ARROW_DOWN, "Cherry pick commit").clicked() {
                ui.close();
            }
            // Reset submenu/item
            if menu_item(
                ui,
                ARROW_COUNTER_CLOCKWISE,
                &format!("Reset {} to this commit", current_branch_name),
            )
            .clicked()
            {
                ui.close();
            }
            if menu_item(ui, ARROW_LEFT, "Revert commit").clicked() {
                ui.close();
            }

            ui.separator();

            // Group 5
            if menu_item(ui, SPARKLE, "Recompose commit with AI (Preview)").clicked() {
                ui.close();
            }
            if menu_item(
                ui,
                SPARKLE,
                &format!("Recompose 1 children of {} with AI (Preview)", short_sha),
            )
            .clicked()
            {
                ui.close();
            }
            if menu_item(
                ui,
                ARROW_UP,
                &format!("Interactive Rebase 1 children of {}", short_sha),
            )
            .clicked()
            {
                ui.close();
            }
            if menu_item(ui, PENCIL_SIMPLE, "Edit commit message").clicked() {
                ui.close();
            }
            if menu_item(ui, TRASH, "Drop commit").clicked() {
                ui.close();
            }
            if menu_item(ui, ARROW_UP, "Move commit up").clicked() {
                ui.close();
            }
            if menu_item(ui, ARROW_DOWN, "Move commit down").clicked() {
                ui.close();
            }

            ui.separator();

            // Group 6
            if menu_item(
                ui,
                GIT_PULL_REQUEST,
                &format!(
                    "Push {} and start a pull request to {}",
                    current_branch_name, branch.name
                ),
            )
            .clicked()
            {
                ui.close();
            }
            if menu_item(ui, INFO, "Explain Branch Changes (Preview)").clicked() {
                ui.close();
            }

            ui.separator();

            // Group 7
            if menu_item(ui, TRASH, &format!("Delete {}", branch.name)).clicked() {
                ui.close();
            }

            ui.separator();

            // Group 8
            if menu_item(ui, COPY, "Copy branch name").clicked() {
                ui.close();
            }
            if menu_item(ui, COPY, "Copy commit sha").clicked() {
                ui.close();
            }
            if menu_item(ui, LINK, &format!("Copy link to branch: {}", branch.name)).clicked() {
                ui.close();
            }
            // Extract remote name from branch name (e.g. "origin/dev" -> "origin")
            let remote_name = if let Some(pos) = branch.name.find('/') {
                &branch.name[..pos]
            } else {
                "origin"
            };
            if menu_item(
                ui,
                LINK,
                &format!("Copy link to this commit on remote: {}", remote_name),
            )
            .clicked()
            {
                ui.close();
            }

            ui.separator();

            // Group 9
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

            // Group 10
            if menu_item(ui, TAG, "Create tag here").clicked() {
                ui.close();
            }
            if menu_item(ui, BOOKMARK, "Create annotated tag here").clicked() {
                ui.close();
            }
        });
}
