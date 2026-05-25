use crate::git::live::{
    RepoLocalSnapshot, RepoOwnership, RepoRemoteSnapshot, classify_repo_ownership,
};
use crate::git::models::{
    Branch, Commit, FileChangeKind, FileStatus, Remote, RepoStatus, Stash, Tag,
};
use crate::state::{GitHubActionRun, GitHubPackage, GitHubPullRequest, GitHubRelease};
use rusqlite::{OptionalExtension, Transaction};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub const SCHEMA_VERSION: u32 = 2;

#[derive(Clone, Debug)]
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
    pub fn to_snapshot(&self) -> RepoLocalSnapshot {
        RepoLocalSnapshot {
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

    pub fn from_snapshot(s: &RepoLocalSnapshot) -> Self {
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

#[derive(Clone, Debug)]
pub struct DiskCache {
    pub schema_version: u32,
    pub repo_path: String,
    pub repo_fingerprint: String,
    pub captured_at: u128,
    pub local_snapshot: BoundedLocalSnapshot,
    pub remote_snapshot: Option<RepoRemoteSnapshot>,

    pub prs_etag: Option<String>,
    pub actions_etag: Option<String>,
    pub releases_etag: Option<String>,
    pub packages_container_etag: Option<String>,
    pub packages_npm_etag: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RepoFingerprints {
    pub head: String,
    pub index: String,
    pub refs_heads: String,
    pub refs_remotes: String,
    pub refs_tags: String,
    pub packed_refs: String,
    pub config: String,
}

use std::cell::RefCell;

thread_local! {
    static TEST_DB_PATH: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub fn cache_dir() -> Option<PathBuf> {
    directories::ProjectDirs::from("io", "parazeeknova", "Palimpsest")
        .map(|dirs| dirs.data_dir().join("cache"))
}

pub fn open_conn() -> Result<rusqlite::Connection, String> {
    let db_path = TEST_DB_PATH.with(|p| p.borrow().clone());
    let db_path = if let Some(path) = db_path {
        path
    } else {
        let dir = cache_dir().ok_or_else(|| "Could not resolve cache path".to_string())?;
        if let Err(e) = std::fs::create_dir_all(&dir) {
            return Err(format!("Failed to create cache dir: {e}"));
        }
        dir.join("palimpsest.db")
    };
    let conn = rusqlite::Connection::open(&db_path).map_err(|e| e.to_string())?;

    let _ = conn.execute("PRAGMA journal_mode = WAL", []);
    let _ = conn.execute("PRAGMA foreign_keys = ON", []);

    migrate(&conn).map_err(|e| e.to_string())?;
    Ok(conn)
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

fn migrate(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations(
            version INTEGER PRIMARY KEY,
            applied_at INTEGER NOT NULL
        )",
        [],
    )?;

    let current_version: Option<i32> = conn
        .query_row("SELECT MAX(version) FROM schema_migrations", [], |row| {
            row.get(0)
        })
        .optional()?
        .flatten();

    let current_version = current_version.unwrap_or(0);

    if current_version < 1 {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS repos(
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                path TEXT UNIQUE NOT NULL,
                repo_key TEXT NOT NULL,
                last_opened INTEGER,
                last_seen INTEGER
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS repo_fingerprints(
                repo_id INTEGER PRIMARY KEY,
                head TEXT,
                index_fp TEXT,
                refs_heads TEXT,
                refs_remotes TEXT,
                refs_tags TEXT,
                packed_refs TEXT,
                config TEXT,
                updated_at INTEGER,
                FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS repo_status(
                repo_id INTEGER PRIMARY KEY,
                branch TEXT,
                staged_count INTEGER,
                unstaged_count INTEGER,
                additions INTEGER,
                deletions INTEGER,
                files_changed INTEGER,
                updated_at INTEGER,
                FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS repo_status_files(
                repo_id INTEGER,
                path TEXT,
                old_path TEXT,
                kind TEXT,
                staged INTEGER,
                additions INTEGER,
                deletions INTEGER,
                PRIMARY KEY(repo_id, path, staged),
                FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS commits(
                repo_id INTEGER,
                hash TEXT,
                short_hash TEXT,
                message TEXT,
                author TEXT,
                email TEXT,
                timestamp INTEGER,
                parents_json TEXT,
                ordinal INTEGER,
                PRIMARY KEY(repo_id, hash),
                FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS branches(
                repo_id INTEGER,
                name TEXT,
                is_current INTEGER,
                is_remote INTEGER,
                upstream TEXT,
                tip_hash TEXT,
                PRIMARY KEY(repo_id, name, is_remote),
                FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS remotes(
                repo_id INTEGER,
                name TEXT,
                url TEXT,
                PRIMARY KEY(repo_id, name),
                FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS tags(
                repo_id INTEGER,
                name TEXT,
                target_hash TEXT,
                author TEXT,
                timestamp INTEGER,
                PRIMARY KEY(repo_id, name),
                FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS stashes(
                repo_id INTEGER,
                hash TEXT,
                message TEXT,
                timestamp INTEGER,
                ordinal INTEGER,
                PRIMARY KEY(repo_id, hash),
                FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        tx.execute(
            "CREATE TABLE IF NOT EXISTS github_cache(
                repo_id INTEGER,
                endpoint TEXT,
                etag TEXT,
                fetched_at INTEGER,
                error TEXT,
                payload_json TEXT,
                PRIMARY KEY(repo_id, endpoint),
                FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (1, ?)",
            [now_millis() as i64],
        )?;
        tx.commit()?;
    }

    if current_version < 2 {
        let tx = conn.unchecked_transaction()?;
        tx.execute("DROP TABLE IF EXISTS github_cache", [])?;
        tx.execute(
            "CREATE TABLE github_cache(
                repo_id INTEGER,
                auth_login TEXT NOT NULL,
                endpoint TEXT,
                etag TEXT,
                fetched_at INTEGER,
                error TEXT,
                payload_json TEXT,
                PRIMARY KEY(repo_id, auth_login, endpoint),
                FOREIGN KEY(repo_id) REFERENCES repos(id) ON DELETE CASCADE
            )",
            [],
        )?;
        tx.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (2, ?)",
            [now_millis() as i64],
        )?;
        tx.commit()?;
    }

    Ok(())
}

fn get_or_create_repo(conn: &rusqlite::Connection, path: &str) -> Result<i64, rusqlite::Error> {
    let id: Option<i64> = conn
        .query_row("SELECT id FROM repos WHERE path = ?", [path], |row| {
            row.get(0)
        })
        .optional()?;

    let now = now_millis() as i64;
    if let Some(id) = id {
        conn.execute("UPDATE repos SET last_seen = ? WHERE id = ?", [now, id])?;
        Ok(id)
    } else {
        let mut hasher = Sha256::new();
        hasher.update(path.as_bytes());
        let repo_key = format!("{:x}", hasher.finalize());

        conn.execute(
            "INSERT INTO repos (path, repo_key, last_opened, last_seen) VALUES (?, ?, ?, ?)",
            (path, &repo_key, now, now),
        )?;
        Ok(conn.last_insert_rowid())
    }
}

pub fn evict_old_repos(
    conn: &rusqlite::Connection,
    max_repos: usize,
) -> Result<(), rusqlite::Error> {
    let mut stmt =
        conn.prepare("SELECT id, path FROM repos ORDER BY last_seen DESC, last_opened DESC")?;
    let repo_rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;

    let mut repos = Vec::new();
    for r in repo_rows {
        repos.push(r?);
    }

    if repos.len() > max_repos {
        let to_remove = &repos[max_repos..];
        for (id, path) in to_remove {
            tracing::info!(repo = %path, id = %id, "Evicting repo from SQLite cache");
            conn.execute("DELETE FROM repos WHERE id = ?", [*id])?;
        }
    }
    Ok(())
}

fn format_kind(k: &FileChangeKind) -> &'static str {
    match k {
        FileChangeKind::Added => "Added",
        FileChangeKind::Modified => "Modified",
        FileChangeKind::Deleted => "Deleted",
        FileChangeKind::Renamed => "Renamed",
        FileChangeKind::TypeChanged => "TypeChanged",
    }
}

fn parse_kind(s: &str) -> FileChangeKind {
    match s {
        "Added" => FileChangeKind::Added,
        "Modified" => FileChangeKind::Modified,
        "Deleted" => FileChangeKind::Deleted,
        "Renamed" => FileChangeKind::Renamed,
        "TypeChanged" => FileChangeKind::TypeChanged,
        _ => FileChangeKind::Modified,
    }
}

fn extract_endpoint_error(aggregate: &str, prefixes: &[&str]) -> Option<String> {
    for part in aggregate.split(", ") {
        for prefix in prefixes {
            if let Some(rest) = part.strip_prefix(prefix) {
                return Some(rest.to_string());
            }
        }
    }
    None
}

pub fn save_cache(cache: &DiskCache, auth_login: Option<&str>) -> Result<(), String> {
    let mut conn = open_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    let repo_id = get_or_create_repo(&tx, &cache.repo_path).map_err(|e| e.to_string())?;

    let now = now_millis() as i64;
    tx.execute(
        "UPDATE repos SET last_opened = ?, last_seen = ? WHERE id = ?",
        [now, now, repo_id],
    )
    .map_err(|e| e.to_string())?;

    // Update fingerprint
    let split_fps = compute_repo_fingerprints(&cache.repo_path);
    tx.execute(
        "INSERT OR REPLACE INTO repo_fingerprints (repo_id, head, index_fp, refs_heads, refs_remotes, refs_tags, packed_refs, config, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            repo_id,
            &split_fps.head,
            &split_fps.index,
            &split_fps.refs_heads,
            &split_fps.refs_remotes,
            &split_fps.refs_tags,
            &split_fps.packed_refs,
            &split_fps.config,
            now,
        ),
    ).map_err(|e| e.to_string())?;

    // Save status
    let local = &cache.local_snapshot;
    tx.execute(
        "INSERT OR REPLACE INTO repo_status (repo_id, branch, staged_count, unstaged_count, additions, deletions, files_changed, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (
            repo_id,
            &local.status.branch,
            local.status.staged_count as i64,
            local.status.unstaged_count as i64,
            local.status.additions as i64,
            local.status.deletions as i64,
            local.status.files_changed as i64,
            now,
        ),
    ).map_err(|e| e.to_string())?;

    // Save status files
    tx.execute("DELETE FROM repo_status_files WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for f in &local.status.staged_files {
        tx.execute(
            "INSERT OR REPLACE INTO repo_status_files (repo_id, path, old_path, kind, staged, additions, deletions)
             VALUES (?, ?, ?, ?, 1, ?, ?)",
            (
                repo_id,
                &f.path,
                &f.old_path,
                format_kind(&f.kind),
                f.additions as i64,
                f.deletions as i64,
            ),
        ).map_err(|e| e.to_string())?;
    }
    for f in &local.status.unstaged_files {
        tx.execute(
            "INSERT OR REPLACE INTO repo_status_files (repo_id, path, old_path, kind, staged, additions, deletions)
             VALUES (?, ?, ?, ?, 0, ?, ?)",
            (
                repo_id,
                &f.path,
                &f.old_path,
                format_kind(&f.kind),
                f.additions as i64,
                f.deletions as i64,
            ),
        ).map_err(|e| e.to_string())?;
    }

    // Save commits (limit to 200)
    tx.execute("DELETE FROM commits WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for (i, c) in local.commits.iter().take(200).enumerate() {
        let parents_json = serde_json::to_string(&c.parents).unwrap_or_else(|_| "[]".to_string());
        let c_time = c
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0) as i64;
        tx.execute(
            "INSERT INTO commits (repo_id, hash, short_hash, message, author, email, timestamp, parents_json, ordinal)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                repo_id,
                &c.hash,
                &c.short_hash,
                &c.message,
                &c.author,
                &c.email,
                c_time,
                &parents_json,
                i as i64,
            ),
        ).map_err(|e| e.to_string())?;
    }

    // Save branches
    tx.execute("DELETE FROM branches WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for b in &local.branches {
        tx.execute(
            "INSERT INTO branches (repo_id, name, is_current, is_remote, upstream, tip_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            (
                repo_id,
                &b.name,
                if b.is_current { 1 } else { 0 },
                if b.is_remote { 1 } else { 0 },
                &b.upstream,
                &b.tip_hash,
            ),
        )
        .map_err(|e| e.to_string())?;
    }

    // Save remotes
    tx.execute("DELETE FROM remotes WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for r in &local.remotes {
        tx.execute(
            "INSERT INTO remotes (repo_id, name, url) VALUES (?, ?, ?)",
            (repo_id, &r.name, &r.url),
        )
        .map_err(|e| e.to_string())?;
    }

    // Save tags (limit to 100)
    tx.execute("DELETE FROM tags WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for t in local.tags.iter().take(100) {
        let t_time = t
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0) as i64;
        tx.execute(
            "INSERT INTO tags (repo_id, name, target_hash, author, timestamp)
             VALUES (?, ?, ?, ?, ?)",
            (repo_id, &t.name, &t.target_hash, &t.author, t_time),
        )
        .map_err(|e| e.to_string())?;
    }

    // Save stashes (limit to 50)
    tx.execute("DELETE FROM stashes WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for (i, s) in local.stashes.iter().take(50).enumerate() {
        let s_time = s
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0) as i64;
        tx.execute(
            "INSERT INTO stashes (repo_id, hash, message, timestamp, ordinal)
             VALUES (?, ?, ?, ?, ?)",
            (repo_id, &s.hash, &s.message, s_time, i as i64),
        )
        .map_err(|e| e.to_string())?;
    }

    // Save GitHub cache if present in remote_snapshot and auth_login is present
    if let Some(login) = auth_login {
        if let Some(ref remote) = cache.remote_snapshot {
            let prs_err = remote
                .github_error
                .as_deref()
                .and_then(|err| extract_endpoint_error(err, &["PRs: "]));
            save_github_cache_data(
                &tx,
                repo_id,
                login,
                "prs",
                cache.prs_etag.as_deref(),
                remote.last_refresh.unwrap_or(0),
                prs_err.as_deref(),
                &serde_json::to_string(&remote.pull_requests).unwrap_or_else(|_| "[]".to_string()),
            )?;

            let actions_err = remote
                .github_error
                .as_deref()
                .and_then(|err| extract_endpoint_error(err, &["Actions: "]));
            save_github_cache_data(
                &tx,
                repo_id,
                login,
                "actions",
                cache.actions_etag.as_deref(),
                remote.last_refresh.unwrap_or(0),
                actions_err.as_deref(),
                &serde_json::to_string(&remote.action_runs).unwrap_or_else(|_| "[]".to_string()),
            )?;

            let releases_err = remote
                .github_error
                .as_deref()
                .and_then(|err| extract_endpoint_error(err, &["Releases: "]));
            save_github_cache_data(
                &tx,
                repo_id,
                login,
                "releases",
                cache.releases_etag.as_deref(),
                remote.last_refresh.unwrap_or(0),
                releases_err.as_deref(),
                &serde_json::to_string(&remote.releases).unwrap_or_else(|_| "[]".to_string()),
            )?;

            let containers: Vec<&GitHubPackage> = remote
                .packages
                .iter()
                .filter(|p| p.package_type == "container")
                .collect();
            let container_err = remote.github_error.as_deref().and_then(|err| {
                extract_endpoint_error(err, &["Container Packages: ", "Packages: "])
            });
            save_github_cache_data(
                &tx,
                repo_id,
                login,
                "packages_container",
                cache.packages_container_etag.as_deref(),
                remote.last_refresh.unwrap_or(0),
                container_err.as_deref(),
                &serde_json::to_string(&containers).unwrap_or_else(|_| "[]".to_string()),
            )?;

            let npms: Vec<&GitHubPackage> = remote
                .packages
                .iter()
                .filter(|p| p.package_type == "npm")
                .collect();
            let npm_err = remote
                .github_error
                .as_deref()
                .and_then(|err| extract_endpoint_error(err, &["NPM Packages: ", "Packages: "]));
            save_github_cache_data(
                &tx,
                repo_id,
                login,
                "packages_npm",
                cache.packages_npm_etag.as_deref(),
                remote.last_refresh.unwrap_or(0),
                npm_err.as_deref(),
                &serde_json::to_string(&npms).unwrap_or_else(|_| "[]".to_string()),
            )?;
        }
    }

    if let Err(e) = evict_old_repos(&tx, 20) {
        tracing::error!("Eviction error: {e}");
    }

    tx.commit().map_err(|e| e.to_string())?;
    tracing::debug!(repo = %cache.repo_path, "Successfully saved repository cache to SQLite");
    Ok(())
}

fn save_fingerprints_in_tx(
    tx: &Transaction,
    repo_id: i64,
    fps: &RepoFingerprints,
    now: i64,
) -> Result<(), String> {
    tx.execute(
        "INSERT OR REPLACE INTO repo_fingerprints (repo_id, head, index_fp, refs_heads, refs_remotes, refs_tags, packed_refs, config, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            repo_id,
            &fps.head,
            &fps.index,
            &fps.refs_heads,
            &fps.refs_remotes,
            &fps.refs_tags,
            &fps.packed_refs,
            &fps.config,
            now,
        ),
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn save_status_slice(
    repo_path: &str,
    status: &RepoStatus,
    fps: &RepoFingerprints,
) -> Result<(), String> {
    let mut conn = open_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let repo_id = get_or_create_repo(&tx, repo_path).map_err(|e| e.to_string())?;
    let now = now_millis() as i64;

    tx.execute(
        "UPDATE repos SET last_seen = ? WHERE id = ?",
        [now, repo_id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT OR REPLACE INTO repo_status (repo_id, branch, staged_count, unstaged_count, additions, deletions, files_changed, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        (
            repo_id,
            &status.branch,
            status.staged_count as i64,
            status.unstaged_count as i64,
            status.additions as i64,
            status.deletions as i64,
            status.files_changed as i64,
            now,
        ),
    ).map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM repo_status_files WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for f in &status.staged_files {
        tx.execute(
            "INSERT OR REPLACE INTO repo_status_files (repo_id, path, old_path, kind, staged, additions, deletions)
             VALUES (?, ?, ?, ?, 1, ?, ?)",
            (
                repo_id,
                &f.path,
                &f.old_path,
                format_kind(&f.kind),
                f.additions as i64,
                f.deletions as i64,
            ),
        ).map_err(|e| e.to_string())?;
    }
    for f in &status.unstaged_files {
        tx.execute(
            "INSERT OR REPLACE INTO repo_status_files (repo_id, path, old_path, kind, staged, additions, deletions)
             VALUES (?, ?, ?, ?, 0, ?, ?)",
            (
                repo_id,
                &f.path,
                &f.old_path,
                format_kind(&f.kind),
                f.additions as i64,
                f.deletions as i64,
            ),
        ).map_err(|e| e.to_string())?;
    }

    save_fingerprints_in_tx(&tx, repo_id, fps, now)?;
    tx.commit().map_err(|e| e.to_string())?;
    tracing::debug!(repo = %repo_path, "Successfully saved status slice to SQLite");
    Ok(())
}

pub fn save_commits_slice(
    repo_path: &str,
    commits: &[Commit],
    fps: &RepoFingerprints,
) -> Result<(), String> {
    let mut conn = open_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let repo_id = get_or_create_repo(&tx, repo_path).map_err(|e| e.to_string())?;
    let now = now_millis() as i64;

    tx.execute(
        "UPDATE repos SET last_seen = ? WHERE id = ?",
        [now, repo_id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM commits WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for (i, c) in commits.iter().take(200).enumerate() {
        let parents_json = serde_json::to_string(&c.parents).unwrap_or_else(|_| "[]".to_string());
        let c_time = c
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0) as i64;
        tx.execute(
            "INSERT INTO commits (repo_id, hash, short_hash, message, author, email, timestamp, parents_json, ordinal)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            (
                repo_id,
                &c.hash,
                &c.short_hash,
                &c.message,
                &c.author,
                &c.email,
                c_time,
                &parents_json,
                i as i64,
            ),
        ).map_err(|e| e.to_string())?;
    }

    save_fingerprints_in_tx(&tx, repo_id, fps, now)?;
    tx.commit().map_err(|e| e.to_string())?;
    tracing::debug!(repo = %repo_path, count = commits.len(), "Successfully saved commits slice to SQLite");
    Ok(())
}

pub fn save_refs_slice(
    repo_path: &str,
    branches: &[Branch],
    fps: &RepoFingerprints,
) -> Result<(), String> {
    let mut conn = open_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let repo_id = get_or_create_repo(&tx, repo_path).map_err(|e| e.to_string())?;
    let now = now_millis() as i64;

    tx.execute(
        "UPDATE repos SET last_seen = ? WHERE id = ?",
        [now, repo_id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM branches WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for b in branches {
        tx.execute(
            "INSERT INTO branches (repo_id, name, is_current, is_remote, upstream, tip_hash)
             VALUES (?, ?, ?, ?, ?, ?)",
            (
                repo_id,
                &b.name,
                if b.is_current { 1 } else { 0 },
                if b.is_remote { 1 } else { 0 },
                &b.upstream,
                &b.tip_hash,
            ),
        )
        .map_err(|e| e.to_string())?;
    }

    save_fingerprints_in_tx(&tx, repo_id, fps, now)?;
    tx.commit().map_err(|e| e.to_string())?;
    tracing::debug!(repo = %repo_path, count = branches.len(), "Successfully saved refs/branches slice to SQLite");
    Ok(())
}

pub fn save_remotes_slice(
    repo_path: &str,
    remotes: &[Remote],
    fps: &RepoFingerprints,
) -> Result<(), String> {
    let mut conn = open_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let repo_id = get_or_create_repo(&tx, repo_path).map_err(|e| e.to_string())?;
    let now = now_millis() as i64;

    tx.execute(
        "UPDATE repos SET last_seen = ? WHERE id = ?",
        [now, repo_id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM remotes WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for r in remotes {
        tx.execute(
            "INSERT INTO remotes (repo_id, name, url) VALUES (?, ?, ?)",
            (repo_id, &r.name, &r.url),
        )
        .map_err(|e| e.to_string())?;
    }

    save_fingerprints_in_tx(&tx, repo_id, fps, now)?;
    tx.commit().map_err(|e| e.to_string())?;
    tracing::debug!(repo = %repo_path, count = remotes.len(), "Successfully saved remotes slice to SQLite");
    Ok(())
}

pub fn save_tags_slice(
    repo_path: &str,
    tags: &[Tag],
    fps: &RepoFingerprints,
) -> Result<(), String> {
    let mut conn = open_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let repo_id = get_or_create_repo(&tx, repo_path).map_err(|e| e.to_string())?;
    let now = now_millis() as i64;

    tx.execute(
        "UPDATE repos SET last_seen = ? WHERE id = ?",
        [now, repo_id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM tags WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for t in tags.iter().take(100) {
        let t_time = t
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0) as i64;
        tx.execute(
            "INSERT INTO tags (repo_id, name, target_hash, author, timestamp)
             VALUES (?, ?, ?, ?, ?)",
            (repo_id, &t.name, &t.target_hash, &t.author, t_time),
        )
        .map_err(|e| e.to_string())?;
    }

    save_fingerprints_in_tx(&tx, repo_id, fps, now)?;
    tx.commit().map_err(|e| e.to_string())?;
    tracing::debug!(repo = %repo_path, count = tags.len(), "Successfully saved tags slice to SQLite");
    Ok(())
}

pub fn save_stashes_slice(
    repo_path: &str,
    stashes: &[Stash],
    fps: &RepoFingerprints,
) -> Result<(), String> {
    let mut conn = open_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let repo_id = get_or_create_repo(&tx, repo_path).map_err(|e| e.to_string())?;
    let now = now_millis() as i64;

    tx.execute(
        "UPDATE repos SET last_seen = ? WHERE id = ?",
        [now, repo_id],
    )
    .map_err(|e| e.to_string())?;

    tx.execute("DELETE FROM stashes WHERE repo_id = ?", [repo_id])
        .map_err(|e| e.to_string())?;
    for (i, s) in stashes.iter().take(50).enumerate() {
        let s_time = s
            .timestamp
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0) as i64;
        tx.execute(
            "INSERT INTO stashes (repo_id, hash, message, timestamp, ordinal)
             VALUES (?, ?, ?, ?, ?)",
            (repo_id, &s.hash, &s.message, s_time, i as i64),
        )
        .map_err(|e| e.to_string())?;
    }

    save_fingerprints_in_tx(&tx, repo_id, fps, now)?;
    tx.commit().map_err(|e| e.to_string())?;
    tracing::debug!(repo = %repo_path, count = stashes.len(), "Successfully saved stashes slice to SQLite");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn save_github_cache_data(
    tx: &Transaction,
    repo_id: i64,
    auth_login: &str,
    endpoint: &str,
    etag: Option<&str>,
    fetched_at: u128,
    error: Option<&str>,
    payload_json: &str,
) -> Result<(), String> {
    tx.execute(
        "INSERT OR REPLACE INTO github_cache (repo_id, auth_login, endpoint, etag, fetched_at, error, payload_json)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        (
            repo_id,
            auth_login,
            endpoint,
            etag,
            fetched_at as i64,
            error,
            payload_json,
        ),
    ).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn save_github_cache_entry(
    repo_path: &str,
    auth_login: &str,
    endpoint: &str,
    etag: Option<&str>,
    fetched_at: u128,
    error: Option<&str>,
    payload_json: &str,
) -> Result<(), String> {
    let mut conn = open_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let repo_id = get_or_create_repo(&tx, repo_path).map_err(|e| e.to_string())?;

    tx.execute(
        "INSERT OR REPLACE INTO github_cache (repo_id, auth_login, endpoint, etag, fetched_at, error, payload_json)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
        (
            repo_id,
            auth_login,
            endpoint,
            etag,
            fetched_at as i64,
            error,
            payload_json,
        ),
    ).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn touch_github_cache_entry(
    repo_path: &str,
    auth_login: &str,
    endpoint: &str,
    fetched_at: u128,
    clear_error: bool,
) -> Result<(), String> {
    let mut conn = open_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let repo_id = get_or_create_repo(&tx, repo_path).map_err(|e| e.to_string())?;

    if clear_error {
        tx.execute(
            "UPDATE github_cache SET fetched_at = ?, error = NULL WHERE repo_id = ? AND auth_login = ? AND endpoint = ?",
            (fetched_at as i64, repo_id, auth_login, endpoint),
        ).map_err(|e| e.to_string())?;
    } else {
        tx.execute(
            "UPDATE github_cache SET fetched_at = ? WHERE repo_id = ? AND auth_login = ? AND endpoint = ?",
            (fetched_at as i64, repo_id, auth_login, endpoint),
        ).map_err(|e| e.to_string())?;
    }

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

#[allow(clippy::type_complexity)]
pub fn load_github_cache_entry(
    repo_path: &str,
    auth_login: &str,
    endpoint: &str,
) -> Result<Option<(Option<String>, u128, Option<String>, String)>, String> {
    let conn = open_conn()?;
    let repo_id: Option<i64> = conn
        .query_row("SELECT id FROM repos WHERE path = ?", [repo_path], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;

    let Some(repo_id) = repo_id else {
        return Ok(None);
    };

    let row: Option<(Option<String>, i64, Option<String>, String)> = conn
        .query_row(
            "SELECT etag, fetched_at, error, payload_json FROM github_cache WHERE repo_id = ? AND auth_login = ? AND endpoint = ?",
            (repo_id, auth_login, endpoint),
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some((etag, fetched_at, error, payload_json)) = row {
        Ok(Some((etag, fetched_at as u128, error, payload_json)))
    } else {
        Ok(None)
    }
}

pub fn load_cache(repo_path: &str, auth_login: Option<&str>) -> Option<DiskCache> {
    let conn = match open_conn() {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(repo = %repo_path, err = %e, "Failed to open SQLite database");
            return None;
        }
    };

    let repo_id: Option<i64> = conn
        .query_row("SELECT id FROM repos WHERE path = ?", [repo_path], |row| {
            row.get(0)
        })
        .optional()
        .ok()
        .flatten();

    let repo_id = repo_id?;

    // Load status
    let status_row: Option<(String, i64, i64, i64, i64, i64, i64)> = conn
        .query_row(
            "SELECT branch, staged_count, unstaged_count, additions, deletions, files_changed, updated_at
             FROM repo_status WHERE repo_id = ?",
            [repo_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()
        .ok()
        .flatten();

    let (branch, staged_count, unstaged_count, additions, deletions, files_changed, updated_at) =
        status_row?;

    // Load status files
    let mut stmt = match conn.prepare(
        "SELECT path, old_path, kind, staged, additions, deletions
         FROM repo_status_files WHERE repo_id = ?",
    ) {
        Ok(s) => s,
        Err(_) => return None,
    };

    let file_rows = stmt
        .query_map([repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
                row.get::<_, i64>(4)?,
                row.get::<_, i64>(5)?,
            ))
        })
        .ok()?;

    let mut staged_files = Vec::new();
    let mut unstaged_files = Vec::new();
    for r in file_rows.flatten() {
        let (path, old_path, kind_str, staged_val, add_val, del_val) = r;
        let file_status = FileStatus {
            path,
            old_path,
            kind: parse_kind(&kind_str),
            staged: staged_val == 1,
            additions: add_val as usize,
            deletions: del_val as usize,
        };
        if staged_val == 1 {
            staged_files.push(file_status);
        } else {
            unstaged_files.push(file_status);
        }
    }

    let status = RepoStatus {
        branch,
        staged_count: staged_count as usize,
        unstaged_count: unstaged_count as usize,
        staged_files,
        unstaged_files,
        additions: additions as usize,
        deletions: deletions as usize,
        files_changed: files_changed as usize,
    };

    // Load commits
    let mut stmt = match conn.prepare(
        "SELECT hash, short_hash, message, author, email, timestamp, parents_json
         FROM commits WHERE repo_id = ? ORDER BY ordinal ASC",
    ) {
        Ok(s) => s,
        Err(_) => return None,
    };

    let commit_rows = stmt
        .query_map([repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, i64>(5)?,
                row.get::<_, String>(6)?,
            ))
        })
        .ok()?;

    let mut commits = Vec::new();
    for r in commit_rows.flatten() {
        let (hash, short_hash, message, author, email, timestamp_val, parents_json) = r;
        let parents = serde_json::from_str(&parents_json).unwrap_or_default();
        let timestamp =
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp_val as u64);
        commits.push(Commit {
            hash,
            short_hash,
            message,
            author,
            email,
            timestamp,
            parents,
        });
    }

    // Load branches
    let mut stmt = match conn.prepare(
        "SELECT name, is_current, is_remote, upstream, tip_hash
         FROM branches WHERE repo_id = ?",
    ) {
        Ok(s) => s,
        Err(_) => return None,
    };

    let branch_rows = stmt
        .query_map([repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
            ))
        })
        .ok()?;

    let mut branches = Vec::new();
    for r in branch_rows.flatten() {
        let (name, is_current_val, is_remote_val, upstream, tip_hash) = r;
        branches.push(Branch {
            name,
            is_current: is_current_val == 1,
            is_remote: is_remote_val == 1,
            upstream,
            tip_hash,
        });
    }

    // Load remotes
    let mut stmt = match conn.prepare("SELECT name, url FROM remotes WHERE repo_id = ?") {
        Ok(s) => s,
        Err(_) => return None,
    };

    let remote_rows = stmt
        .query_map([repo_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .ok()?;

    let mut remotes = Vec::new();
    for r in remote_rows.flatten() {
        let (name, url) = r;
        remotes.push(Remote { name, url });
    }

    // Load tags
    let mut stmt = match conn
        .prepare("SELECT name, target_hash, author, timestamp FROM tags WHERE repo_id = ?")
    {
        Ok(s) => s,
        Err(_) => return None,
    };

    let tag_rows = stmt
        .query_map([repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .ok()?;

    let mut tags = Vec::new();
    for r in tag_rows.flatten() {
        let (name, target_hash, author, timestamp_val) = r;
        let timestamp =
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp_val as u64);
        tags.push(Tag {
            name,
            target_hash,
            author,
            timestamp,
        });
    }

    // Load stashes
    let mut stmt = match conn.prepare(
        "SELECT hash, message, timestamp FROM stashes WHERE repo_id = ? ORDER BY ordinal ASC",
    ) {
        Ok(s) => s,
        Err(_) => return None,
    };

    let stash_rows = stmt
        .query_map([repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })
        .ok()?;

    let mut stashes = Vec::new();
    for r in stash_rows.flatten() {
        let (hash, message, timestamp_val) = r;
        let timestamp =
            SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp_val as u64);
        stashes.push(Stash {
            hash,
            message,
            timestamp,
        });
    }

    let ownership = classify_repo_ownership(&remotes, auth_login);

    let local_snapshot = BoundedLocalSnapshot {
        commits,
        branches,
        remotes,
        tags,
        stashes,
        status,
        repo_error: None,
        last_refresh: Some(updated_at as u128),
        ownership,
    };

    // Load cached github fields
    let mut pull_requests = Vec::new();
    let mut action_runs = Vec::new();
    let mut releases = Vec::new();
    let mut packages = Vec::new();
    let mut errors = Vec::new();
    let mut max_fetched_at = 0u128;
    let mut prs_etag = None;
    let mut actions_etag = None;
    let mut releases_etag = None;
    let mut packages_container_etag = None;
    let mut packages_npm_etag = None;

    if let Some(login) = auth_login {
        if let Ok(Some((etag, fetched_at, err, payload))) =
            load_github_cache_entry(repo_path, login, "prs")
        {
            prs_etag = etag;
            if let Ok(data) = serde_json::from_str::<Vec<GitHubPullRequest>>(&payload) {
                pull_requests = data;
            }
            if let Some(e) = err {
                errors.push(format!("PRs: {e}"));
            }
            if fetched_at > max_fetched_at {
                max_fetched_at = fetched_at;
            }
        }

        if let Ok(Some((etag, fetched_at, err, payload))) =
            load_github_cache_entry(repo_path, login, "actions")
        {
            actions_etag = etag;
            if let Ok(data) = serde_json::from_str::<Vec<GitHubActionRun>>(&payload) {
                action_runs = data;
            }
            if let Some(e) = err {
                errors.push(format!("Actions: {e}"));
            }
            if fetched_at > max_fetched_at {
                max_fetched_at = fetched_at;
            }
        }

        if let Ok(Some((etag, fetched_at, err, payload))) =
            load_github_cache_entry(repo_path, login, "releases")
        {
            releases_etag = etag;
            if let Ok(data) = serde_json::from_str::<Vec<GitHubRelease>>(&payload) {
                releases = data;
            }
            if let Some(e) = err {
                errors.push(format!("Releases: {e}"));
            }
            if fetched_at > max_fetched_at {
                max_fetched_at = fetched_at;
            }
        }

        if let Ok(Some((etag, fetched_at, err, payload))) =
            load_github_cache_entry(repo_path, login, "packages_container")
        {
            packages_container_etag = etag;
            if let Ok(data) = serde_json::from_str::<Vec<GitHubPackage>>(&payload) {
                packages.extend(data);
            }
            if let Some(e) = err {
                errors.push(format!("Container Packages: {e}"));
            }
            if fetched_at > max_fetched_at {
                max_fetched_at = fetched_at;
            }
        }

        if let Ok(Some((etag, fetched_at, err, payload))) =
            load_github_cache_entry(repo_path, login, "packages_npm")
        {
            packages_npm_etag = etag;
            if let Ok(data) = serde_json::from_str::<Vec<GitHubPackage>>(&payload) {
                packages.extend(data);
            }
            if let Some(e) = err {
                errors.push(format!("NPM Packages: {e}"));
            }
            if fetched_at > max_fetched_at {
                max_fetched_at = fetched_at;
            }
        }
    }

    let github_error = if errors.is_empty() {
        None
    } else {
        Some(errors.join(", "))
    };

    let remote_snapshot = if max_fetched_at > 0 && auth_login.is_some() {
        let repo_ownership = match ownership {
            Some(true) => RepoOwnership::Owned,
            Some(false) => RepoOwnership::External,
            None => RepoOwnership::Unknown,
        };
        Some(RepoRemoteSnapshot {
            pull_requests,
            action_runs,
            releases,
            packages,
            github_error,
            last_refresh: Some(max_fetched_at),
            ownership: repo_ownership,
        })
    } else {
        None
    };

    // Load fingerprint
    let repo_fingerprint: String = conn
        .query_row(
            "SELECT head || ':' || index_fp || ':' || refs_heads || ':' || refs_remotes || ':' || refs_tags || ':' || packed_refs || ':' || config
             FROM repo_fingerprints WHERE repo_id = ?",
            [repo_id],
            |row| row.get(0),
        )
        .unwrap_or_default();

    tracing::info!(
        repo = %repo_path,
        auth_login = ?auth_login,
        has_remote = remote_snapshot.is_some(),
        ownership = ?ownership,
        "Successfully hydrated repository cache from SQLite"
    );
    Some(DiskCache {
        schema_version: SCHEMA_VERSION,
        repo_path: repo_path.to_string(),
        repo_fingerprint,
        captured_at: updated_at as u128,
        local_snapshot,
        remote_snapshot,
        prs_etag,
        actions_etag,
        releases_etag,
        packages_container_etag,
        packages_npm_etag,
    })
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

pub fn save_fingerprints(repo_path: &str, fps: &RepoFingerprints) -> Result<(), String> {
    let mut conn = open_conn()?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    let repo_id = get_or_create_repo(&tx, repo_path).map_err(|e| e.to_string())?;
    let now = now_millis() as i64;

    tx.execute(
        "INSERT OR REPLACE INTO repo_fingerprints (repo_id, head, index_fp, refs_heads, refs_remotes, refs_tags, packed_refs, config, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        (
            repo_id,
            &fps.head,
            &fps.index,
            &fps.refs_heads,
            &fps.refs_remotes,
            &fps.refs_tags,
            &fps.packed_refs,
            &fps.config,
            now,
        ),
    ).map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn load_fingerprints(repo_path: &str) -> Result<Option<RepoFingerprints>, String> {
    let conn = open_conn()?;
    let repo_id: Option<i64> = conn
        .query_row("SELECT id FROM repos WHERE path = ?", [repo_path], |row| {
            row.get(0)
        })
        .optional()
        .map_err(|e| e.to_string())?;

    let Some(repo_id) = repo_id else {
        return Ok(None);
    };

    let row: Option<(String, String, String, String, String, String, String)> = conn
        .query_row(
            "SELECT head, index_fp, refs_heads, refs_remotes, refs_tags, packed_refs, config FROM repo_fingerprints WHERE repo_id = ?",
            [repo_id],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;

    if let Some((head, index, refs_heads, refs_remotes, refs_tags, packed_refs, config)) = row {
        Ok(Some(RepoFingerprints {
            head,
            index,
            refs_heads,
            refs_remotes,
            refs_tags,
            packed_refs,
            config,
        }))
    } else {
        Ok(None)
    }
}

pub fn compute_repo_fingerprint(repo_path: &str) -> String {
    let fp = compute_repo_fingerprints(repo_path);
    format!(
        "{}:{}:{}:{}:{}:{}:{}",
        fp.head, fp.index, fp.refs_heads, fp.refs_remotes, fp.refs_tags, fp.packed_refs, fp.config
    )
}

pub fn compute_repo_fingerprints(repo_path: &str) -> RepoFingerprints {
    let git_dir = Path::new(repo_path).join(".git");
    let actual_git_dir = if git_dir.exists() {
        git_dir
    } else {
        let alt = Path::new(repo_path);
        if alt.join("HEAD").exists() {
            alt.to_path_buf()
        } else {
            return RepoFingerprints::default();
        }
    };

    let head = get_file_fingerprint(&actual_git_dir.join("HEAD")).unwrap_or_default();
    let index = get_file_fingerprint(&actual_git_dir.join("index")).unwrap_or_default();
    let packed_refs = get_file_fingerprint(&actual_git_dir.join("packed-refs")).unwrap_or_default();
    let config = get_file_fingerprint(&actual_git_dir.join("config")).unwrap_or_default();

    let fetch_head = get_file_fingerprint(&actual_git_dir.join("FETCH_HEAD")).unwrap_or_default();
    let mut refs_remotes_parts = Vec::new();
    if !fetch_head.is_empty() {
        refs_remotes_parts.push(fetch_head);
    }
    append_dir_fingerprint(
        &actual_git_dir.join("refs/remotes"),
        &actual_git_dir,
        &mut refs_remotes_parts,
    );
    refs_remotes_parts.sort();
    let refs_remotes = hash_parts(&refs_remotes_parts);

    let mut refs_heads_parts = Vec::new();
    append_dir_fingerprint(
        &actual_git_dir.join("refs/heads"),
        &actual_git_dir,
        &mut refs_heads_parts,
    );
    refs_heads_parts.sort();
    let refs_heads = hash_parts(&refs_heads_parts);

    let mut refs_tags_parts = Vec::new();
    append_dir_fingerprint(
        &actual_git_dir.join("refs/tags"),
        &actual_git_dir,
        &mut refs_tags_parts,
    );
    refs_tags_parts.sort();
    let refs_tags = hash_parts(&refs_tags_parts);

    RepoFingerprints {
        head,
        index,
        refs_heads,
        refs_remotes,
        refs_tags,
        packed_refs,
        config,
    }
}

fn hash_parts(parts: &[String]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let mut hasher = Sha256::new();
    for p in parts {
        hasher.update(p.as_bytes());
        hasher.update(b"\n");
    }
    format!("{:x}", hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct TestDbGuard;

    impl TestDbGuard {
        fn new(path: std::path::PathBuf) -> Self {
            super::TEST_DB_PATH.with(|p| *p.borrow_mut() = Some(path));
            Self
        }
    }

    impl Drop for TestDbGuard {
        fn drop(&mut self) {
            super::TEST_DB_PATH.with(|p| *p.borrow_mut() = None);
        }
    }

    #[test]
    fn test_sqlite_migrations() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        // Check if tables exist
        let count: i32 = conn
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='repos'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn test_fingerprints_split() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_split_fp_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp_dir.join(".git")).unwrap();
        let path_str = temp_dir.to_str().unwrap();

        let fp1 = compute_repo_fingerprints(path_str);

        fs::write(temp_dir.join(".git/HEAD"), "ref: refs/heads/main").unwrap();
        let fp2 = compute_repo_fingerprints(path_str);
        assert_ne!(fp1.head, fp2.head);
        assert_eq!(fp1.index, fp2.index);

        fs::write(temp_dir.join(".git/index"), "dummy index content").unwrap();
        let fp3 = compute_repo_fingerprints(path_str);
        assert_ne!(fp2.index, fp3.index);
        assert_eq!(fp2.head, fp3.head);

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_sqlite_roundtrip() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_rt_test_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp_dir.join(".git")).unwrap();
        let _guard = TestDbGuard::new(temp_dir.join("test.db"));
        let path_str = temp_dir.to_str().unwrap().to_string();

        let local = BoundedLocalSnapshot {
            commits: vec![Commit {
                hash: "1234567890abcdef".to_string(),
                short_hash: "1234567".to_string(),
                message: "feat: roundtrip test".to_string(),
                author: "Test User".to_string(),
                email: "test@example.com".to_string(),
                timestamp: SystemTime::UNIX_EPOCH,
                parents: vec!["parent".to_string()],
            }],
            branches: vec![Branch {
                name: "main".to_string(),
                is_current: true,
                is_remote: false,
                upstream: None,
                tip_hash: "1234567".to_string(),
            }],
            remotes: vec![Remote {
                name: "origin".to_string(),
                url: "git@github.com:example/repo.git".to_string(),
            }],
            tags: vec![Tag {
                name: "v1.0.0".to_string(),
                target_hash: "1234567".to_string(),
                author: "Test Author".to_string(),
                timestamp: SystemTime::UNIX_EPOCH,
            }],
            stashes: vec![Stash {
                message: "stash msg".to_string(),
                hash: "stash12".to_string(),
                timestamp: SystemTime::UNIX_EPOCH,
            }],
            status: RepoStatus {
                branch: "main".to_string(),
                staged_count: 1,
                unstaged_count: 2,
                staged_files: vec![FileStatus {
                    path: "staged.txt".to_string(),
                    old_path: None,
                    kind: FileChangeKind::Added,
                    staged: true,
                    additions: 10,
                    deletions: 0,
                }],
                unstaged_files: vec![FileStatus {
                    path: "unstaged.txt".to_string(),
                    old_path: None,
                    kind: FileChangeKind::Modified,
                    staged: false,
                    additions: 5,
                    deletions: 2,
                }],
                additions: 15,
                deletions: 2,
                files_changed: 2,
            },
            repo_error: None,
            last_refresh: None,
            ownership: Some(true),
        };

        let dc = DiskCache {
            schema_version: SCHEMA_VERSION,
            repo_path: path_str.clone(),
            repo_fingerprint: "dummy-fp".to_string(),
            captured_at: 1000,
            local_snapshot: local,
            remote_snapshot: Some(RepoRemoteSnapshot {
                pull_requests: vec![GitHubPullRequest {
                    number: 42,
                    title: "PR Title".to_string(),
                    state: "open".to_string(),
                    user_login: "user".to_string(),
                    html_url: "url".to_string(),
                    head_ref: "feature".to_string(),
                    base_ref: "main".to_string(),
                    draft: false,
                }],
                action_runs: vec![GitHubActionRun {
                    id: 999,
                    name: "Workflow".to_string(),
                    status: "completed".to_string(),
                    conclusion: Some("success".to_string()),
                    html_url: "runurl".to_string(),
                    head_branch: "feature".to_string(),
                }],
                releases: vec![GitHubRelease {
                    tag_name: "v1.0.0".to_string(),
                    name: Some("v1.0.0 Release".to_string()),
                    html_url: "relurl".to_string(),
                    draft: false,
                    prerelease: false,
                    body: Some("release notes".to_string()),
                }],
                packages: vec![GitHubPackage {
                    name: "pkg".to_string(),
                    package_type: "npm".to_string(),
                    html_url: "pkgurl".to_string(),
                }],
                github_error: None,
                last_refresh: Some(2000),
                ownership: RepoOwnership::Owned,
            }),
            prs_etag: Some("etag-1".to_string()),
            actions_etag: None,
            releases_etag: None,
            packages_container_etag: None,
            packages_npm_etag: None,
        };

        save_cache(&dc, Some("user")).unwrap();

        let loaded = load_cache(&path_str, Some("user")).unwrap();
        assert_eq!(loaded.repo_path, path_str);
        assert_eq!(loaded.local_snapshot.commits.len(), 1);
        assert_eq!(
            loaded.local_snapshot.commits[0].message,
            "feat: roundtrip test"
        );
        assert_eq!(
            loaded.local_snapshot.status.staged_files[0].path,
            "staged.txt"
        );
        assert_eq!(
            loaded.local_snapshot.status.unstaged_files[0].path,
            "unstaged.txt"
        );

        let remote = loaded.remote_snapshot.unwrap();
        assert_eq!(remote.pull_requests.len(), 1);
        assert_eq!(remote.pull_requests[0].title, "PR Title");
        assert_eq!(remote.action_runs.len(), 1);
        assert_eq!(remote.releases.len(), 1);
        assert_eq!(remote.packages.len(), 1);
        assert_eq!(loaded.prs_etag.as_deref(), Some("etag-1"));

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_github_cache_robustness() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_gh_test_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp_dir.join(".git")).unwrap();
        let _guard = TestDbGuard::new(temp_dir.join("test.db"));
        let path_str = temp_dir.to_str().unwrap().to_string();

        let conn = open_conn().unwrap();
        let _repo_id = get_or_create_repo(&conn, &path_str).unwrap();

        // 1. Initial save
        save_github_cache_entry(
            &path_str,
            "alice",
            "prs",
            Some("etag-initial"),
            1000,
            None,
            "[\"initial\"]",
        )
        .unwrap();

        let (etag, fetched_at, error, payload) = load_github_cache_entry(&path_str, "alice", "prs")
            .unwrap()
            .unwrap();
        assert_eq!(etag.as_deref(), Some("etag-initial"));
        assert_eq!(fetched_at, 1000);
        assert_eq!(error, None);
        assert_eq!(payload, "[\"initial\"]");

        // 2. Mock 304 - update fetched_at but preserve old payload
        save_github_cache_entry(
            &path_str,
            "alice",
            "prs",
            Some("etag-initial"),
            2000,
            None,
            &payload,
        )
        .unwrap();
        let (_, fetched_at, _, payload2) = load_github_cache_entry(&path_str, "alice", "prs")
            .unwrap()
            .unwrap();
        assert_eq!(fetched_at, 2000);
        assert_eq!(payload2, "[\"initial\"]"); // payload remains identical

        // 3. Mock API error - store error separately while keeping last good payload
        save_github_cache_entry(
            &path_str,
            "alice",
            "prs",
            Some("etag-initial"),
            3000,
            Some("Rate Limit Exceeded"),
            &payload2,
        )
        .unwrap();
        let (_, fetched_at3, error3, payload3) = load_github_cache_entry(&path_str, "alice", "prs")
            .unwrap()
            .unwrap();
        assert_eq!(fetched_at3, 3000);
        assert_eq!(error3.as_deref(), Some("Rate Limit Exceeded"));
        assert_eq!(payload3, "[\"initial\"]"); // payload still preserved

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_cache_eviction() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        // Insert 25 repos with sequential last_seen values
        for i in 0..25 {
            let path = format!("/path/to/repo/{}", i);
            let mut hasher = Sha256::new();
            hasher.update(path.as_bytes());
            let repo_key = format!("{:x}", hasher.finalize());
            let time_val = i as i64 * 1000;
            conn.execute(
                "INSERT INTO repos (path, repo_key, last_opened, last_seen) VALUES (?, ?, ?, ?)",
                (path, repo_key, time_val, time_val),
            )
            .unwrap();
        }

        // Trigger eviction to keep 20 repos
        evict_old_repos(&conn, 20).unwrap();

        // Verify count is 20
        let count: i32 = conn
            .query_row("SELECT count(*) FROM repos", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 20);

        // Verify repos 0 to 4 were evicted (they had the smallest last_seen times)
        let min_id_path: String = conn
            .query_row(
                "SELECT path FROM repos ORDER BY last_seen ASC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(min_id_path, "/path/to/repo/5");
    }

    #[test]
    fn test_github_cache_user_scoped() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_scope_test_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp_dir.join(".git")).unwrap();
        let _guard = TestDbGuard::new(temp_dir.join("test.db"));
        let path_str = temp_dir.to_str().unwrap().to_string();

        let local = BoundedLocalSnapshot {
            commits: vec![],
            branches: vec![],
            remotes: vec![],
            tags: vec![],
            stashes: vec![],
            status: RepoStatus {
                branch: "main".to_string(),
                staged_count: 0,
                unstaged_count: 0,
                staged_files: vec![],
                unstaged_files: vec![],
                additions: 0,
                deletions: 0,
                files_changed: 0,
            },
            repo_error: None,
            last_refresh: None,
            ownership: Some(true),
        };

        // Create a cache structure with remote snapshot data
        let dc = DiskCache {
            schema_version: SCHEMA_VERSION,
            repo_path: path_str.clone(),
            repo_fingerprint: "dummy-fp".to_string(),
            captured_at: 1000,
            local_snapshot: local,
            remote_snapshot: Some(RepoRemoteSnapshot {
                pull_requests: vec![GitHubPullRequest {
                    number: 42,
                    title: "PR Alice".to_string(),
                    state: "open".to_string(),
                    user_login: "alice".to_string(),
                    html_url: "url".to_string(),
                    head_ref: "feature".to_string(),
                    base_ref: "main".to_string(),
                    draft: false,
                }],
                action_runs: vec![],
                releases: vec![],
                packages: vec![],
                github_error: None,
                last_refresh: Some(2000),
                ownership: RepoOwnership::Owned,
            }),
            prs_etag: Some("etag-alice".to_string()),
            actions_etag: None,
            releases_etag: None,
            packages_container_etag: None,
            packages_npm_etag: None,
        };

        // 1. Save cache with user "alice"
        save_cache(&dc, Some("alice")).unwrap();

        // 2. Loading with "bob" should NOT load the remote snapshot, and should have prs_etag = None
        let loaded_bob = load_cache(&path_str, Some("bob")).unwrap();
        assert!(loaded_bob.remote_snapshot.is_none());
        assert_eq!(loaded_bob.prs_etag, None);

        // 3. Loading with None should NOT load the remote snapshot, and should have prs_etag = None
        let loaded_none = load_cache(&path_str, None).unwrap();
        assert!(loaded_none.remote_snapshot.is_none());
        assert_eq!(loaded_none.prs_etag, None);

        // 4. Loading with "alice" should load it successfully
        let loaded_alice = load_cache(&path_str, Some("alice")).unwrap();
        let remote = loaded_alice.remote_snapshot.unwrap();
        assert_eq!(remote.pull_requests.len(), 1);
        assert_eq!(remote.pull_requests[0].title, "PR Alice");
        assert_eq!(loaded_alice.prs_etag.as_deref(), Some("etag-alice"));

        // 5. Verify local cache is still loaded regardless of auth login (even with None or "bob")
        assert_eq!(loaded_bob.local_snapshot.status.branch, "main");
        assert_eq!(loaded_none.local_snapshot.status.branch, "main");

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_slice_specific_persistence() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_slice_test_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp_dir.join(".git")).unwrap();
        let _guard = TestDbGuard::new(temp_dir.join("test.db"));
        let path_str = temp_dir.to_str().unwrap().to_string();

        let local = BoundedLocalSnapshot {
            commits: vec![Commit {
                hash: "c1".to_string(),
                short_hash: "c1".to_string(),
                message: "initial commit".to_string(),
                author: "author".to_string(),
                email: "email".to_string(),
                timestamp: SystemTime::UNIX_EPOCH,
                parents: vec![],
            }],
            branches: vec![Branch {
                name: "main".to_string(),
                is_current: true,
                is_remote: false,
                upstream: None,
                tip_hash: "c1".to_string(),
            }],
            remotes: vec![],
            tags: vec![Tag {
                name: "v1.0.0".to_string(),
                target_hash: "c1".to_string(),
                author: "author".to_string(),
                timestamp: SystemTime::UNIX_EPOCH,
            }],
            stashes: vec![],
            status: RepoStatus {
                branch: "main".to_string(),
                staged_count: 0,
                unstaged_count: 0,
                staged_files: vec![],
                unstaged_files: vec![],
                additions: 0,
                deletions: 0,
                files_changed: 0,
            },
            repo_error: None,
            last_refresh: None,
            ownership: Some(true),
        };

        let dc = DiskCache {
            schema_version: SCHEMA_VERSION,
            repo_path: path_str.clone(),
            repo_fingerprint: "fp-initial".to_string(),
            captured_at: 1000,
            local_snapshot: local,
            remote_snapshot: None,
            prs_etag: None,
            actions_etag: None,
            releases_etag: None,
            packages_container_etag: None,
            packages_npm_etag: None,
        };

        // First full save
        save_cache(&dc, None).unwrap();

        // 1. Test status-only persistence (only changes status tables, does not alter commits table)
        let new_status = RepoStatus {
            branch: "feature-branch".to_string(),
            staged_count: 5,
            unstaged_count: 2,
            staged_files: vec![],
            unstaged_files: vec![],
            additions: 50,
            deletions: 10,
            files_changed: 1,
        };
        let fp_status = RepoFingerprints {
            head: "fp-status-head".to_string(),
            index: "fp-status-index".to_string(),
            refs_heads: "fp-status-heads".to_string(),
            refs_remotes: "fp-status-remotes".to_string(),
            refs_tags: "fp-status-tags".to_string(),
            packed_refs: "fp-status-packed".to_string(),
            config: "fp-status-config".to_string(),
        };

        save_status_slice(&path_str, &new_status, &fp_status).unwrap();

        // Reload and assert
        let loaded = load_cache(&path_str, None).unwrap();
        // status is updated
        assert_eq!(loaded.local_snapshot.status.branch, "feature-branch");
        assert_eq!(loaded.local_snapshot.status.staged_count, 5);
        assert_eq!(loaded.local_snapshot.status.additions, 50);
        // commits are NOT altered (still 1 commit)
        assert_eq!(loaded.local_snapshot.commits.len(), 1);
        assert_eq!(loaded.local_snapshot.commits[0].hash, "c1");

        // 2. Test tag-only persistence (only changes tags table, does not alter status table)
        let new_tags = vec![
            Tag {
                name: "v2.0.0".to_string(),
                target_hash: "c2".to_string(),
                author: "new author".to_string(),
                timestamp: SystemTime::UNIX_EPOCH,
            },
            Tag {
                name: "v3.0.0".to_string(),
                target_hash: "c3".to_string(),
                author: "new author".to_string(),
                timestamp: SystemTime::UNIX_EPOCH,
            },
        ];
        let fp_tags = RepoFingerprints {
            head: "fp-status-head".to_string(),
            index: "fp-status-index".to_string(),
            refs_heads: "fp-status-heads".to_string(),
            refs_remotes: "fp-status-remotes".to_string(),
            refs_tags: "fp-tags-tags".to_string(),
            packed_refs: "fp-status-packed".to_string(),
            config: "fp-status-config".to_string(),
        };

        save_tags_slice(&path_str, &new_tags, &fp_tags).unwrap();

        let loaded = load_cache(&path_str, None).unwrap();
        // tags are updated
        assert_eq!(loaded.local_snapshot.tags.len(), 2);
        assert_eq!(loaded.local_snapshot.tags[0].name, "v2.0.0");
        assert_eq!(loaded.local_snapshot.tags[1].name, "v3.0.0");
        // status is NOT altered (still "feature-branch")
        assert_eq!(loaded.local_snapshot.status.branch, "feature-branch");

        // 3. Test ref-only persistence (only changes branches table, does not alter tags table)
        let new_branches = vec![
            Branch {
                name: "main-new".to_string(),
                is_current: false,
                is_remote: false,
                upstream: None,
                tip_hash: "c1".to_string(),
            },
            Branch {
                name: "feature-branch".to_string(),
                is_current: true,
                is_remote: false,
                upstream: None,
                tip_hash: "c2".to_string(),
            },
        ];
        let fp_branches = RepoFingerprints {
            head: "fp-branches-head".to_string(),
            index: "fp-status-index".to_string(),
            refs_heads: "fp-branches-heads".to_string(),
            refs_remotes: "fp-status-remotes".to_string(),
            refs_tags: "fp-tags-tags".to_string(),
            packed_refs: "fp-status-packed".to_string(),
            config: "fp-status-config".to_string(),
        };

        save_refs_slice(&path_str, &new_branches, &fp_branches).unwrap();

        let loaded = load_cache(&path_str, None).unwrap();
        // branches/refs are updated
        assert_eq!(loaded.local_snapshot.branches.len(), 2);
        assert_eq!(loaded.local_snapshot.branches[0].name, "feature-branch");
        assert_eq!(loaded.local_snapshot.branches[1].name, "main-new");
        // tags are NOT altered (still 2 tags)
        assert_eq!(loaded.local_snapshot.tags.len(), 2);
        assert_eq!(loaded.local_snapshot.tags[0].name, "v2.0.0");

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_touch_github_cache_entry() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_touch_test_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp_dir.join(".git")).unwrap();
        let _guard = TestDbGuard::new(temp_dir.join("test.db"));
        let path_str = temp_dir.to_str().unwrap().to_string();

        let conn = open_conn().unwrap();
        let _repo_id = get_or_create_repo(&conn, &path_str).unwrap();

        // 1. Initial save with error
        save_github_cache_entry(
            &path_str,
            "alice",
            "prs",
            Some("etag-initial"),
            1000,
            Some("Rate Limit Exceeded"),
            "[\"initial\"]",
        )
        .unwrap();

        // 2. Touch to mock a successful 304 response (updates fetched_at, clears prior error, preserves payload and ETag)
        touch_github_cache_entry(&path_str, "alice", "prs", 2000, true).unwrap();

        let (etag, fetched_at, error, payload) = load_github_cache_entry(&path_str, "alice", "prs")
            .unwrap()
            .unwrap();
        assert_eq!(etag.as_deref(), Some("etag-initial"));
        assert_eq!(fetched_at, 2000);
        assert_eq!(error, None); // error is cleared
        assert_eq!(payload, "[\"initial\"]"); // payload is preserved

        // 3. Touch with clear_error = false
        // First save with error again
        save_github_cache_entry(
            &path_str,
            "alice",
            "prs",
            Some("etag-initial"),
            2000,
            Some("Another Error"),
            "[\"initial\"]",
        )
        .unwrap();

        // Touch with clear_error = false
        touch_github_cache_entry(&path_str, "alice", "prs", 3000, false).unwrap();

        let (_, fetched_at, error, _) = load_github_cache_entry(&path_str, "alice", "prs")
            .unwrap()
            .unwrap();
        assert_eq!(fetched_at, 3000);
        assert_eq!(error.as_deref(), Some("Another Error")); // error is not cleared

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_ownership_hydration_scenarios() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_owner_test_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp_dir.join(".git")).unwrap();
        let _guard = TestDbGuard::new(temp_dir.join("test.db"));
        let path_str = temp_dir.to_str().unwrap().to_string();

        let local = BoundedLocalSnapshot {
            commits: vec![],
            branches: vec![],
            remotes: vec![Remote {
                name: "origin".to_string(),
                url: "git@github.com:alice/project.git".to_string(),
            }],
            tags: vec![],
            stashes: vec![],
            status: RepoStatus {
                branch: "main".to_string(),
                staged_count: 0,
                unstaged_count: 0,
                staged_files: vec![],
                unstaged_files: vec![],
                additions: 0,
                deletions: 0,
                files_changed: 0,
            },
            repo_error: None,
            last_refresh: None,
            ownership: None,
        };

        let dc = DiskCache {
            schema_version: SCHEMA_VERSION,
            repo_path: path_str.clone(),
            repo_fingerprint: "dummy-fp".to_string(),
            captured_at: 1000,
            local_snapshot: local,
            remote_snapshot: Some(RepoRemoteSnapshot {
                pull_requests: vec![],
                action_runs: vec![],
                releases: vec![],
                packages: vec![],
                github_error: None,
                last_refresh: Some(2000),
                ownership: RepoOwnership::Unknown,
            }),
            prs_etag: None,
            actions_etag: None,
            releases_etag: None,
            packages_container_etag: None,
            packages_npm_etag: None,
        };

        save_cache(&dc, Some("alice")).unwrap();

        // 1. load_cache(path, Some("alice")) -> local ownership Some(true), remote ownership Owned
        let loaded_alice = load_cache(&path_str, Some("alice")).unwrap();
        assert_eq!(loaded_alice.local_snapshot.ownership, Some(true));
        assert_eq!(
            loaded_alice.remote_snapshot.unwrap().ownership,
            RepoOwnership::Owned
        );

        // 2. load_cache(path, Some("bob")) -> local ownership Some(false), remote_snapshot is None
        let loaded_bob = load_cache(&path_str, Some("bob")).unwrap();
        assert_eq!(loaded_bob.local_snapshot.ownership, Some(false));
        assert!(loaded_bob.remote_snapshot.is_none());

        // 3. load_cache(path, None) -> local ownership None, remote_snapshot is None
        let loaded_none = load_cache(&path_str, None).unwrap();
        assert_eq!(loaded_none.local_snapshot.ownership, None);
        assert!(loaded_none.remote_snapshot.is_none());

        fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_endpoint_error_isolation() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_err_iso_test_{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(temp_dir.join(".git")).unwrap();
        let _guard = TestDbGuard::new(temp_dir.join("test.db"));
        let path_str = temp_dir.to_str().unwrap().to_string();

        let local = BoundedLocalSnapshot {
            commits: vec![],
            branches: vec![],
            remotes: vec![Remote {
                name: "origin".to_string(),
                url: "git@github.com:alice/project.git".to_string(),
            }],
            tags: vec![],
            stashes: vec![],
            status: RepoStatus {
                branch: "main".to_string(),
                staged_count: 0,
                unstaged_count: 0,
                staged_files: vec![],
                unstaged_files: vec![],
                additions: 0,
                deletions: 0,
                files_changed: 0,
            },
            repo_error: None,
            last_refresh: None,
            ownership: None,
        };

        // 1. Save single error
        let mut dc = DiskCache {
            schema_version: SCHEMA_VERSION,
            repo_path: path_str.clone(),
            repo_fingerprint: "dummy-fp".to_string(),
            captured_at: 1000,
            local_snapshot: local.clone(),
            remote_snapshot: Some(RepoRemoteSnapshot {
                pull_requests: vec![],
                action_runs: vec![],
                releases: vec![],
                packages: vec![],
                github_error: Some("PRs: Rate Limit Exceeded".to_string()),
                last_refresh: Some(2000),
                ownership: RepoOwnership::Owned,
            }),
            prs_etag: None,
            actions_etag: None,
            releases_etag: None,
            packages_container_etag: None,
            packages_npm_etag: None,
        };

        save_cache(&dc, Some("alice")).unwrap();

        // Check raw database contents
        {
            let conn = open_conn().unwrap();
            let repo_id: i64 = conn
                .query_row("SELECT id FROM repos WHERE path = ?", [&path_str], |r| {
                    r.get(0)
                })
                .unwrap();

            // prs endpoint should have "Rate Limit Exceeded"
            let prs_err: Option<String> = conn.query_row(
                "SELECT error FROM github_cache WHERE repo_id = ? AND auth_login = 'alice' AND endpoint = 'prs'",
                [repo_id],
                |r| r.get(0)
            ).unwrap();
            assert_eq!(prs_err, Some("Rate Limit Exceeded".to_string()));

            // actions endpoint should have NULL/None error
            let actions_err: Option<String> = conn.query_row(
                "SELECT error FROM github_cache WHERE repo_id = ? AND auth_login = 'alice' AND endpoint = 'actions'",
                [repo_id],
                |r| r.get(0)
            ).unwrap();
            assert_eq!(actions_err, None);
        }

        // Hydrate from cache and verify
        let loaded = load_cache(&path_str, Some("alice")).unwrap();
        assert_eq!(
            loaded.remote_snapshot.unwrap().github_error.as_deref(),
            Some("PRs: Rate Limit Exceeded")
        );

        // 2. Save aggregate errors
        dc.remote_snapshot = Some(RepoRemoteSnapshot {
            pull_requests: vec![],
            action_runs: vec![],
            releases: vec![],
            packages: vec![],
            github_error: Some("PRs: err1, Actions: err2".to_string()),
            last_refresh: Some(3000),
            ownership: RepoOwnership::Owned,
        });

        save_cache(&dc, Some("alice")).unwrap();

        // Check raw database contents
        {
            let conn = open_conn().unwrap();
            let repo_id: i64 = conn
                .query_row("SELECT id FROM repos WHERE path = ?", [&path_str], |r| {
                    r.get(0)
                })
                .unwrap();

            let prs_err: Option<String> = conn.query_row(
                "SELECT error FROM github_cache WHERE repo_id = ? AND auth_login = 'alice' AND endpoint = 'prs'",
                [repo_id],
                |r| r.get(0)
            ).unwrap();
            assert_eq!(prs_err, Some("err1".to_string()));

            let actions_err: Option<String> = conn.query_row(
                "SELECT error FROM github_cache WHERE repo_id = ? AND auth_login = 'alice' AND endpoint = 'actions'",
                [repo_id],
                |r| r.get(0)
            ).unwrap();
            assert_eq!(actions_err, Some("err2".to_string()));
        }

        // Hydrate and verify order/content
        let loaded = load_cache(&path_str, Some("alice")).unwrap();
        let final_err = loaded.remote_snapshot.unwrap().github_error.unwrap();
        assert_eq!(final_err, "PRs: err1, Actions: err2");

        fs::remove_dir_all(&temp_dir).unwrap();
    }
}
