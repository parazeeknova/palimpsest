use eframe::egui;
use std::collections::BTreeMap;

use crate::git::models::FileChangeKind;

pub const TREE_ROW_HEIGHT: f32 = 20.0;
pub const TREE_SLOT_WIDTH: f32 = 22.0;
pub const TREE_LEFT_PADDING: f32 = 6.0;
pub const TREE_CARET_SLOT: f32 = 6.0;
pub const TREE_ICON_GAP: f32 = 24.0;

// Include the compile-time generated file icon SVGs map
include!(concat!(env!("OUT_DIR"), "/file_icons_map.rs"));

const FILE_STEMS_BY_ICON_KEY: &[(&str, &[&str])] = &[
    ("docker", &["Containerfile", "Dockerfile"]),
    ("ruby", &["Podfile"]),
    ("heroku", &["Procfile"]),
];

const FILE_SUFFIXES_BY_ICON_KEY: &[(&str, &[&str])] = &[
    ("astro", &["astro"]),
    (
        "audio",
        &[
            "aac", "flac", "m4a", "mka", "mp3", "ogg", "opus", "wav", "wma", "wv",
        ],
    ),
    ("backup", &["bak"]),
    ("ballerina", &["bal"]),
    ("bicep", &["bicep"]),
    ("bun", &["lockb"]),
    ("c", &["c", "h"]),
    ("cairo", &["cairo"]),
    ("code", &["handlebars", "metadata", "rkt", "scm"]),
    ("coffeescript", &["coffee"]),
    (
        "cpp",
        &[
            "c++", "h++", "cc", "cpp", "cppm", "cxx", "hh", "hpp", "hxx", "inl", "ixx",
        ],
    ),
    ("crystal", &["cr", "ecr"]),
    ("csharp", &["cs"]),
    ("csproj", &["csproj"]),
    ("css", &["css", "pcss", "postcss"]),
    ("cue", &["cue"]),
    ("dart", &["dart"]),
    ("diff", &["diff"]),
    (
        "docker",
        &[
            "docker-compose.yml",
            "docker-compose.yaml",
            "compose.yml",
            "compose.yaml",
        ],
    ),
    (
        "document",
        &[
            "doc", "docx", "mdx", "odp", "ods", "odt", "pdf", "ppt", "pptx", "rtf", "txt", "xls",
            "xlsx",
        ],
    ),
    ("editorconfig", &["editorconfig"]),
    ("elixir", &["eex", "ex", "exs", "heex", "leex", "neex"]),
    ("elm", &["elm"]),
    (
        "erlang",
        &[
            "Emakefile",
            "app.src",
            "erl",
            "escript",
            "hrl",
            "rebar.config",
            "xrl",
            "yrl",
        ],
    ),
    (
        "eslint",
        &[
            "eslint.config.cjs",
            "eslint.config.cts",
            "eslint.config.js",
            "eslint.config.mjs",
            "eslint.config.mts",
            "eslint.config.ts",
            "eslintrc",
            "eslintrc.js",
            "eslintrc.json",
        ],
    ),
    ("font", &["otf", "ttf", "woff", "woff2"]),
    ("fsharp", &["fs"]),
    ("fsproj", &["fsproj"]),
    ("gitlab", &["gitlab-ci.yml", "gitlab-ci.yaml"]),
    ("gleam", &["gleam"]),
    ("go", &["go", "mod", "work"]),
    ("graphql", &["gql", "graphql", "graphqls"]),
    ("haskell", &["hs"]),
    ("hcl", &["hcl"]),
    (
        "helm",
        &[
            "helmfile.yaml",
            "helmfile.yml",
            "Chart.yaml",
            "Chart.yml",
            "Chart.lock",
            "values.yaml",
            "values.yml",
            "requirements.yaml",
            "requirements.yml",
            "tpl",
        ],
    ),
    ("html", &["htm", "html"]),
    (
        "image",
        &[
            "avif", "bmp", "gif", "heic", "heif", "ico", "j2k", "jfif", "jp2", "jpeg", "jpg",
            "jxl", "png", "psd", "qoi", "svg", "tiff", "webp",
        ],
    ),
    ("ipynb", &["ipynb"]),
    ("java", &["java"]),
    ("javascript", &["cjs", "js", "mjs"]),
    ("json", &["json", "jsonc"]),
    ("julia", &["jl"]),
    ("kdl", &["kdl"]),
    ("kotlin", &["kt"]),
    ("lock", &["lock"]),
    ("log", &["log"]),
    ("lua", &["lua"]),
    ("luau", &["luau"]),
    ("markdown", &["markdown", "md"]),
    ("metal", &["metal"]),
    ("nim", &["nim", "nims", "nimble"]),
    ("nix", &["nix"]),
    ("ocaml", &["ml", "mli", "mlx"]),
    ("odin", &["odin"]),
    ("php", &["php"]),
    (
        "prettier",
        &[
            "prettier.config.cjs",
            "prettier.config.js",
            "prettier.config.mjs",
            "prettierignore",
            "prettierrc",
            "prettierrc.cjs",
            "prettierrc.js",
            "prettierrc.json",
            "prettierrc.json5",
            "prettierrc.mjs",
            "prettierrc.toml",
            "prettierrc.yaml",
            "prettierrc.yml",
        ],
    ),
    ("prisma", &["prisma"]),
    ("puppet", &["pp"]),
    ("python", &["py"]),
    ("r", &["r", "R"]),
    ("react", &["cjsx", "ctsx", "jsx", "mjsx", "mtsx", "tsx"]),
    ("roc", &["roc"]),
    ("ruby", &["rb"]),
    ("rust", &["rs"]),
    ("sass", &["sass", "scss"]),
    ("scala", &["scala", "sc"]),
    ("settings", &["conf", "ini"]),
    ("solidity", &["sol"]),
    (
        "storage",
        &[
            "accdb", "csv", "dat", "db", "dbf", "dll", "fmp", "fp7", "frm", "gdb", "ib", "ldf",
            "mdb", "mdf", "myd", "myi", "pdb", "RData", "rdata", "sav", "sdf", "sql", "sqlite",
            "tsv",
        ],
    ),
    (
        "stylelint",
        &[
            "stylelint.config.cjs",
            "stylelint.config.js",
            "stylelint.config.mjs",
            "stylelintignore",
            "stylelintrc",
            "stylelintrc.cjs",
            "stylelintrc.js",
            "stylelintrc.json",
            "stylelintrc.mjs",
            "stylelintrc.yaml",
            "stylelintrc.yml",
        ],
    ),
    ("surrealql", &["surql"]),
    ("svelte", &["svelte"]),
    ("swift", &["swift"]),
    ("tcl", &["tcl"]),
    ("template", &["hbs", "plist", "xml"]),
    (
        "terminal",
        &[
            "bash",
            "bash_aliases",
            "bash_login",
            "bash_logout",
            "bash_profile",
            "bashrc",
            "fish",
            "nu",
            "profile",
            "ps1",
            "sh",
            "zlogin",
            "zlogout",
            "zprofile",
            "zsh",
            "zsh_aliases",
            "zsh_histfile",
            "zsh_history",
            "zshenv",
            "zshrc",
        ],
    ),
    ("terraform", &["tf", "tfvars"]),
    ("toml", &["toml"]),
    ("typescript", &["cts", "mts", "ts"]),
    ("v", &["v", "vsh", "vv"]),
    (
        "vcs",
        &[
            "COMMIT_EDITMSG",
            "EDIT_DESCRIPTION",
            "MERGE_MSG",
            "NOTES_EDITMSG",
            "TAG_EDITMSG",
            "gitattributes",
            "gitignore",
            "gitkeep",
            "gitmodules",
        ],
    ),
];

