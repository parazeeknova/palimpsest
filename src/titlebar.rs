use eframe::egui;
use egui_phosphor::regular::{
    ARROW_LEFT, ARROW_RIGHT, GEAR_SIX, LIST, MAGNIFYING_GLASS, MINUS, SQUARE, USER_CIRCLE, X,
};

pub fn show(
    ui: &mut egui::Ui,
    _frame: &mut eframe::Frame,
    menu_open: &mut bool,
    search_query: &mut String,
) {
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

    let logo = egui::Image::new(egui::include_image!("assets/logo.svg"))
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
                    ui.menu_button(egui::RichText::new("File").size(12.0), |ui| {
                        if ui.button("Open repository").clicked() {
                            ui.close();
                        }
                        if ui.button("New repository").clicked() {
                            ui.close();
                        }
                        if ui.button("Exit").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });

                    ui.menu_button(egui::RichText::new("Window").size(12.0), |ui| {
                        if ui.button("Minimize").clicked() {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                        if ui.button("Maximize").clicked() {
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::Maximized(true));
                        }
                        if ui.button("Close").clicked() {
                            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });

                    ui.menu_button(egui::RichText::new("Help").size(12.0), |ui| {
                        ui.label("Palimpsest");
                        ui.label("Local-first git client");
                    });
                });
            }

            let search_width = (rect.width() * 0.25).clamp(140.0, 240.0);
            let group_width = search_width + 60.0;
            let spacer = ((ui.available_width() - group_width).max(0.0)) * 0.5;
            ui.add_space(spacer);

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
                            .hint_text("Search anything...")
                            .frame(egui::Frame::NONE)
                            .background_color(bg_fill),
                    );
                    ui.painter().line_segment(
                        [
                            edit_response.rect.left_bottom(),
                            edit_response.rect.right_bottom(),
                        ],
                        egui::Stroke::new(1.0, visuals.bg_stroke.color),
                    );
                },
            );

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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

                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(GEAR_SIX).size(14.0))
                            .min_size(egui::vec2(18.0, 18.0)),
                    )
                    .clicked()
                {
                    ui.close();
                }

                if ui
                    .add(
                        egui::Button::new(egui::RichText::new(USER_CIRCLE).size(14.0))
                            .min_size(egui::vec2(18.0, 18.0)),
                    )
                    .clicked()
                {
                    ui.close();
                }
            });
        },
    );
}
