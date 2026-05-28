use crate::state::{AppState, CachedBranch, CachedStash};
use eframe::egui;
use egui::Key;
use egui::KeyboardShortcut;
use egui::Modifiers;
use egui::containers::modal::Modal;
use egui_phosphor::regular::{
    ARROW_CLOCKWISE, ARROW_COUNTER_CLOCKWISE, FILE_PLUS, FOLDER, GEAR, GIT_BRANCH, GIT_FORK,
    GIT_PULL_REQUEST, MAGNIFYING_GLASS, POWER, TAG, TERMINAL_WINDOW, TRASH,
};

fn primary_modifiers() -> Modifiers {
    if cfg!(target_os = "macos") {
        Modifiers::COMMAND
    } else {
        Modifiers::CTRL
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum QuickLaunchAction {
    OpenRepository,
    ExitApp,
    OpenLogs,
    Refresh,
    NextTab,
    PreviousTab,
    Fetch,
    Pull,
    Push,
    StageAll,
    UnstageAll,
    DiscardAll,
    CreateBranch,
    CreateTag,
    SaveStash,
    CheckoutBranch(String),
    DeleteBranch(String),
    ApplyStash(usize),
    PopStash(usize),
    DropStash(usize),
}

#[derive(Clone)]
struct CommandEntry {
    icon: &'static str,
    label: String,
    keywords: String,
    action: QuickLaunchAction,
    requires_repo: bool,
    shortcut: Option<&'static str>,
}

fn fixed_commands() -> Vec<CommandEntry> {
    let mut commands = Vec::new();

    let push = |commands: &mut Vec<CommandEntry>,
                icon: &'static str,
                label: &str,
                keywords: &str,
                action: QuickLaunchAction,
                requires_repo: bool,
                shortcut: Option<&'static str>| {
        commands.push(CommandEntry {
            icon,
            label: label.to_string(),
            keywords: keywords.to_string(),
            action,
            requires_repo,
            shortcut,
        });
    };

    if cfg!(target_os = "macos") {
        push(
            &mut commands,
            FOLDER,
            "Open Repository",
            "open repo folder directory pick",
            QuickLaunchAction::OpenRepository,
            false,
            Some("⌘O"),
        );
        push(
            &mut commands,
            POWER,
            "Exit App",
            "exit quit close shutdown",
            QuickLaunchAction::ExitApp,
            false,
            Some("⌘Q"),
        );
        push(
            &mut commands,
            TERMINAL_WINDOW,
            "Open Logs",
            "logs debug console trace info error",
            QuickLaunchAction::OpenLogs,
            false,
            Some("⇧⌘L"),
        );
        push(
            &mut commands,
            GIT_BRANCH,
            "Next Tab",
            "tab next forward switch",
            QuickLaunchAction::NextTab,
            false,
            Some("⌘Tab"),
        );
        push(
            &mut commands,
            GIT_BRANCH,
            "Previous Tab",
            "tab prev back switch",
            QuickLaunchAction::PreviousTab,
            false,
            Some("⌘Shift+Tab"),
        );
    } else {
        push(
            &mut commands,
            FOLDER,
            "Open Repository",
            "open repo folder directory pick",
            QuickLaunchAction::OpenRepository,
            false,
            Some("Ctrl+O"),
        );
        push(
            &mut commands,
            POWER,
            "Exit App",
            "exit quit close shutdown",
            QuickLaunchAction::ExitApp,
            false,
            Some("Ctrl+Q"),
        );
        push(
            &mut commands,
            TERMINAL_WINDOW,
            "Open Logs",
            "logs debug console trace info error",
            QuickLaunchAction::OpenLogs,
            false,
            Some("Ctrl+Shift+L"),
        );
        push(
            &mut commands,
            GIT_BRANCH,
            "Next Tab",
            "tab next forward switch",
            QuickLaunchAction::NextTab,
            false,
            Some("Ctrl+Tab"),
        );
        push(
            &mut commands,
            GIT_BRANCH,
            "Previous Tab",
            "tab prev back switch",
            QuickLaunchAction::PreviousTab,
            false,
            Some("Ctrl+Shift+Tab"),
        );
    }

    push(
        &mut commands,
        ARROW_COUNTER_CLOCKWISE,
        "Fetch",
        "fetch remote update sync",
        QuickLaunchAction::Fetch,
        true,
        Some(if cfg!(target_os = "macos") {
            "⇧⌘F"
        } else {
            "Ctrl+Shift+F"
        }),
    );
    push(
        &mut commands,
        ARROW_CLOCKWISE,
        "Pull",
        "pull download sync merge",
        QuickLaunchAction::Pull,
        true,
        Some(if cfg!(target_os = "macos") {
            "⇧⌘U"
        } else {
            "Ctrl+Shift+U"
        }),
    );
    push(
        &mut commands,
        GIT_PULL_REQUEST,
        "Push",
        "push upload sync publish",
        QuickLaunchAction::Push,
        true,
        Some(if cfg!(target_os = "macos") {
            "⇧⌘P"
        } else {
            "Ctrl+Shift+P"
        }),
    );
    push(
        &mut commands,
        FILE_PLUS,
        "Stage All",
        "stage add all files changes",
        QuickLaunchAction::StageAll,
        true,
        None,
    );
    push(
        &mut commands,
        GEAR,
        "Unstage All",
        "unstage reset index clear staged",
        QuickLaunchAction::UnstageAll,
        true,
        None,
    );
    push(
        &mut commands,
        TRASH,
        "Discard All",
        "discard reset undo changes",
        QuickLaunchAction::DiscardAll,
        true,
        None,
    );
    push(
        &mut commands,
        GIT_FORK,
        "Create Branch",
        "branch new create fork",
        QuickLaunchAction::CreateBranch,
        true,
        Some(if cfg!(target_os = "macos") {
            "⇧⌘B"
        } else {
            "Ctrl+Shift+B"
        }),
    );
    push(
        &mut commands,
        TAG,
        "Create Tag",
        "tag label release version",
        QuickLaunchAction::CreateTag,
        true,
        Some(if cfg!(target_os = "macos") {
            "⇧⌘T"
        } else {
            "Ctrl+Shift+T"
        }),
    );
    push(
        &mut commands,
        GIT_PULL_REQUEST,
        "Save Stash",
        "stash save wip",
        QuickLaunchAction::SaveStash,
        true,
        Some(if cfg!(target_os = "macos") {
            "⇧⌘H"
        } else {
            "Ctrl+Shift+H"
        }),
    );
    push(
        &mut commands,
        ARROW_CLOCKWISE,
        "Refresh",
        "refresh reload update f5",
        QuickLaunchAction::Refresh,
        true,
        Some("F5"),
    );

    commands
}

fn branch_commands(branches: &[CachedBranch]) -> Vec<CommandEntry> {
    branches
        .iter()
        .filter(|branch| !branch.is_current)
        .flat_map(|branch| {
            let checkout = CommandEntry {
                icon: GIT_BRANCH,
                label: format!("Checkout Branch: {}", branch.name),
                keywords: format!("checkout switch branch {}", branch.name),
                action: QuickLaunchAction::CheckoutBranch(branch.name.clone()),
                requires_repo: true,
                shortcut: None,
            };
            let delete = CommandEntry {
                icon: TRASH,
                label: format!("Delete Branch: {}", branch.name),
                keywords: format!("delete remove branch {}", branch.name),
                action: QuickLaunchAction::DeleteBranch(branch.name.clone()),
                requires_repo: true,
                shortcut: None,
            };
            [checkout, delete]
        })
        .collect()
}

fn stash_commands(stashes: &[CachedStash]) -> Vec<CommandEntry> {
    stashes
        .iter()
        .enumerate()
        .flat_map(|(idx, stash)| {
            let label_suffix = if stash.message.is_empty() {
                format!("stash@{{{}}}", idx)
            } else {
                format!("stash@{{{}}}: {}", idx, stash.message)
            };

            let apply = CommandEntry {
                icon: ARROW_CLOCKWISE,
                label: format!("Apply Stash: {}", label_suffix),
                keywords: format!("stash apply {} {}", idx, stash.message),
                action: QuickLaunchAction::ApplyStash(idx),
                requires_repo: true,
                shortcut: None,
            };
            let pop = CommandEntry {
                icon: ARROW_COUNTER_CLOCKWISE,
                label: format!("Pop Stash: {}", label_suffix),
                keywords: format!("stash pop {} {}", idx, stash.message),
                action: QuickLaunchAction::PopStash(idx),
                requires_repo: true,
                shortcut: None,
            };
            let drop = CommandEntry {
                icon: TRASH,
                label: format!("Drop Stash: {}", label_suffix),
                keywords: format!("stash drop delete {} {}", idx, stash.message),
                action: QuickLaunchAction::DropStash(idx),
                requires_repo: true,
                shortcut: None,
            };
            [apply, pop, drop]
        })
        .collect()
}

fn all_commands(app_state: &AppState) -> Vec<CommandEntry> {
    let mut commands = fixed_commands();
    commands.extend(branch_commands(&app_state.cached_branches));
    commands.extend(stash_commands(&app_state.cached_stashes));
    commands
}

fn matches_query(query: &str, entry: &CommandEntry) -> bool {
    if query.is_empty() {
        return true;
    }

    let q = query.to_lowercase();
    entry.label.to_lowercase().contains(&q) || entry.keywords.to_lowercase().contains(&q)
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

pub fn show(
    ctx: &egui::Context,
    state: &mut State,
    app_state: &AppState,
    has_repo: bool,
    busy_action: Option<&QuickLaunchAction>,
) -> PaletteResult {
    let commands = all_commands(app_state);
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
            egui::pos2(search_rect.left() + 8.0, search_rect.top() + 4.0),
            egui::vec2(search_rect.width() - 50.0, search_rect.height() - 8.0),
        );
        let search_response =
            ui.scope_builder(egui::UiBuilder::new().max_rect(text_edit_rect), |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.add_space(4.0);
                    ui.label(
                        egui::RichText::new(MAGNIFYING_GLASS)
                            .size(13.0)
                            .color(egui::Color32::from_rgb(120, 120, 120)),
                    );
                    ui.add_space(6.0);

                    ui.vertical(|ui| {
                        ui.add_space(4.0);
                        let input_width = ui.available_width();
                        ui.add_sized(
                            [input_width, 20.0],
                            egui::TextEdit::singleline(&mut state.query)
                                .hint_text("Type a command...")
                                .font(egui::FontId::proportional(15.0))
                                .frame(egui::Frame::NONE)
                                .background_color(editor_fill),
                        )
                    })
                    .inner
                })
                .inner
            });
        ui.add_space(2.0);

        if search_response.inner.changed() {
            state.selected_index = 0;
        }

        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            return None;
        }

        let is_busy = busy_action.is_some();

        if !is_busy {
            if ctx.input(|i| i.key_pressed(Key::ArrowDown)) {
                state.selected_index =
                    (state.selected_index + 1).min(filtered.len().saturating_sub(1));
            }

            if ctx.input(|i| i.key_pressed(Key::ArrowUp)) {
                state.selected_index = state.selected_index.saturating_sub(1);
            }

            if ctx.input(|i| i.key_pressed(Key::Enter)) {
                if let Some(entry) = filtered.get(state.selected_index) {
                    return Some(entry.action.clone());
                }
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
                    let is_busy_row = busy_action.is_some_and(|busy| busy == &entry.action);
                    let row_response = ui.add(CommandRow {
                        icon: entry.icon,
                        label: &entry.label,
                        shortcut: entry.shortcut,
                        is_selected,
                        disabled: is_busy && !is_busy_row,
                        busy: is_busy_row,
                    });

                    if row_response.clicked() && !is_busy {
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
    disabled: bool,
    busy: bool,
}

impl egui::Widget for CommandRow<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let height = 32.0;
        let sense = if self.disabled {
            egui::Sense::hover()
        } else {
            egui::Sense::click()
        };
        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), height), sense);

        if self.is_selected {
            ui.painter()
                .rect_filled(rect, 4.0, egui::Color32::from_rgb(52, 52, 52));
        }

        let tint = if self.disabled {
            egui::Color32::from_rgb(135, 135, 135)
        } else {
            ui.visuals().text_color()
        };

        let icon_x = rect.left() + 12.0;
        let icon_y = rect.center().y;
        ui.painter().text(
            egui::pos2(icon_x, icon_y),
            egui::Align2::CENTER_CENTER,
            self.icon,
            egui::FontId::proportional(16.0),
            tint,
        );

        let label_x = icon_x + 28.0;
        ui.painter().text(
            egui::pos2(label_x, rect.center().y),
            egui::Align2::LEFT_CENTER,
            self.label,
            egui::FontId::proportional(13.0),
            tint,
        );

        if self.busy {
            let spinner_rect = egui::Rect::from_center_size(
                egui::pos2(rect.right() - 18.0, rect.center().y),
                egui::vec2(12.0, 12.0),
            );
            ui.put(spinner_rect, egui::Spinner::new().size(12.0));
        } else if let Some(shortcut) = self.shortcut {
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