#[derive(Clone, Debug)]
pub struct FileTreeItem {
    pub path: String,
    pub change_kind: Option<FileChangeKind>,
}

#[derive(Clone, Debug)]
pub enum TreeEntryKind {
    File,
    Directory,
}

#[derive(Clone, Debug)]
pub struct TreeEntry {
    pub path: String,
    pub label: String,
    pub kind: TreeEntryKind,
    pub file_kind: Option<FileChangeKind>,
    pub file_index: Option<usize>,
    pub expanded: bool,
    pub has_children: bool,
    pub children: Vec<TreeEntry>,
    child_map: BTreeMap<String, TreeEntry>,
}

#[derive(Clone, Debug, Default)]
pub struct TreeState {
    pub rows: Vec<TreeEntry>,
    pub rebuild_key: Option<String>,
}

pub fn paint_tree_tab(
    ui: &mut egui::Ui,
    tree_state: &mut TreeState,
    files: &[FileTreeItem],
    populated: bool,
    muted: egui::Color32,
    rebuild_key: &str,
    id_salt: &str,
) {
    if !populated {
        ui.label(
            egui::RichText::new("Loading files...")
                .size(10.0)
                .color(muted),
        );
        return;
    }
    if files.is_empty() {
        ui.label(egui::RichText::new("No files").size(10.0).color(muted));
        return;
    }

    rebuild_tree_if_needed(tree_state, files, rebuild_key);
    paint_tree_header(ui, tree_state, muted);
    ui.add_space(6.0);

    egui::ScrollArea::vertical()
        .id_salt(id_salt)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            let len = tree_state.rows.len();
            let mut ancestors_last = Vec::new();
            for (index, row) in tree_state.rows.iter_mut().enumerate() {
                paint_tree_entry(ui, row, 0, &mut ancestors_last, index + 1 == len, muted);
            }
        });
}

