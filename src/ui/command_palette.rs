use eframe::egui;
use egui::Key;
use egui::KeyboardShortcut;
use egui::Modifiers;
use egui::containers::modal::Modal;
use egui_phosphor::regular::{
    ARROW_CLOCKWISE, ARROW_COUNTER_CLOCKWISE, FILE_PLUS, FOLDER, GIT_FORK, GIT_PULL_REQUEST, POWER,
    TERMINAL_WINDOW, TRASH,
};

fn primary_modifiers() -> Modifiers {
    if cfg!(target_os = "macos") {
        Modifiers::COMMAND
    } else {
        Modifiers::CTRL
    }
}

#[derive(Clone, Debug)]
pub enum QuickLaunchAction {
    OpenRepository,
    ExitApp,
    OpenLogs,
    Fetch,
    Pull,
    Push,
    StageAll,
    DiscardAll,
    CreateBranch,
}

#[derive(Clone)]
struct CommandEntry {
    icon: &'static str,
    label: &'static str,
    keywords: &'static str,
    action: QuickLaunchAction,
    requires_repo: bool,
    shortcut: Option<&'static str>,
}

fn all_commands() -> Vec<CommandEntry> {
    if cfg!(target_os = "macos") {
        vec![
            CommandEntry {
                icon: FOLDER,
                label: "Open Repository",
                keywords: "open repo folder directory pick",
                action: QuickLaunchAction::OpenRepository,
                requires_repo: false,
                shortcut: Some("⌘O"),
            },
            CommandEntry {
                icon: POWER,
                label: "Exit App",
                keywords: "exit quit close shutdown",
                action: QuickLaunchAction::ExitApp,
                requires_repo: false,
                shortcut: Some("⌘Q"),
            },
            CommandEntry {
                icon: TERMINAL_WINDOW,
                label: "Open Logs",
                keywords: "logs debug console trace info error",
                action: QuickLaunchAction::OpenLogs,
                requires_repo: false,
                shortcut: Some("⇧⌘L"),
            },
            CommandEntry {
                icon: ARROW_COUNTER_CLOCKWISE,
                label: "Fetch",
                keywords: "fetch remote update",
                action: QuickLaunchAction::Fetch,
                requires_repo: true,
                shortcut: None,
            },
            CommandEntry {
                icon: ARROW_CLOCKWISE,
                label: "Pull",
                keywords: "pull download sync",
                action: QuickLaunchAction::Pull,
                requires_repo: true,
                shortcut: None,
            },
            CommandEntry {
                icon: GIT_PULL_REQUEST,
                label: "Push",
                keywords: "push upload sync",
                action: QuickLaunchAction::Push,
                requires_repo: true,
                shortcut: None,
            },
            CommandEntry {
                icon: FILE_PLUS,
                label: "Stage All",
                keywords: "stage add all files changes",
                action: QuickLaunchAction::StageAll,
                requires_repo: true,
                shortcut: None,
            },
            CommandEntry {
                icon: TRASH,
                label: "Discard All",
                keywords: "discard reset undo changes",
                action: QuickLaunchAction::DiscardAll,
                requires_repo: true,
                shortcut: None,
            },
            CommandEntry {
                icon: GIT_FORK,
                label: "Create Branch",
                keywords: "branch new create fork",
                action: QuickLaunchAction::CreateBranch,
                requires_repo: true,
                shortcut: None,
            },
        ]
    } else {
        vec![
            CommandEntry {
                icon: FOLDER,
                label: "Open Repository",
                keywords: "open repo folder directory pick",
                action: QuickLaunchAction::OpenRepository,
                requires_repo: false,
                shortcut: Some("Ctrl+O"),
            },
            CommandEntry {
                icon: POWER,
                label: "Exit App",
                keywords: "exit quit close shutdown",
                action: QuickLaunchAction::ExitApp,
                requires_repo: false,
                shortcut: Some("Ctrl+Q"),
            },
            CommandEntry {
                icon: TERMINAL_WINDOW,
                label: "Open Logs",
                keywords: "logs debug console trace info error",
                action: QuickLaunchAction::OpenLogs,
                requires_repo: false,
                shortcut: Some("Ctrl+Shift+L"),
            },
            CommandEntry {
                icon: ARROW_COUNTER_CLOCKWISE,
                label: "Fetch",
                keywords: "fetch remote update",
                action: QuickLaunchAction::Fetch,
                requires_repo: true,
                shortcut: None,
            },
            CommandEntry {
                icon: ARROW_CLOCKWISE,
                label: "Pull",
                keywords: "pull download sync",
                action: QuickLaunchAction::Pull,
                requires_repo: true,
                shortcut: None,
            },
            CommandEntry {
                icon: GIT_PULL_REQUEST,
                label: "Push",
                keywords: "push upload sync",
                action: QuickLaunchAction::Push,
                requires_repo: true,
                shortcut: None,
            },
            CommandEntry {
                icon: FILE_PLUS,
                label: "Stage All",
                keywords: "stage add all files changes",
                action: QuickLaunchAction::StageAll,
                requires_repo: true,
                shortcut: None,
            },
            CommandEntry {
                icon: TRASH,
                label: "Discard All",
                keywords: "discard reset undo changes",
                action: QuickLaunchAction::DiscardAll,
                requires_repo: true,
                shortcut: None,
            },
            CommandEntry {
                icon: GIT_FORK,
                label: "Create Branch",
                keywords: "branch new create fork",
                action: QuickLaunchAction::CreateBranch,
                requires_repo: true,
                shortcut: None,
            },
        ]
    }
}

