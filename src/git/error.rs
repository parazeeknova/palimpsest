use std::fmt;

#[derive(Debug)]
pub enum GitError {
    NotAGitRepo,
    NotFound(String),
    Corrupt(String),
    Io(std::io::Error),
    Git(String),
}

impl fmt::Display for GitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GitError::NotAGitRepo => write!(f, "Not a git repository"),
            GitError::NotFound(msg) => write!(f, "Not found: {}", msg),
            GitError::Corrupt(msg) => write!(f, "Repository corrupt: {}", msg),
            GitError::Io(e) => write!(f, "IO error: {}", e),
            GitError::Git(msg) => write!(f, "Git error: {}", msg),
        }
    }
}

impl From<git2::Error> for GitError {
    fn from(e: git2::Error) -> Self {
        match e.code() {
            git2::ErrorCode::NotFound => GitError::NotFound(e.message().to_string()),
            git2::ErrorCode::GenericError => {
                let msg = e.message();
                if msg.contains("not a git repository") {
                    GitError::NotAGitRepo
                } else if msg.contains("corrupt") || msg.contains("malformed") {
                    GitError::Corrupt(msg.to_string())
                } else {
                    GitError::Git(msg.to_string())
                }
            }
            _ => GitError::Git(e.message().to_string()),
        }
    }
}

impl From<std::io::Error> for GitError {
    fn from(e: std::io::Error) -> Self {
        GitError::Io(e)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_not_a_git_repo() {
        let err = GitError::NotAGitRepo;
        assert_eq!(format!("{}", err), "Not a git repository");
    }

    #[test]
    fn test_display_not_found() {
        let err = GitError::NotFound("refs/heads/main".to_string());
        assert_eq!(format!("{}", err), "Not found: refs/heads/main");
    }

    #[test]
    fn test_display_corrupt() {
        let err = GitError::Corrupt("bad object".to_string());
        assert_eq!(format!("{}", err), "Repository corrupt: bad object");
    }

    #[test]
    fn test_display_git_error() {
        let err = GitError::Git("failed to resolve reference".to_string());
        assert_eq!(format!("{}", err), "Git error: failed to resolve reference");
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let git_err = GitError::from(io_err);
        match git_err {
            GitError::Io(e) => assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied),
            _ => panic!("Expected Io variant"),
        }
    }
}