fn paint_tree_header(ui: &mut egui::Ui, tree_state: &mut TreeState, muted: egui::Color32) {
    ui.horizontal(|ui| {
        if ui
            .button(egui::RichText::new("Expand All").size(9.0).color(muted))
            .clicked()
        {
            set_all_directories_expanded(tree_state, true);
        }
        if ui
            .button(egui::RichText::new("Collapse All").size(9.0).color(muted))
            .clicked()
        {
            set_all_directories_expanded(tree_state, false);
        }
    });
}

pub fn rebuild_tree_if_needed(
    tree_state: &mut TreeState,
    files: &[FileTreeItem],
    rebuild_key: &str,
) {
    if tree_state.rebuild_key.as_deref() == Some(rebuild_key) {
        return;
    }

    tree_state.rows = build_tree_entries(files);
    tree_state.rebuild_key = Some(rebuild_key.to_string());
}

fn build_tree_entries(files: &[FileTreeItem]) -> Vec<TreeEntry> {
    let mut root_map: BTreeMap<String, TreeEntry> = BTreeMap::new();

    for (file_index, file) in files.iter().enumerate() {
        let segments: Vec<&str> = file
            .path
            .split('/')
            .filter(|segment| !segment.is_empty())
            .collect();
        if segments.is_empty() {
            continue;
        }

        insert_tree_entry(
            &mut root_map,
            &segments,
            0,
            file_index,
            file.change_kind.as_ref(),
            String::new(),
        );
    }

    let mut root_nodes: Vec<TreeEntry> = root_map.into_values().collect();
    finalize_tree_entries(&mut root_nodes);
    sort_tree_entries(&mut root_nodes);
    root_nodes
}