fn matches_query(query: &str, entry: &CommandEntry) -> bool {
    if query.is_empty() {
        return true;
    }
    let q = query.to_lowercase();
    entry.label.to_lowercase().contains(&q) || entry.keywords.contains(&q)
}

#[derive(Default)]
pub struct State {
    query: String,
    selected_index: usize,
}

pub enum PaletteResult {
    StillOpen,
    Closed,
    Action(QuickLaunchAction),
}

pub fn show(ctx: &egui::Context, state: &mut State, has_repo: bool) -> PaletteResult {
    let commands = all_commands();
    let filtered: Vec<&CommandEntry> = commands
        .iter()
        .filter(|c| matches_query(&state.query, c) && (!c.requires_repo || has_repo))
        .collect();

    if filtered.is_empty() {
        state.selected_index = 0;
    } else if state.selected_index >= filtered.len() {
        state.selected_index = filtered.len().saturating_sub(1);
    }

    let modal = Modal::new(egui::Id::new("command_palette"))
        .backdrop_color(egui::Color32::from_black_alpha(140));

    let response = modal.show(ctx, |ui| {
        ui.set_min_width(520.0);
        ui.set_max_width(520.0);

        ui.add_space(2.0);

        let search_height = 30.0;
        let section_fill = egui::Color32::from_rgb(40, 40, 40);
        let section_stroke = egui::Stroke::new(1.0_f32, egui::Color32::from_rgb(72, 72, 72));
        let editor_fill = egui::Color32::from_rgb(49, 49, 49);

        let (search_rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), search_height),
            egui::Sense::hover(),
        );

        ui.painter().rect_filled(search_rect, 6, section_fill);
        ui.painter()
            .rect_stroke(search_rect, 6, section_stroke, egui::StrokeKind::Inside);

        let shortcut_label = if cfg!(target_os = "macos") {
            "⌘K"
        } else {
            "Ctrl+K"
        };
        let shortcut_pos = egui::pos2(search_rect.right() - 10.0, search_rect.center().y);
        ui.painter().text(
            shortcut_pos,
            egui::Align2::RIGHT_CENTER,
            shortcut_label,
            egui::FontId::proportional(13.0),
            egui::Color32::from_rgb(120, 120, 120),
        );

        let text_edit_rect = egui::Rect::from_min_size(
            egui::pos2(search_rect.left() + 8.0, search_rect.top() + 8.0),
            egui::vec2(search_rect.width() - 50.0, search_rect.height() - 4.0),
        );
        let search_response =
            ui.scope_builder(egui::UiBuilder::new().max_rect(text_edit_rect), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut state.query)
                        .hint_text("Type a command...")
                        .desired_width(text_edit_rect.width())
                        .font(egui::FontId::proportional(15.0))
                        .frame(egui::Frame::NONE)
                        .background_color(editor_fill),
                )
            });
        ui.add_space(2.0);

        if search_response.inner.changed() {
            state.selected_index = 0;
        }

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            return None;
        }

        if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
            state.selected_index = (state.selected_index + 1).min(filtered.len().saturating_sub(1));
        }

        if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
            state.selected_index = state.selected_index.saturating_sub(1);
        }

        if ctx.input(|i| i.key_pressed(Key::Enter)) {
            if let Some(entry) = filtered.get(state.selected_index) {
                return Some(entry.action.clone());
            }
        }

        ui.add_space(8.0);

        let visible_count = filtered.len().min(15);
        let list_height = visible_count as f32 * 32.0;
        egui::ScrollArea::vertical()
            .max_height(list_height)
            .id_salt(format!("cmd_list_{}", filtered.len()))
            .auto_shrink([true, true])
            .show(ui, |ui| {
                for (i, entry) in filtered.iter().enumerate() {
                    let is_selected = i == state.selected_index;
                    let row_response = ui.add(CommandRow {
                        icon: entry.icon,
                        label: entry.label,
                        shortcut: entry.shortcut,
                        is_selected,
                    });

                    if row_response.clicked() {
                        return Some(entry.action.clone());
                    }

                    if is_selected {
                        row_response.scroll_to_me(Some(egui::Align::Center));
                    }
                }
                None
            })
            .inner
    });

    if response.should_close() {
        state.query.clear();
        state.selected_index = 0;
        return PaletteResult::Closed;
    }

    if let Some(action) = response.inner {
        state.query.clear();
        state.selected_index = 0;
        return PaletteResult::Action(action);
    }

    PaletteResult::StillOpen
}

