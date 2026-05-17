use eframe::egui;

mod titlebar;

fn main() -> eframe::Result {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Palimpsest")
            .with_inner_size([960.0, 720.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Palimpsest",
        native_options,
        Box::new(|cc| Ok(Box::new(PalimpsestApp::new(cc)))),
    )
}

struct PalimpsestApp {
    titlebar_menu_open: bool,
    search_query: String,
}

impl PalimpsestApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        Self {
            titlebar_menu_open: false,
            search_query: String::new(),
        }
    }
}

impl eframe::App for PalimpsestApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        titlebar::show(
            ui,
            frame,
            &mut self.titlebar_menu_open,
            &mut self.search_query,
        );
        egui::CentralPanel::default().show_inside(ui, |_ui| {});
    }
}