fn insert_tree_entry(
    nodes: &mut BTreeMap<String, TreeEntry>,
    segments: &[&str],
    _depth: usize,
    file_index: usize,
    file_kind: Option<&FileChangeKind>,
    mut path_prefix: String,
) {
    if !path_prefix.is_empty() {
        path_prefix.push('/');
    }
    path_prefix.push_str(segments[0]);

    let is_file = segments.len() == 1;
    let entry = nodes
        .entry(segments[0].to_string())
        .or_insert_with(|| TreeEntry {
            path: path_prefix.clone(),
            label: segments[0].to_string(),
            kind: if is_file {
                TreeEntryKind::File
            } else {
                TreeEntryKind::Directory
            },
            file_kind: if is_file { file_kind.cloned() } else { None },
            file_index: if is_file { Some(file_index) } else { None },
            expanded: true,
            has_children: !is_file,
            children: Vec::new(),
            child_map: BTreeMap::new(),
        });

    if is_file {
        if matches!(entry.kind, TreeEntryKind::Directory) {
            entry.child_map.insert(
                format!("{}__file", entry.path),
                TreeEntry {
                    path: entry.path.clone(),
                    label: entry.label.clone(),
                    kind: TreeEntryKind::File,
                    file_kind: file_kind.cloned(),
                    file_index: Some(file_index),
                    expanded: true,
                    has_children: false,
                    children: Vec::new(),
                    child_map: BTreeMap::new(),
                },
            );
            entry.has_children = true;
        } else {
            entry.kind = TreeEntryKind::File;
            entry.file_kind = file_kind.cloned();
            entry.file_index = Some(file_index);
        }
        return;
    }

    if matches!(entry.kind, TreeEntryKind::File) {
        let original_file = TreeEntry {
            path: entry.path.clone(),
            label: entry.label.clone(),
            kind: TreeEntryKind::File,
            file_kind: entry.file_kind.take(),
            file_index: entry.file_index.take(),
            expanded: true,
            has_children: false,
            children: Vec::new(),
            child_map: BTreeMap::new(),
        };
        entry.kind = TreeEntryKind::Directory;
        entry.has_children = true;
        entry
            .child_map
            .insert(format!("{}__file", entry.path), original_file);
    }

    insert_tree_entry(
        &mut entry.child_map,
        &segments[1..],
        _depth + 1,
        file_index,
        file_kind,
        path_prefix,
    );
    entry.has_children = true;
}

fn finalize_tree_entries(entries: &mut [TreeEntry]) {
    for entry in entries.iter_mut() {
        finalize_tree_entry(entry);
    }
}

fn finalize_tree_entry(entry: &mut TreeEntry) {
    for child in entry.child_map.values_mut() {
        finalize_tree_entry(child);
    }

    if !entry.child_map.is_empty() {
        let child_map = std::mem::take(&mut entry.child_map);
        entry.children = child_map.into_values().collect();
        entry.has_children = true;
    }
    sort_tree_entries(&mut entry.children);
}

fn sort_tree_entries(entries: &mut [TreeEntry]) {
    entries.sort_by(|a, b| {
        let a_is_dir = matches!(a.kind, TreeEntryKind::Directory);
        let b_is_dir = matches!(b.kind, TreeEntryKind::Directory);

        if a_is_dir && !b_is_dir {
            return std::cmp::Ordering::Less;
        }
        if !a_is_dir && b_is_dir {
            return std::cmp::Ordering::Greater;
        }

        let a_is_dot = a.label.starts_with('.');
        let b_is_dot = b.label.starts_with('.');

        if a_is_dot && !b_is_dot {
            return std::cmp::Ordering::Less;
        }
        if !a_is_dot && b_is_dot {
            return std::cmp::Ordering::Greater;
        }

        a.label.to_lowercase().cmp(&b.label.to_lowercase())
    });

    for entry in entries.iter_mut() {
        if !entry.children.is_empty() {
            sort_tree_entries(&mut entry.children);
        }
    }
}

fn get_icon_key(path: &str) -> &'static str {
    let filename = path.rsplit('/').next().unwrap_or(path);

    // 1. Exact match on stems
    for &(key, stems) in FILE_STEMS_BY_ICON_KEY {
        if stems.contains(&filename) {
            return key;
        }
    }

    // 2. Exact match on suffixes (like full config filenames)
    for &(key, suffixes) in FILE_SUFFIXES_BY_ICON_KEY {
        if suffixes.contains(&filename) {
            return key;
        }
    }

    // 3. Match suffix extensions
    let parts: Vec<&str> = filename.split('.').collect();
    if parts.len() > 1 {
        // Try double extensions (like stories.tsx, gitlab-ci.yml)
        if parts.len() > 2 {
            let double_ext = format!("{}.{}", parts[parts.len() - 2], parts[parts.len() - 1]);
            for &(key, suffixes) in FILE_SUFFIXES_BY_ICON_KEY {
                if suffixes.contains(&double_ext.as_str()) {
                    return key;
                }
            }
        }

        // Try single extension (after last dot)
        let last_ext = parts[parts.len() - 1];
        for &(key, suffixes) in FILE_SUFFIXES_BY_ICON_KEY {
            if suffixes.contains(&last_ext) {
                return key;
            }
        }
    }

    "file"
}

