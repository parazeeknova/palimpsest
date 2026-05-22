use eframe::egui;
use egui_phosphor::regular::{
    ARROW_CLOCKWISE, ARROW_COUNTER_CLOCKWISE, ARROWS_CLOCKWISE, BROWSERS, CARET_DOWN, FOLDER,
    GIT_BRANCH, GIT_FORK, GIT_PULL_REQUEST, SIDEBAR, STACK, TERMINAL_WINDOW, TEXT_ALIGN_LEFT,
};

const TOOLBAR_HEIGHT: f32 = 46.0;
const CENTER_WIDTH: f32 = 230.0;
const ACTION_WIDTH: f32 = 58.0;
const QUICK_ACTION_WIDTH: f32 = 76.0;
const ACTION_HEIGHT: f32 = 34.0;
const LEFT_ACTIONS: f32 = QUICK_ACTION_WIDTH + ACTION_WIDTH * 4.0;
const RIGHT_ACTIONS: f32 = ACTION_WIDTH * 6.0;

pub fn show(ui: &mut egui::Ui, repo_name: Option<&str>, current_branch: Option<&str>) -> bool {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, TOOLBAR_HEIGHT), egui::Sense::hover());

    let visuals = ui.visuals().widgets.inactive;
    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 72, 72));
    let top_edge_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(78, 78, 78));
    ui.painter().rect_filled(rect, 0.0, visuals.bg_fill);
    ui.painter()
        .line_segment([rect.left_top(), rect.right_top()], top_edge_stroke);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], stroke);

    let (left_rect, center_rect, right_rect) = section_rects(rect);
    let center_fill = egui::Color32::from_rgb(43, 43, 43);
    ui.painter().rect_filled(center_rect, 0.0, center_fill);
    ui.painter()
        .line_segment([center_rect.left_top(), center_rect.left_bottom()], stroke);
    ui.painter().line_segment(
        [center_rect.right_top(), center_rect.right_bottom()],
        stroke,
    );

    let quick_launch_clicked = child_ui(
        ui,
        left_rect.shrink2(egui::vec2(8.0, 3.0)),
        "toolbar_left",
        egui::Layout::left_to_right(egui::Align::Center),
        left_panel,
    )
    .inner;
    child_ui(
        ui,
        center_rect.shrink2(egui::vec2(8.0, 2.0)),
        "toolbar_center",
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| center_panel(ui, repo_name, current_branch),
    );
    child_ui(
        ui,
        right_rect.shrink2(egui::vec2(8.0, 3.0)),
        "toolbar_right",
        egui::Layout::right_to_left(egui::Align::Center),
        right_panel,
    );
    quick_launch_clicked
}

fn section_rects(rect: egui::Rect) -> (egui::Rect, egui::Rect, egui::Rect) {
    let center_width = CENTER_WIDTH.min((rect.width() * 0.32).max(180.0));
    let preferred_left = rect.center().x - center_width * 0.5;
    let min_left = rect.left() + LEFT_ACTIONS.min(rect.width() * 0.36);
    let max_left = rect.right() - RIGHT_ACTIONS.min(rect.width() * 0.42) - center_width;
    let center_left = if min_left <= max_left {
        preferred_left.clamp(min_left, max_left)
    } else {
        preferred_left.clamp(rect.left(), rect.right() - center_width)
    };

    let center_rect = egui::Rect::from_min_size(
        egui::pos2(center_left, rect.top()),
        egui::vec2(center_width, rect.height()),
    );
    let left_rect = egui::Rect::from_min_max(rect.left_top(), center_rect.left_bottom());
    let right_rect = egui::Rect::from_min_max(center_rect.right_top(), rect.right_bottom());

    (left_rect, center_rect, right_rect)
}

fn child_ui<R>(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    id_salt: &'static str,
    layout: egui::Layout,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    ui.scope_builder(
        egui::UiBuilder::new()
            .id_salt(id_salt)
            .max_rect(rect)
            .layout(layout),
        add_contents,
    )
}

fn left_panel(ui: &mut egui::Ui) -> bool {
    ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
    let quick_launch_clicked = toolbar_button(ui, QUICK_ACTION_WIDTH, FOLDER, "Quick Launch", None);
    toolbar_button(ui, ACTION_WIDTH, ARROW_COUNTER_CLOCKWISE, "Fetch", None);
    toolbar_button(ui, ACTION_WIDTH, ARROW_CLOCKWISE, "Pull", None);
    toolbar_button(ui, ACTION_WIDTH, GIT_PULL_REQUEST, "Push", None);
    toolbar_menu_button(ui, ACTION_WIDTH, STACK, "Stash", Some(CARET_DOWN), |ui| {
        drop(ui.button("Stash changes"));
        drop(ui.button("Apply stash"));
        drop(ui.button("Pop stash"));
    });
    quick_launch_clicked
}