pub fn check_shortcut(ctx: &egui::Context) -> bool {
    let shortcut = KeyboardShortcut::new(primary_modifiers(), Key::K);
    ctx.input_mut(|i| i.consume_shortcut(&shortcut))
}

struct CommandRow<'a> {
    icon: &'a str,
    label: &'a str,
    shortcut: Option<&'a str>,
    is_selected: bool,
}

impl egui::Widget for CommandRow<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let height = 32.0;
        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), height),
            egui::Sense::click(),
        );

        if self.is_selected {
            ui.painter()
                .rect_filled(rect, 4.0, egui::Color32::from_rgb(52, 52, 52));
        }

        let icon_x = rect.left() + 12.0;
        let icon_y = rect.center().y;
        ui.painter().text(
            egui::pos2(icon_x, icon_y),
            egui::Align2::CENTER_CENTER,
            self.icon,
            egui::FontId::proportional(16.0),
            ui.visuals().text_color(),
        );

        let label_x = icon_x + 28.0;
        ui.painter().text(
            egui::pos2(label_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            self.label,
            egui::FontId::proportional(13.0),
            ui.visuals().text_color(),
        );

        if let Some(shortcut) = self.shortcut {
            let shortcut_x = rect.right() - 12.0;
            ui.painter().text(
                egui::pos2(shortcut_x, rect.center().y),
                egui::Align2::RIGHT_CENTER,
                shortcut,
                egui::FontId::proportional(13.0),
                egui::Color32::from_rgb(120, 120, 120),
            );
        }

        response
    }
}