fn get_icon_bytes(key: &str) -> &'static [u8] {
    for &(k, bytes) in FILE_ICONS {
        if k == key {
            return bytes;
        }
    }
    // Fall back to "file"
    for &(k, bytes) in FILE_ICONS {
        if k == "file" {
            return bytes;
        }
    }
    &[]
}

fn paint_tree_entry(
    ui: &mut egui::Ui,
    entry: &mut TreeEntry,
    depth: usize,
    ancestors_last: &mut Vec<bool>,
    is_last: bool,
    muted: egui::Color32,
) -> f32 {
    let row_height = TREE_ROW_HEIGHT;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), row_height),
        egui::Sense::click(),
    );

    if response.hovered() {
        ui.painter()
            .rect_filled(rect, 3.0, egui::Color32::from_white_alpha(12));
    }

    let row_left = rect.left() + TREE_LEFT_PADDING;
    let slot_left = row_left + TREE_SLOT_WIDTH * depth as f32;
    let center_y = rect.center().y;

    paint_tree_guides(ui, rect, ancestors_last, muted);

    if matches!(entry.kind, TreeEntryKind::Directory) {
        let chevron_rect = egui::Rect::from_center_size(
            egui::pos2(slot_left + TREE_CARET_SLOT, center_y),
            egui::vec2(8.0, 8.0),
        );
        let chevron_key = if entry.expanded {
            "chevron_down"
        } else {
            "chevron_right"
        };
        let svg_bytes = get_icon_bytes(chevron_key);
        let image_source = egui::ImageSource::Bytes {
            uri: std::borrow::Cow::Owned(format!("bytes://{}.svg", chevron_key)),
            bytes: egui::load::Bytes::Static(svg_bytes),
        };
        let image = egui::Image::new(image_source).tint(muted);
        ui.put(chevron_rect, image);

        if response.clicked() {
            entry.expanded = !entry.expanded;
        }
    }

    let (icon_key, icon_color) = match entry.kind {
        TreeEntryKind::Directory => {
            let key = if entry.expanded {
                "folder_open"
            } else {
                "folder"
            };
            (key, muted)
        }
        TreeEntryKind::File => (
            get_icon_key(&entry.path),
            file_icon_color(entry.file_kind.as_ref()),
        ),
    };

    let icon_x = if matches!(entry.kind, TreeEntryKind::Directory) {
        slot_left + TREE_ICON_GAP
    } else {
        slot_left + 2.0
    };

    let icon_rect =
        egui::Rect::from_center_size(egui::pos2(icon_x, center_y), egui::vec2(13.0, 13.0));
    let svg_bytes = get_icon_bytes(icon_key);
    let image_source = egui::ImageSource::Bytes {
        uri: std::borrow::Cow::Owned(format!("bytes://{}.svg", icon_key)),
        bytes: egui::load::Bytes::Static(svg_bytes),
    };
    let image = egui::Image::new(image_source).tint(icon_color);
    ui.put(icon_rect, image);

    ui.painter().text(
        egui::pos2(icon_x + 10.0, center_y),
        egui::Align2::LEFT_CENTER,
        &entry.label,
        egui::FontId::proportional(10.0),
        ui.visuals().text_color(),
    );

    if let Some(kind) = entry.file_kind.as_ref() {
        let (status_label, status_color) = file_status_label(kind.clone());
        ui.painter().text(
            egui::pos2(rect.right() - 12.0, center_y),
            egui::Align2::RIGHT_CENTER,
            status_label,
            egui::FontId::proportional(9.0),
            status_color,
        );
    }

    let mut subtree_height = row_height;

    if matches!(entry.kind, TreeEntryKind::Directory)
        && entry.expanded
        && !entry.children.is_empty()
    {
        let guide_x = slot_left + TREE_CARET_SLOT;
        let mut child_bottom = rect.bottom();

        ancestors_last.push(is_last);
        let child_len = entry.children.len();
        for (index, child) in entry.children.iter_mut().enumerate() {
            let child_height = paint_tree_entry(
                ui,
                child,
                depth + 1,
                ancestors_last,
                index + 1 == child_len,
                muted,
            );
            child_bottom += child_height;
            subtree_height += child_height;
        }
        ancestors_last.pop();

        ui.painter().line_segment(
            [
                egui::pos2(guide_x, rect.bottom() - 2.0),
                egui::pos2(guide_x, child_bottom - 1.0),
            ],
            egui::Stroke::new(1.0_f32, muted.linear_multiply(0.35)),
        );
    }

    subtree_height
}

