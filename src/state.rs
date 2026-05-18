use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use zed::{Store, create_reducer};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppState {
    pub current_repo: Option<String>,
    pub recent_repos: Vec<String>,
    pub show_window_buttons: bool,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_repo: None,
            recent_repos: Vec::new(),
            show_window_buttons: true,
        }
    }
}

impl AppState {
    pub fn repo_name(&self) -> Option<&str> {
        self.current_repo.as_deref().map(|p| {
            Path::new(p)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(p)
        })
    }

    fn push_recent(mut self, path: &str) -> Self {
        self.recent_repos.retain(|p| p != path);
        self.recent_repos.insert(0, path.to_string());
        if self.recent_repos.len() > 10 {
            self.recent_repos.truncate(10);
        }
        self
    }
}

impl AppState {
    fn with_current_repo(mut self, repo: Option<String>) -> Self {
        self.current_repo = repo;
        self
    }
}

#[derive(Clone, Debug)]
pub enum AppAction {
    OpenRepo(String),
    SelectRecent(usize),
    ToggleWindowButtons(bool),
}

fn reducer(state: &AppState, action: &AppAction) -> AppState {
    match action {
        AppAction::OpenRepo(path) => state
            .clone()
            .push_recent(path)
            .with_current_repo(Some(path.clone())),
        AppAction::SelectRecent(index) => {
            if let Some(path) = state.recent_repos.get(*index).cloned() {
                AppState {
                    current_repo: Some(path),
                    recent_repos: state.recent_repos.clone(),
                    show_window_buttons: state.show_window_buttons,
                }
            } else {
                state.clone()
            }
        }
        AppAction::ToggleWindowButtons(show) => AppState {
            current_repo: state.current_repo.clone(),
            recent_repos: state.recent_repos.clone(),
            show_window_buttons: *show,
        },
    }
}

pub type AppStore = Store<AppState, AppAction>;

pub fn create_store() -> Arc<AppStore> {
    let initial = AppState::default();
    Arc::new(Store::new(initial, Box::new(create_reducer(reducer))))
}