fn center_panel(ui: &mut egui::Ui, repo_name: Option<&str>, current_branch: Option<&str>) {
    let rect = ui.max_rect();
    let group_rect = egui::Rect::from_center_size(rect.center(), egui::vec2(200.0, ACTION_HEIGHT));
    let text_rect = egui::Rect::from_min_size(
        egui::pos2(group_rect.left() + 8.0, group_rect.top()),
        egui::vec2(184.0, ACTION_HEIGHT),
    );

    let menu_icon_rect = egui::Rect::from_min_size(
        egui::pos2(rect.left(), rect.bottom() - 20.0),
        egui::vec2(16.0, 16.0),
    );
    ui.painter().text(
        menu_icon_rect.center(),
        egui::Align2::CENTER_CENTER,
        TEXT_ALIGN_LEFT,
        egui::FontId::proportional(14.0),
        ui.visuals().text_color(),
    );

    child_ui(
        ui,
        text_rect,
        "toolbar_center_text",
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            if let Some(name) = repo_name {
                ui.add(
                    egui::Label::new(egui::RichText::new(name).size(13.0).strong())
                        .truncate()
                        .halign(egui::Align::Center),
                );
                let branch_name = current_branch.unwrap_or("no branch");
                let branch_text = format!("{} {}", GIT_BRANCH, branch_name);
                ui.add_space(3.0);
                let mut rich_text = egui::RichText::new(branch_text).size(10.0);
                if ui.rect_contains_pointer(text_rect) {
                    rich_text = rich_text.underline();
                }
                ui.add(
                    egui::Label::new(rich_text)
                        .truncate()
                        .halign(egui::Align::Center),
                );
            } else {
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("Welcome to Palimpsest!")
                            .size(12.0)
                            .strong(),
                    )
                    .truncate()
                    .halign(egui::Align::Center),
                );
                ui.add(
                    egui::Label::new(
                        egui::RichText::new("Open a repo to start")
                            .size(10.0)
                            .color(egui::Color32::from_rgb(140, 140, 140)),
                    )
                    .truncate()
                    .halign(egui::Align::Center),
                );
            }
        },
    );
}

fn right_panel(ui: &mut egui::Ui) {
    ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
    toolbar_button(ui, ACTION_WIDTH, BROWSERS, "Workspace", Some(CARET_DOWN));
    toolbar_button(ui, ACTION_WIDTH, SIDEBAR, "Appearance", Some(CARET_DOWN));
    toolbar_button(ui, ACTION_WIDTH, TERMINAL_WINDOW, "Console", None);
    toolbar_button(
        ui,
        ACTION_WIDTH,
        ARROWS_CLOCKWISE,
        "Open in",
        Some(CARET_DOWN),
    );
    toolbar_button(ui, ACTION_WIDTH, GIT_FORK, "New Branch", None);
}

fn toolbar_button(
    ui: &mut egui::Ui,
    width: f32,
    icon: &str,
    label: &str,
    suffix: Option<&str>,
) -> bool {
    let response = ui.allocate_ui_with_layout(
        egui::vec2(width, ACTION_HEIGHT),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.add_sized(
                [width, 20.0],
                IconRow {
                    icon,
                    suffix,
                    icon_size: 16.0,
                },
            );
            ui.add_sized(
                [width, 12.0],
                CenteredText {
                    text: label,
                    size: 10.0,
                },
            );
        },
    );
    let interacted = response.response.interact(egui::Sense::click());
    if interacted.hovered() {
        ui.painter().rect_filled(
            response.response.rect,
            4.0,
            egui::Color32::from_white_alpha(18),
        );
    }
    interacted.clicked()
}

fn toolbar_menu_button(
    ui: &mut egui::Ui,
    width: f32,
    icon: &str,
    label: &str,
    suffix: Option<&str>,
    add_contents: impl FnOnce(&mut egui::Ui),
) {
    let response = ui.allocate_ui_with_layout(
        egui::vec2(width, ACTION_HEIGHT),
        egui::Layout::top_down(egui::Align::Center),
        |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);
            ui.add_sized(
                [width, 20.0],
                IconRow {
                    icon,
                    suffix,
                    icon_size: 16.0,
                },
            );
            ui.add_sized(
                [width, 12.0],
                CenteredText {
                    text: label,
                    size: 10.0,
                },
            );
        },
    );

    let popup_id = response.response.id.with("popup");
    let is_open = egui::Popup::is_id_open(ui.ctx(), popup_id);

    let interacted = response.response.interact(egui::Sense::click());
    if interacted.hovered() || is_open {
        ui.painter().rect_filled(
            response.response.rect,
            4.0,
            egui::Color32::from_white_alpha(18),
        );
    }

    egui::Popup::menu(&response.response)
        .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
        .show(add_contents);
}

struct IconRow<'a> {
    icon: &'a str,
    suffix: Option<&'a str>,
    icon_size: f32,
}

impl egui::Widget for IconRow<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        let text = if let Some(suffix) = self.suffix {
            format!("{} {}", self.icon, suffix)
        } else {
            self.icon.to_owned()
        };
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            text,
            egui::FontId::proportional(self.icon_size),
            ui.visuals().text_color(),
        );
        response
    }
}

struct CenteredText<'a> {
    text: &'a str,
    size: f32,
}

impl egui::Widget for CenteredText<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), egui::Sense::hover());
        ui.painter().text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            self.text,
            egui::FontId::proportional(self.size),
            ui.visuals().text_color(),
        );
        response
    }
}
