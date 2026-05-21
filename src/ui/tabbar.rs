use eframe::egui;
use egui_phosphor::regular::{GIT_BRANCH, PLUS, X};

use crate::state::AppState;

const TABBAR_HEIGHT: f32 = 25.0;
const PLUS_WIDTH: f32 = 28.0;
const CLOSE_WIDTH: f32 = 18.0;
const TAB_MAX_WIDTH: f32 = 260.0;
const TAB_MIN_WIDTH: f32 = 120.0;

pub enum TabAction {
    Open,
    Activate(usize),
    Close(usize),
}

struct Tab {
    name: String,
    location: String,
    active: bool,
    closeable: bool,
}

pub fn show(ui: &mut egui::Ui, state: &AppState) -> Option<TabAction> {
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

    let tabs: Vec<Tab> = state
        .open_tabs
        .iter()
        .enumerate()
        .map(|(index, path)| Tab {
            name: repo_display_name(path).to_string(),
            location: repo_parent_location(path),
            active: state.active_tab == Some(index),
            closeable: true,
        })
        .collect();

    let plus_rect = egui::Rect::from_min_max(
        egui::pos2(rect.right() - PLUS_WIDTH, rect.top()),
        rect.right_bottom(),
    );
    let tabs_rect = egui::Rect::from_min_max(rect.left_top(), plus_rect.left_bottom());

    if tabs.is_empty() {
        let empty_tab_rect = egui::Rect::from_min_max(rect.left_top(), plus_rect.left_bottom());
        paint_empty_tab(ui, empty_tab_rect);
        paint_plus(ui, plus_rect, stroke);
        if ui
            .interact(
                empty_tab_rect,
                ui.make_persistent_id("tabbar_empty"),
                egui::Sense::click(),
            )
            .clicked()
            || ui
                .interact(
                    plus_rect,
                    ui.make_persistent_id("tabbar_open"),
                    egui::Sense::click(),
                )
                .clicked()
        {
            return Some(TabAction::Open);
        }
        return None;
    }

    let mut left = tabs_rect.left();
    for (index, tab) in tabs.iter().enumerate() {
        if left >= tabs_rect.right() {
            break;
        }

        let width = TAB_MAX_WIDTH
            .min(tabs_rect.right() - left)
            .max(TAB_MIN_WIDTH);
        let right = (left + width).min(tabs_rect.right());
        let tab_rect = egui::Rect::from_min_max(
            egui::pos2(left, tabs_rect.top()),
            egui::pos2(right, tabs_rect.bottom()),
        );
        if let Some(action) = paint_tab(ui, tab_rect, tab, index, stroke) {
            return Some(action);
        }
        left = right;
    }

    paint_plus(ui, plus_rect, stroke);
    if ui
        .interact(
            plus_rect,
            ui.make_persistent_id("tabbar_open"),
            egui::Sense::click(),
        )
        .clicked()
    {
        return Some(TabAction::Open);
    }

    None
}

