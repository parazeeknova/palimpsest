use eframe::egui;
use std::sync::Arc;

mod components;
mod state;

use components::{body, commit_panel, sidebar, tabbar, titlebar, toolbar};
use state::{AppAction, AppStore};

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
    store: Arc<AppStore>,
    titlebar_menu_open: bool,
    search_query: String,
    body_state: body::State,
    commit_panel_state: commit_panel::State,
}

impl PalimpsestApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        egui_extras::install_image_loaders(&cc.egui_ctx);

        let store = state::create_store();

        Self {
            store,
            titlebar_menu_open: false,
            search_query: String::new(),
            body_state: body::State::default(),
            commit_panel_state: commit_panel::State::default(),
        }
    }

    fn repo_name(&self) -> Option<String> {
        self.store.get_state().repo_name().map(|s| s.to_owned())
    }
}

impl eframe::App for PalimpsestApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let background = ui.visuals().widgets.inactive.bg_fill;
        ui.painter().rect_filled(ui.max_rect(), 0.0, background);

        let repo_name = self.repo_name();
        let repo_name_ref = repo_name.as_deref();
        let mut show_window_buttons = self.store.get_state().show_window_buttons;

        let open_repo = titlebar::show(
            ui,
            frame,
            &mut self.titlebar_menu_open,
            &mut self.search_query,
            repo_name_ref,
            &self.store.get_state().recent_repos,
            &mut show_window_buttons,
        );

        if show_window_buttons != self.store.get_state().show_window_buttons {
            self.store
                .dispatch(AppAction::ToggleWindowButtons(show_window_buttons));
        }

        match open_repo {
            titlebar::OpenAction::PickFolder => {
                if let Some(path) = rfd::FileDialog::new().pick_folder() {
                    self.store
                        .dispatch(AppAction::OpenRepo(path.to_string_lossy().to_string()));
                }
            }
            titlebar::OpenAction::SelectRecent(index) => {
                self.store.dispatch(AppAction::SelectRecent(index));
            }
            titlebar::OpenAction::None => {}
        }

        let repo_name = self.repo_name();
        toolbar::show(ui, repo_name.as_deref());
        tabbar::show(ui, repo_name.as_deref());

        let content_rect = ui.available_rect_before_wrap();
        let (content_rect, _) = ui.allocate_exact_size(content_rect.size(), egui::Sense::hover());
        let sidebar_rect = egui::Rect::from_min_size(
            content_rect.left_top(),
            egui::vec2(sidebar::SIDEBAR_WIDTH, content_rect.height()),
        );
        let body_rect = egui::Rect::from_min_max(
            egui::pos2(sidebar_rect.right(), content_rect.top()),
            content_rect.right_bottom(),
        );

        let repo_name = self.repo_name();
        ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt("app_sidebar")
                .max_rect(sidebar_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| sidebar::show(ui, repo_name.as_deref()),
        );
        ui.scope_builder(
            egui::UiBuilder::new()
                .id_salt("app_body")
                .max_rect(body_rect)
                .layout(egui::Layout::top_down(egui::Align::Min)),
            |ui| body::show(ui, &mut self.body_state, &mut self.commit_panel_state),
        );
    }
}
