use crate::state::CachedBranch;
use eframe::egui;
use std::hash::{Hash, Hasher};

pub fn get_branch_color(name: &str, branches: &[CachedBranch]) -> egui::Color32 {
    let target_base = name.split('/').next_back().unwrap_or(name);

    let mut unique_names: Vec<String> = branches
        .iter()
        .map(|b| b.name.split('/').next_back().unwrap_or(&b.name).to_string())
        .collect();
    unique_names.sort();
    unique_names.dedup();

    let index = unique_names
        .iter()
        .position(|n| n == target_base)
        .unwrap_or(0);
    let count = unique_names.len().max(1);

    let hue = index as f32 / count as f32;
    egui::Color32::from(egui::epaint::Hsva::new(hue, 0.75, 0.85, 1.0))
}

pub fn get_tag_color(name: &str) -> egui::Color32 {
    let base_name = name.split('/').next_back().unwrap_or(name);
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    base_name.hash(&mut hasher);
    let hash = hasher.finish();
    let hue = (hash as f32) / (u64::MAX as f32);
    egui::Color32::from(egui::epaint::Hsva::new(hue, 0.65, 0.85, 1.0))
}
