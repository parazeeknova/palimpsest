use std::time::SystemTime;

use git2::{Repository, Sort, StatusOptions};

use crate::git::error::GitError;
use crate::git::models::{Branch, Commit, Remote, RepoStatus, Stash, Tag};

pub struct GitRepo {
    repo: Repository,
}

impl GitRepo {
    pub fn open(path: &str) -> Result<Self, GitError> {
        tracing::debug!(path = %path, "Attempting to open git repository");
        let repo = Repository::open(path)?;
        tracing::info!(path = %path, "Git repository opened successfully");
        Ok(Self { repo })
    }

    pub fn repo_name(&self) -> Option<String> {
        self.repo
            .workdir()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    }

    pub fn head_branch(&self) -> Result<String, GitError> {
        let head = self.repo.head()?;
        if head.is_branch() {
            let name = head.shorthand().unwrap_or("HEAD").to_string();
            tracing::debug!(branch = %name, "Current head branch");
            Ok(name)
        } else {
            tracing::debug!("HEAD is detached");
            Ok("HEAD (detached)".to_string())
        }
    }

    pub fn commits(&self, limit: usize) -> Result<Vec<Commit>, GitError> {
        tracing::debug!(limit, "Fetching commits");
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TOPOLOGICAL)?;
        revwalk.push_head()?;

        let commits: Vec<Commit> = revwalk
            .take(limit)
            .filter_map(|oid_result| {
                let oid = oid_result.ok()?;
                let commit = self.repo.find_commit(oid).ok()?;
                Some(self.commit_from_git2(&commit))
            })
            .collect();

        tracing::info!(count = commits.len(), "Commits fetched");
        Ok(commits)
    }

    pub fn branches(&self) -> Result<Vec<Branch>, GitError> {
        tracing::debug!("Fetching branches");
        let head_name = self.head_branch().ok();

        let mut branches = Vec::new();

        for branch_result in self.repo.branches(Some(git2::BranchType::Local))? {
            let (branch, _) = branch_result?;
            let name = branch.name()?.unwrap_or("unknown").to_string();
            let tip = branch.get().peel_to_commit()?;
            let is_current = head_name.as_deref() == Some(&name);

            let upstream = branch
                .upstream()
                .ok()
                .map(|b| b.name().ok().flatten().unwrap_or("unknown").to_string());

            branches.push(Branch {
                name,
                is_current,
                is_remote: false,
                upstream,
                tip_hash: tip.id().to_string()[..7].to_string(),
            });
        }

        for branch_result in self.repo.branches(Some(git2::BranchType::Remote))? {
            let (branch, _) = branch_result?;
            let name = branch.name()?.unwrap_or("unknown").to_string();
            let tip = branch.get().peel_to_commit()?;

            branches.push(Branch {
                name,
                is_current: false,
                is_remote: true,
                upstream: None,
                tip_hash: tip.id().to_string()[..7].to_string(),
            });
        }

        tracing::info!(count = branches.len(), "Branches fetched");
        Ok(branches)
    }

    pub fn remotes(&self) -> Result<Vec<Remote>, GitError> {
        tracing::debug!("Fetching remotes");
        let remotes = self.repo.remotes()?;
        let result: Vec<Remote> = remotes
            .iter()
            .filter_map(|name| {
                let name = name?;
                let remote = self.repo.find_remote(name).ok()?;
                let url = remote.url().unwrap_or("").to_string();
                Some(Remote {
                    name: name.to_string(),
                    url,
                })
            })
            .collect();

        tracing::info!(count = result.len(), "Remotes fetched");
        Ok(result)
    }

    pub fn tags(&self) -> Result<Vec<Tag>, GitError> {
        tracing::debug!("Fetching tags");
        let tags = self.repo.tag_names(None)?;
        let result: Vec<Tag> = tags
            .iter()
            .filter_map(|name| {
                let name = name?;
                let oid = self
                    .repo
                    .revparse_single(&format!("refs/tags/{}", name))
                    .ok()?;
                let target = oid.peel_to_commit().ok()?;
                Some(Tag {
                    name: name.to_string(),
                    target_hash: target.id().to_string()[..7].to_string(),
                })
            })
            .collect();

        tracing::info!(count = result.len(), "Tags fetched");
        Ok(result)
    }

    #[allow(dead_code)]
    pub fn stashes(&self) -> Result<Vec<Stash>, GitError> {
        tracing::debug!("Fetching stashes");
        let mut stash_oids = Vec::new();

        let mut repo = Repository::open(self.repo.path())?;
        repo.stash_foreach(|_index, _name, oid| {
            stash_oids.push(*oid);
            true
        })?;

        let stashes: Vec<Stash> = stash_oids
            .iter()
            .filter_map(|oid| {
                let commit = self.repo.find_commit(*oid).ok()?;
                let timestamp = secs_to_system_time(commit.time().seconds());

                let message = commit.message().unwrap_or("WIP on stash").to_string();

                Some(Stash {
                    message,
                    hash: oid.to_string()[..7].to_string(),
                    timestamp,
                })
            })
            .collect();

        tracing::info!(count = stashes.len(), "Stashes fetched");
        Ok(stashes)
    }

    pub fn status(&self) -> Result<RepoStatus, GitError> {
        tracing::debug!("Fetching repository status");
        let branch = self.head_branch().unwrap_or_else(|_| "HEAD".to_string());

        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.renames_head_to_index(true);
        opts.renames_index_to_workdir(true);

        let statuses = self.repo.statuses(Some(&mut opts))?;

        let mut staged_count = 0;
        let mut unstaged_count = 0;
        let mut staged_files = Vec::new();
        let mut additions = 0;
        let mut deletions = 0;
        let mut files_changed = 0;

        for entry in statuses.iter() {
            let status = entry.status();
            let path = entry.path().unwrap_or("unknown").to_string();
            files_changed += 1;

            if status.is_wt_new()
                || status.is_wt_modified()
                || status.is_wt_deleted()
                || status.is_wt_typechange()
                || status.is_wt_renamed()
            {
                unstaged_count += 1;
            }

            if status.is_index_new()
                || status.is_index_modified()
                || status.is_index_deleted()
                || status.is_index_typechange()
                || status.is_index_renamed()
            {
                staged_count += 1;
                staged_files.push(path.clone());
            }

            if let Ok(diff) = self.repo.diff_index_to_workdir(None, None) {
                let stats = diff.stats().ok();
                if let Some(s) = stats {
                    additions = s.insertions();
                    deletions = s.deletions();
                }
            }
        }

        let result = RepoStatus {
            branch,
            staged_count,
            unstaged_count,
            staged_files,
            additions,
            deletions,
            files_changed,
        };

        tracing::info!(
            staged = result.staged_count,
            unstaged = result.unstaged_count,
            "Repository status fetched"
        );
        Ok(result)
    }

    fn commit_from_git2(&self, commit: &git2::Commit) -> Commit {
        let author = commit.author();
        let parents: Vec<String> = commit
            .parent_ids()
            .map(|oid| oid.to_string()[..7].to_string())
            .collect();

        Commit {
            hash: commit.id().to_string(),
            short_hash: commit.id().to_string()[..7].to_string(),
            message: commit.message().unwrap_or("").trim().to_string(),
            author: author.name().unwrap_or("Unknown").to_string(),
            email: author.email().unwrap_or("").to_string(),
            timestamp: secs_to_system_time(commit.time().seconds()),
            parents,
        }
    }
}

fn secs_to_system_time(secs: i64) -> SystemTime {
    if secs >= 0 {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
    } else {
        SystemTime::UNIX_EPOCH
    }
}
