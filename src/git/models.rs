use std::time::SystemTime;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum FileChangeKind {
    Added,
    Modified,
    Deleted,
    Renamed,
    TypeChanged,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct FileStatus {
    pub path: String,
    pub old_path: Option<String>,
    pub kind: FileChangeKind,
    pub staged: bool,
    pub additions: usize,
    pub deletions: usize,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Commit {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub email: String,
    pub timestamp: SystemTime,
    pub parents: Vec<String>,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Branch {
    pub name: String,
    pub is_current: bool,
    pub is_remote: bool,
    pub upstream: Option<String>,
    pub tip_hash: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Tag {
    pub name: String,
    pub target_hash: String,
    pub author: String,
    pub timestamp: SystemTime,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct Stash {
    pub message: String,
    pub hash: String,
    pub timestamp: SystemTime,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct RepoStatus {
    pub branch: String,
    pub staged_count: usize,
    pub unstaged_count: usize,
    pub staged_files: Vec<FileStatus>,
    pub unstaged_files: Vec<FileStatus>,
    pub additions: usize,
    pub deletions: usize,
    pub files_changed: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_clone() {
        let commit = Commit {
            hash: "abc123".to_string(),
            short_hash: "abc123".to_string(),
            message: "fix: something".to_string(),
            author: "Test User".to_string(),
            email: "test@example.com".to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
            parents: vec!["def456".to_string()],
        };
        let cloned = commit.clone();
        assert_eq!(cloned.message, commit.message);
        assert_eq!(cloned.parents.len(), 1);
    }

    #[test]
    fn test_branch_current_flag() {
        let branch = Branch {
            name: "main".to_string(),
            is_current: true,
            is_remote: false,
            upstream: None,
            tip_hash: "abc".to_string(),
        };
        assert!(branch.is_current);
        assert!(!branch.is_remote);
    }

    #[test]
    fn test_remote_basic() {
        let remote = Remote {
            name: "origin".to_string(),
            url: "https://github.com/user/repo.git".to_string(),
        };
        assert_eq!(remote.name, "origin");
    }

    #[test]
    fn test_tag_basic() {
        let tag = Tag {
            name: "v1.0.0".to_string(),
            target_hash: "abc123".to_string(),
            author: "Test Author".to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
        };
        assert_eq!(tag.name, "v1.0.0");
    }

    #[test]
    fn test_stash_basic() {
        let stash = Stash {
            message: "WIP on main".to_string(),
            hash: "abc".to_string(),
            timestamp: SystemTime::UNIX_EPOCH,
        };
        assert!(stash.message.starts_with("WIP"));
    }

    #[test]
    fn test_repo_status_empty() {
        let status = RepoStatus {
            branch: "main".to_string(),
            staged_count: 0,
            unstaged_count: 0,
            staged_files: vec![],
            unstaged_files: vec![],
            additions: 0,
            deletions: 0,
            files_changed: 0,
        };
        assert_eq!(status.branch, "main");
        assert!(status.staged_files.is_empty());
    }

    #[test]
    fn test_repo_status_with_changes() {
        let status = RepoStatus {
            branch: "feature".to_string(),
            staged_count: 2,
            unstaged_count: 1,
            staged_files: vec![
                FileStatus {
                    path: "src/main.rs".to_string(),
                    old_path: None,
                    kind: FileChangeKind::Modified,
                    staged: true,
                    additions: 30,
                    deletions: 5,
                },
                FileStatus {
                    path: "Cargo.toml".to_string(),
                    old_path: None,
                    kind: FileChangeKind::Added,
                    staged: true,
                    additions: 12,
                    deletions: 0,
                },
            ],
            unstaged_files: vec![FileStatus {
                path: "README.md".to_string(),
                old_path: None,
                kind: FileChangeKind::Modified,
                staged: false,
                additions: 0,
                deletions: 0,
            }],
            additions: 42,
            deletions: 5,
            files_changed: 3,
        };
        assert_eq!(status.staged_files.len(), 2);
        assert_eq!(status.additions, 42);
    }
}
