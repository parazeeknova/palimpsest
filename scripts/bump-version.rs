#!/usr/bin/env cargo
//! ```cargo
//! [dependencies]
//! ```
use std::fs;
use std::path::PathBuf;

fn root() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."))
}

fn next_version(version: &str) -> String {
    let parts: Vec<i32> = version
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let (major, minor, patch) = (parts[0], parts[1], parts[2]);
    if patch < 99 {
        format!("{}.{}.{}", major, minor, patch + 1)
    } else {
        format!("{}.{}.0", major, minor + 1)
    }
}

fn read_package_version(cargo_toml: &PathBuf) -> String {
    let cargo = fs::read_to_string(cargo_toml).unwrap_or_else(|e| {
        eprintln!("Failed to read Cargo.toml: {}", e);
        std::process::exit(1);
    });
    for line in cargo.lines() {
        let line = line.trim();
        if line.starts_with("version") && line.contains('=') {
            if let Some(start) = line.find('"') {
                if let Some(end) = line[start + 1..].find('"') {
                    return line[start + 1..start + 1 + end].to_string();
                }
            }
        }
    }
    panic!("Could not find package version in Cargo.toml");
}

fn replace_package_version(path: &PathBuf, old: &str, new: &str) {
    let text = fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("Failed to read {}: {}", path.display(), e);
        std::process::exit(1);
    });
    let text = text.replacen(
        &format!("version = \"{}\"", old),
        &format!("version = \"{}\"", new),
        1,
    );
    fs::write(path, text).unwrap_or_else(|e| {
        eprintln!("Failed to write {}: {}", path.display(), e);
        std::process::exit(1);
    });
}

fn replace_lock_version(cargo_lock: &PathBuf, old: &str, new: &str) {
    if !cargo_lock.exists() {
        return;
    }
    let text = fs::read_to_string(cargo_lock).unwrap_or_else(|e| {
        eprintln!("Failed to read Cargo.lock: {}", e);
        std::process::exit(1);
    });
    let search = format!("[[package]]\nname = \"palimpsest\"\nversion = \"{}\"", old);
    let replace = format!("[[package]]\nname = \"palimpsest\"\nversion = \"{}\"", new);
    if let Some(pos) = text.find(&search) {
        let new_text = format!(
            "{}{}{}",
            &text[..pos],
            replace,
            &text[pos + search.len()..]
        );
        fs::write(cargo_lock, new_text).unwrap_or_else(|e| {
            eprintln!("Failed to write Cargo.lock: {}", e);
            std::process::exit(1);
        });
    }
}

fn replace_packager_version(packager_toml: &PathBuf, old: &str, new: &str) {
    if packager_toml.exists() {
        replace_package_version(packager_toml, old, new);
    }
}

fn main() {
    let root = root();
    let cargo_toml = root.join("Cargo.toml");
    let cargo_lock = root.join("Cargo.lock");
    let packager_toml = root.join("Packager.toml");

    let old = read_package_version(&cargo_toml);
    let new = next_version(&old);
    replace_package_version(&cargo_toml, &old, &new);
    replace_lock_version(&cargo_lock, &old, &new);
    replace_packager_version(&packager_toml, &old, &new);
    println!("palimpsest version bumped: {} -> {}", old, new);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_temp_file(content: &str) -> (PathBuf, std::fs::File) {
        let dir = std::env::temp_dir();
        let path = dir.join(format!("bump_test_{}.toml", std::process::id()));
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        (path, file)
    }

    #[test]
    fn test_next_version_increment_patch() {
        assert_eq!(next_version("1.2.3"), "1.2.4");
        assert_eq!(next_version("0.0.0"), "0.0.1");
        assert_eq!(next_version("10.20.30"), "10.20.31");
    }

    #[test]
    fn test_next_version_roll_over_patch_to_minor() {
        assert_eq!(next_version("1.2.99"), "1.3.0");
        assert_eq!(next_version("0.0.99"), "0.1.0");
        assert_eq!(next_version("5.99.99"), "5.100.0");
    }

    #[test]
    fn test_read_package_version_success() {
        let (path, _file) = create_temp_file(
            r#"[package]
name = "palimpsest"
version = "1.2.3"
edition = "2021"
"#,
        );
        assert_eq!(read_package_version(&path), "1.2.3");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_read_package_version_with_spaces() {
        let (path, _file) = create_temp_file(
            r#"[package]
name = "palimpsest"
version   =   "2.0.1"
edition = "2021"
"#,
        );
        assert_eq!(read_package_version(&path), "2.0.1");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    #[should_panic(expected = "Could not find package version in Cargo.toml")]
    fn test_read_package_version_missing() {
        let (path, _file) = create_temp_file(
            r#"[package]
name = "palimpsest"
edition = "2021"
"#,
        );
        read_package_version(&path);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_replace_package_version() {
        let (path, _file) = create_temp_file(
            r#"[package]
name = "palimpsest"
version = "1.0.0"
edition = "2021"
"#,
        );
        replace_package_version(&path, "1.0.0", "1.0.1");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("version = \"1.0.1\""));
        assert!(!content.contains("version = \"1.0.0\""));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_replace_package_version_only_first_occurrence() {
        let (path, _file) = create_temp_file(
            r#"[package]
name = "palimpsest"
version = "1.0.0"

[dependencies]
serde = { version = "1.0.0" }
"#,
        );
        replace_package_version(&path, "1.0.0", "1.0.1");
        let content = fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        assert_eq!(lines[2], "version = \"1.0.1\"");
        assert!(content.contains("serde = { version = \"1.0.0\" }"));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_replace_lock_version_success() {
        let (path, _file) = create_temp_file(
            r#"[[package]]
name = "palimpsest"
version = "1.2.3"
dependencies = []

[[package]]
name = "other"
version = "0.1.0"
"#,
        );
        replace_lock_version(&path, "1.2.3", "1.2.4");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("version = \"1.2.4\""));
        assert!(content.contains("name = \"palimpsest\""));
        assert!(content.contains("version = \"0.1.0\""));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_replace_lock_version_no_change_if_not_found() {
        let original = r#"[[package]]
name = "other"
version = "0.1.0"
"#;
        let (path, _file) = create_temp_file(original);
        replace_lock_version(&path, "1.2.3", "1.2.4");
        let content = fs::read_to_string(&path).unwrap();
        assert_eq!(content, original);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_replace_lock_version_missing_file() {
        let non_existent = PathBuf::from("/non/existent/Cargo.lock");
        replace_lock_version(&non_existent, "1.0.0", "1.0.1");
    }

    #[test]
    fn test_replace_packager_version_success() {
        let (path, _file) = create_temp_file(
            r#"[package]
name = "palimpsest-packager"
version = "3.2.1"
"#,
        );
        replace_packager_version(&path, "3.2.1", "3.2.2");
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("version = \"3.2.2\""));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_replace_packager_version_missing_file() {
        let non_existent = PathBuf::from("/non/existent/Packager.toml");
        replace_packager_version(&non_existent, "1.0.0", "1.0.1");
    }
}
