use crate::git::live::RepoRemoteSnapshot;
use crate::git::models::{Branch, Commit, Remote, RepoStatus, Stash, Tag};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct BoundedLocalSnapshot {
    pub commits: Vec<Commit>,
    pub branches: Vec<Branch>,
    pub remotes: Vec<Remote>,
    pub tags: Vec<Tag>,
    pub stashes: Vec<Stash>,
    pub status: RepoStatus,
    pub repo_error: Option<String>,
    pub last_refresh: Option<u128>,
    pub ownership: Option<bool>,
}

impl BoundedLocalSnapshot {
    pub fn to_snapshot(&self) -> crate::git::live::RepoLocalSnapshot {
        crate::git::live::RepoLocalSnapshot {
            commits: self.commits.clone(),
            branches: self.branches.clone(),
            remotes: self.remotes.clone(),
            tags: self.tags.clone(),
            stashes: self.stashes.clone(),
            status: self.status.clone(),
            repo_error: self.repo_error.clone(),
            last_refresh: self.last_refresh,
            ownership: self.ownership,
        }
    }

    pub fn from_snapshot(s: &crate::git::live::RepoLocalSnapshot) -> Self {
        Self {
            commits: s.commits.clone(),
            branches: s.branches.clone(),
            remotes: s.remotes.clone(),
            tags: s.tags.clone(),
            stashes: s.stashes.clone(),
            status: s.status.clone(),
            repo_error: s.repo_error.clone(),
            last_refresh: s.last_refresh,
            ownership: s.ownership,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DiskCache {
    pub schema_version: u32,
    pub repo_path: String,
    pub repo_fingerprint: String,
    pub captured_at: u128,
    pub local_snapshot: BoundedLocalSnapshot,
    pub remote_snapshot: Option<RepoRemoteSnapshot>,

    // GitHub endpoint ETags
    pub prs_etag: Option<String>,
    pub actions_etag: Option<String>,
    pub releases_etag: Option<String>,
    pub packages_container_etag: Option<String>,
    pub packages_npm_etag: Option<String>,

    // Last fetched timestamps per endpoint
    pub prs_fetched_at: Option<u128>,
    pub actions_fetched_at: Option<u128>,
    pub releases_fetched_at: Option<u128>,
    pub packages_container_fetched_at: Option<u128>,
    pub packages_npm_fetched_at: Option<u128>,

    // Errors and retry metadata
    pub prs_error: Option<String>,
    pub actions_error: Option<String>,
    pub releases_error: Option<String>,
    pub packages_error: Option<String>,
}

#[cfg(not(test))]
pub fn cache_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("io", "parazeeknova", "Palimpsest")
        .map(|dirs| dirs.data_dir().join("cache"))
}

#[cfg(test)]
pub fn cache_dir() -> Option<PathBuf> {
    Some(std::env::temp_dir().join("palimpsest-cache-tests"))
}

fn cache_path_for_repo(repo_path: &str) -> Option<PathBuf> {
    let dir = cache_dir()?;
    let mut hasher = Sha256::new();
    hasher.update(repo_path.as_bytes());
    let filename = format!("{:x}.json", hasher.finalize());
    Some(dir.join(filename))
}

pub fn save_cache(cache: &DiskCache) -> Result<(), String> {
    let path = cache_path_for_repo(&cache.repo_path)
        .ok_or_else(|| "Could not resolve cache path".to_string())?;

    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            let err_msg = format!("Failed to create cache dir: {e}");
            tracing::error!(repo = %cache.repo_path, err = %err_msg);
            return Err(err_msg);
        }
    }

    let serialized = match serde_json::to_string_pretty(cache) {
        Ok(s) => s,
        Err(e) => {
            let err_msg = format!("Failed to serialize cache: {e}");
            tracing::error!(repo = %cache.repo_path, err = %err_msg);
            return Err(err_msg);
        }
    };

    if let Err(e) = std::fs::write(&path, serialized) {
        let err_msg = format!("Failed to write cache file: {e}");
        tracing::error!(repo = %cache.repo_path, err = %err_msg);
        return Err(err_msg);
    }

    tracing::debug!(repo = %cache.repo_path, "Successfully saved repository cache to disk");
    Ok(())
}

pub fn load_cache(repo_path: &str) -> Option<DiskCache> {
    let path = cache_path_for_repo(repo_path)?;
    if !path.exists() {
        tracing::debug!(repo = %repo_path, "No repository cache file found");
        return None;
    }

    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(repo = %repo_path, err = %e, "Failed to read cache file");
            return None;
        }
    };

    let cache: DiskCache = match serde_json::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(repo = %repo_path, err = %e, "Failed to deserialize repository cache");
            return None;
        }
    };

    if cache.schema_version == SCHEMA_VERSION {
        tracing::info!(repo = %repo_path, "Successfully hydrated repository cache from disk");
        Some(cache)
    } else {
        tracing::info!(
            repo = %repo_path,
            old = cache.schema_version,
            current = SCHEMA_VERSION,
            "Cache schema version mismatch. Deleting obsolete cache file"
        );
        let _ = std::fs::remove_file(&path);
        None
    }
}