fn paint_tree_guides(
    ui: &egui::Ui,
    rect: egui::Rect,
    ancestors_last: &[bool],
    muted: egui::Color32,
) {
    let row_left = rect.left() + TREE_LEFT_PADDING;

    for (depth, is_last) in ancestors_last.iter().enumerate() {
        if *is_last {
            continue;
        }

        let guide_x = row_left + TREE_SLOT_WIDTH * depth as f32 + TREE_CARET_SLOT;
        ui.painter().line_segment(
            [
                egui::pos2(guide_x, rect.top()),
                egui::pos2(guide_x, rect.bottom()),
            ],
            egui::Stroke::new(1.0_f32, muted.linear_multiply(0.28)),
        );
    }
}

fn set_all_directories_expanded(tree_state: &mut TreeState, expanded: bool) {
    for entry in &mut tree_state.rows {
        set_entry_expanded(entry, expanded);
    }
}

fn set_entry_expanded(entry: &mut TreeEntry, expanded: bool) {
    if matches!(entry.kind, TreeEntryKind::Directory) {
        entry.expanded = expanded;
        for child in &mut entry.children {
            set_entry_expanded(child, expanded);
        }
    }
}

fn file_icon_color(file_kind: Option<&FileChangeKind>) -> egui::Color32 {
    match file_kind {
        Some(FileChangeKind::Added) => egui::Color32::from_rgb(78, 190, 116),
        Some(FileChangeKind::Deleted) => egui::Color32::from_rgb(228, 86, 86),
        Some(FileChangeKind::Renamed) => egui::Color32::from_rgb(172, 172, 172),
        Some(FileChangeKind::TypeChanged) => egui::Color32::from_rgb(172, 172, 172),
        Some(FileChangeKind::Modified) => egui::Color32::from_rgb(252, 197, 34),
        None => egui::Color32::from_rgb(140, 140, 140),
    }
}

fn file_status_label(kind: FileChangeKind) -> (&'static str, egui::Color32) {
    match kind {
        FileChangeKind::Added => ("A", egui::Color32::from_rgb(78, 190, 116)),
        FileChangeKind::Modified => ("M", egui::Color32::from_rgb(252, 197, 34)),
        FileChangeKind::Deleted => ("D", egui::Color32::from_rgb(228, 86, 86)),
        FileChangeKind::Renamed => ("R", egui::Color32::from_rgb(172, 172, 172)),
        FileChangeKind::TypeChanged => ("T", egui::Color32::from_rgb(172, 172, 172)),
    }
}

pub fn paint_file_icon_rect(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    path: &str,
    icon_color: egui::Color32,
) {
    let icon_key = get_icon_key(path);
    let svg_bytes = get_icon_bytes(icon_key);
    let image_source = egui::ImageSource::Bytes {
        uri: std::borrow::Cow::Owned(format!("bytes://{}.svg", icon_key)),
        bytes: egui::load::Bytes::Static(svg_bytes),
    };
    let image = egui::Image::new(image_source).tint(icon_color);
    ui.put(rect, image);
}