fn paint_tab(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    tab: &Tab,
    index: usize,
    stroke: egui::Stroke,
) -> Option<TabAction> {
    let fill = if tab.active {
        egui::Color32::from_rgb(62, 62, 62)
    } else {
        egui::Color32::from_rgb(38, 38, 38)
    };

    ui.painter().rect_filled(rect, 0.0, fill);
    ui.painter()
        .line_segment([rect.right_top(), rect.right_bottom()], stroke);

    let close_x = rect.right() - CLOSE_WIDTH - 4.0;
    let close_rect = egui::Rect::from_min_size(
        egui::pos2(close_x, rect.center().y - 8.0),
        egui::vec2(CLOSE_WIDTH, 16.0),
    );

    let icon_x = rect.left() + 8.0;
    let icon_y = rect.center().y;
    ui.painter().text(
        egui::pos2(icon_x, icon_y),
        egui::Align2::LEFT_CENTER,
        GIT_BRANCH,
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgb(120, 120, 120),
    );

    let text_area = egui::Rect::from_min_max(
        egui::pos2(icon_x + 16.0, rect.top()),
        egui::pos2(close_rect.left() - 4.0, rect.bottom()),
    );

    let display = truncate_tab_text(&tab.name, &tab.location, text_area.width());

    let name_end = text_area.left() + display.name_len as f32 * 6.0;
    ui.painter().text(
        egui::pos2(text_area.left(), rect.center().y),
        egui::Align2::LEFT_CENTER,
        &display.name,
        egui::FontId::proportional(12.0),
        ui.visuals().text_color(),
    );
    let separator = "  ";
    let sep_width = separator.chars().count() as f32 * 6.0;
    ui.painter().text(
        egui::pos2(name_end + sep_width, rect.center().y),
        egui::Align2::LEFT_CENTER,
        &display.location,
        egui::FontId::proportional(12.0),
        egui::Color32::from_rgb(120, 120, 120),
    );

    let activate_response = ui.interact(
        rect,
        ui.make_persistent_id(("tabbar_tab", index)),
        egui::Sense::click(),
    );
    if activate_response.clicked() {
        return Some(TabAction::Activate(index));
    }

    if tab.closeable {
        ui.painter().text(
            close_rect.center(),
            egui::Align2::CENTER_CENTER,
            X,
            egui::FontId::proportional(10.0),
            ui.visuals().text_color(),
        );
        let close_response = ui.interact(
            close_rect,
            ui.make_persistent_id(("tabbar_close", index)),
            egui::Sense::click(),
        );
        if close_response.clicked() {
            return Some(TabAction::Close(index));
        }
    }

    None
}

struct TruncatedTabText {
    name: String,
    location: String,
    name_len: usize,
}

fn truncate_tab_text(name: &str, location: &str, max_width: f32) -> TruncatedTabText {
    let char_width = 6.0;
    let available = (max_width / char_width) as usize;
    let name_chars = name.chars().count();

    if available < 5 {
        return TruncatedTabText {
            name: truncate_chars(name, available.max(3)),
            location: String::new(),
            name_len: available.max(3),
        };
    }

    let loc_display = truncate_path_display(location);
    let loc_chars = loc_display.chars().count();

    if name_chars + loc_chars < available {
        return TruncatedTabText {
            name: name.to_string(),
            location: loc_display,
            name_len: name_chars,
        };
    }

    let name_budget = (available as i64 - loc_chars as i64 - 1).max(3) as usize;
    let truncated_name = truncate_chars(name, name_budget);

    TruncatedTabText {
        name: truncated_name,
        location: loc_display,
        name_len: name_budget,
    }
}

fn truncate_path_display(path: &str) -> String {
    if path.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = path.split(std::path::MAIN_SEPARATOR).collect();
    if parts.len() <= 2 {
        truncate_chars(path, 20)
    } else {
        let first = parts.first().copied().unwrap_or("");
        let last = parts.last().copied().unwrap_or("");
        let combined = format!("{}/…/{}", first, last);
        if combined.chars().count() > 24 {
            let last_truncated = truncate_chars(last, 14);
            format!("{}/…/{}", first, last_truncated)
        } else {
            combined
        }
    }
}

fn truncate_chars(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        let keep = max.saturating_sub(1);
        let truncated: String = s.chars().take(keep).collect();
        format!("{}…", truncated)
    }
}

fn paint_empty_tab(ui: &mut egui::Ui, rect: egui::Rect) {
    let fill = egui::Color32::from_rgb(62, 62, 62);
    ui.painter().rect_filled(rect, 0.0, fill);
}

fn paint_plus(ui: &mut egui::Ui, rect: egui::Rect, _stroke: egui::Stroke) {
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        PLUS,
        egui::FontId::proportional(20.0),
        ui.visuals().text_color(),
    );
}

fn repo_parent_location(path: &str) -> String {
    let p = std::path::Path::new(path);
    p.parent()
        .and_then(|parent| parent.file_name())
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default()
}

fn repo_display_name(path: &str) -> &str {
    std::path::Path::new(path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(path)
}