fn get_file_fingerprint(path: &Path) -> Option<String> {
    let metadata = std::fs::metadata(path).ok()?;
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let size = metadata.len();
    let name = path.file_name()?.to_string_lossy();
    Some(format!("{}:{}:{}", name, mtime, size))
}

fn append_dir_fingerprint(dir: &Path, base_dir: &Path, out: &mut Vec<String>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                append_dir_fingerprint(&path, base_dir, out);
            } else if path.is_file() {
                if let Ok(meta) = std::fs::metadata(&path) {
                    let mtime = meta
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_millis())
                        .unwrap_or(0);
                    let size = meta.len();
                    if let Ok(rel) = path.strip_prefix(base_dir) {
                        out.push(format!("{}:{}:{}", rel.to_string_lossy(), mtime, size));
                    }
                }
            }
        }
    }
}

pub fn compute_repo_fingerprint(repo_path: &str) -> String {
    let git_dir = Path::new(repo_path).join(".git");
    if !git_dir.exists() {
        let git_dir_alt = Path::new(repo_path);
        if git_dir_alt.join("HEAD").exists() {
            return compute_fingerprint_for_git_dir(git_dir_alt);
        }
        return "invalid-repo".to_string();
    }
    compute_fingerprint_for_git_dir(&git_dir)
}

fn compute_fingerprint_for_git_dir(git_dir: &Path) -> String {
    let mut parts = Vec::new();

    for file in &["HEAD", "index", "packed-refs", "FETCH_HEAD", "config"] {
        let p = git_dir.join(file);
        if let Some(f_fp) = get_file_fingerprint(&p) {
            parts.push(f_fp);
        }
    }

    for ref_dir in &["refs/heads", "refs/remotes", "refs/tags"] {
        let d = git_dir.join(ref_dir);
        append_dir_fingerprint(&d, git_dir, &mut parts);
    }

    parts.sort();

    let mut hasher = Sha256::new();
    for p in &parts {
        hasher.update(p.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_cache_load_save_invalid() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_cache_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&temp_dir).unwrap();

        let path_str = temp_dir.to_str().unwrap().to_string();
        let local = BoundedLocalSnapshot {
            commits: Vec::new(),
            branches: Vec::new(),
            remotes: Vec::new(),
            tags: Vec::new(),
            stashes: Vec::new(),
            status: RepoStatus {
                branch: "main".to_string(),
                staged_count: 0,
                unstaged_count: 0,
                staged_files: Vec::new(),
                unstaged_files: Vec::new(),
                additions: 0,
                deletions: 0,
                files_changed: 0,
            },
            repo_error: None,
            last_refresh: None,
            ownership: None,
        };

        let cache = DiskCache {
            schema_version: SCHEMA_VERSION,
            repo_path: path_str.clone(),
            repo_fingerprint: "test-fingerprint".to_string(),
            captured_at: 12345678,
            local_snapshot: local,
            remote_snapshot: None,
            prs_etag: Some("etag-val".to_string()),
            actions_etag: None,
            releases_etag: None,
            packages_container_etag: None,
            packages_npm_etag: None,
            prs_fetched_at: None,
            actions_fetched_at: None,
            releases_fetched_at: None,
            packages_container_fetched_at: None,
            packages_npm_fetched_at: None,
            prs_error: None,
            actions_error: None,
            releases_error: None,
            packages_error: None,
        };

        save_cache(&cache).unwrap();

        let loaded = load_cache(&path_str).unwrap();
        assert_eq!(loaded.repo_fingerprint, "test-fingerprint");
        assert_eq!(loaded.prs_etag.as_deref(), Some("etag-val"));

        // Test invalid schema version removes cache file and returns None
        let mut loaded_corrupt = loaded;
        loaded_corrupt.schema_version = SCHEMA_VERSION + 1;
        save_cache(&loaded_corrupt).unwrap();
        assert!(load_cache(&path_str).is_none());

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_fingerprint_changes() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_fp_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp_dir.join(".git")).unwrap();

        let path_str = temp_dir.to_str().unwrap();
        let fp1 = compute_repo_fingerprint(path_str);

        // Write HEAD file
        fs::write(temp_dir.join(".git/HEAD"), "ref: refs/heads/main").unwrap();
        let fp2 = compute_repo_fingerprint(path_str);
        assert_ne!(fp1, fp2);

        // Modify HEAD file
        fs::write(temp_dir.join(".git/HEAD"), "ref: refs/heads/feature").unwrap();
        let fp3 = compute_repo_fingerprint(path_str);
        assert_ne!(fp2, fp3);

        fs::remove_dir_all(&temp_dir).unwrap();
    }
}
