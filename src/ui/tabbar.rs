use eframe::egui;
use egui_phosphor::regular::PLUS;

use crate::ui::sidebar::SIDEBAR_WIDTH;

const TABBAR_HEIGHT: f32 = 25.0;
const PLUS_WIDTH: f32 = 28.0;

struct Tab<'a> {
    title: &'a str,
    accent: Option<egui::Color32>,
    active: bool,
}

pub fn show(ui: &mut egui::Ui, repo_name: Option<&str>) {
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, TABBAR_HEIGHT), egui::Sense::hover());

    let stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 72, 72));
    let top_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(88, 88, 88));
    let bg_fill = egui::Color32::from_rgb(34, 34, 34);
    ui.painter().rect_filled(rect, 0.0, bg_fill);
    ui.painter()
        .line_segment([rect.left_top(), rect.right_top()], top_stroke);
    ui.painter()
        .line_segment([rect.left_bottom(), rect.right_bottom()], stroke);

    let tabs: Vec<Tab<'_>> = match repo_name {
        Some(name) => vec![Tab {
            title: name,
            accent: None,
            active: true,
        }],
        None => vec![],
    };

    if tabs.is_empty() {
        let plus_rect =
            egui::Rect::from_min_size(rect.left_top(), egui::vec2(PLUS_WIDTH, TABBAR_HEIGHT));
        paint_plus(ui, plus_rect, stroke);
        return;
    }

    let plus_rect = egui::Rect::from_min_max(
        egui::pos2(rect.right() - PLUS_WIDTH, rect.top()),
        rect.right_bottom(),
    );
    let tabs_rect = egui::Rect::from_min_max(rect.left_top(), plus_rect.left_bottom());

    let mut left = tabs_rect.left();
    for (index, tab) in tabs.iter().enumerate() {
        if left >= tabs_rect.right() {
            break;
        }

        let width = if index == 0 {
            SIDEBAR_WIDTH
        } else {
            let remaining_tabs = (tabs.len() - index) as f32;
            ((tabs_rect.right() - left) / remaining_tabs).max(112.0)
        };
        let right = (left + width).min(tabs_rect.right());
        let tab_rect = egui::Rect::from_min_max(
            egui::pos2(left, tabs_rect.top()),
            egui::pos2(right, tabs_rect.bottom()),
        );
        paint_tab(ui, tab_rect, tab, stroke);
        left = right;
    }

    paint_plus(ui, plus_rect, stroke);
}

fn paint_tab(ui: &mut egui::Ui, rect: egui::Rect, tab: &Tab<'_>, stroke: egui::Stroke) {
    let fill = if tab.active {
        egui::Color32::from_rgb(62, 62, 62)
    } else {
        egui::Color32::from_rgb(38, 38, 38)
    };

    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter()
        .line_segment([rect.right_top(), rect.right_bottom()], stroke);

    if let Some(color) = tab.accent {
        ui.painter()
            .circle_filled(egui::pos2(rect.left() + 12.0, rect.center().y), 4.0, color);
    }

    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        tab.title,
        egui::FontId::proportional(13.0),
        ui.visuals().text_color(),
    );
}

fn paint_plus(ui: &mut egui::Ui, rect: egui::Rect, stroke: egui::Stroke) {
    ui.painter()
        .line_segment([rect.left_top(), rect.left_bottom()], stroke);
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        PLUS,
        egui::FontId::proportional(20.0),
        ui.visuals().text_color(),
    );
}
