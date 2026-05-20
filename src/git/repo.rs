use std::collections::HashSet;
use std::time::SystemTime;

use git2::{Repository, Sort, StatusOptions};

use crate::git::error::GitError;
use crate::git::models::{
    Branch, Commit, FileChangeKind, FileStatus, Remote, RepoStatus, Stash, Tag,
};

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
            Ok(name)
        } else {
            tracing::debug!("HEAD is detached");
            Ok("HEAD (detached)".to_string())
        }
    }

    pub fn commits(&self, limit: usize) -> Result<Vec<Commit>, GitError> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TOPOLOGICAL)?;
        revwalk.push_head()?;

        let commits: Vec<Commit> = revwalk
            .take(limit)
            .map(|oid_result| {
                let oid = oid_result?;
                let commit = self.repo.find_commit(oid)?;
                Ok(self.commit_from_git2(&commit))
            })
            .collect::<Result<Vec<_>, git2::Error>>()?;

        tracing::info!(count = commits.len(), "Commits fetched");
        Ok(commits)
    }

    pub fn branches(&self) -> Result<Vec<Branch>, GitError> {
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
        let remotes = self.repo.remotes()?;
        let result: Vec<Remote> = remotes
            .iter()
            .flatten()
            .map(|name| {
                let remote = self.repo.find_remote(name)?;
                let url = remote.url().unwrap_or("").to_string();
                Ok(Remote {
                    name: name.to_string(),
                    url,
                })
            })
            .collect::<Result<Vec<_>, git2::Error>>()?;

        tracing::info!(count = result.len(), "Remotes fetched");
        Ok(result)
    }

    pub fn tags(&self) -> Result<Vec<Tag>, GitError> {
        let tags = self.repo.tag_names(None)?;
        let result: Vec<Tag> = tags
            .iter()
            .flatten()
            .map(|name| {
                let oid = self.repo.revparse_single(&format!("refs/tags/{}", name))?;
                let target = oid.peel_to_commit()?;
                Ok(Tag {
                    name: name.to_string(),
                    target_hash: target.id().to_string()[..7].to_string(),
                })
            })
            .collect::<Result<Vec<_>, git2::Error>>()?;

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
            .map(|oid| {
                let commit = self.repo.find_commit(*oid)?;
                let timestamp = secs_to_system_time(commit.time().seconds());
                let message = commit.message().unwrap_or("WIP on stash").to_string();
                Ok(Stash {
                    message,
                    hash: oid.to_string()[..7].to_string(),
                    timestamp,
                })
            })
            .collect::<Result<Vec<_>, git2::Error>>()?;

        tracing::info!(count = stashes.len(), "Stashes fetched");
        Ok(stashes)
    }

    pub fn status(&self) -> Result<RepoStatus, GitError> {
        let branch = self.head_branch().unwrap_or_else(|_| "HEAD".to_string());

        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        opts.renames_head_to_index(true);
        opts.renames_index_to_workdir(true);

        let statuses = self.repo.statuses(Some(&mut opts))?;

        let mut staged_paths = HashSet::new();
        let mut unstaged_paths = HashSet::new();
        let mut file_entries: Vec<(String, git2::Status, Option<String>)> = Vec::new();

        for entry in statuses.iter() {
            let status = entry.status();
            let path = entry.path().unwrap_or("unknown").to_string();
            let head_path = entry
                .head_to_index()
                .and_then(|d| d.new_file().path())
                .or_else(|| entry.index_to_workdir().and_then(|d| d.new_file().path()))
                .and_then(|p| p.to_str())
                .map(|s| s.to_string());

            if status.is_wt_new()
                || status.is_wt_modified()
                || status.is_wt_deleted()
                || status.is_wt_typechange()
                || status.is_wt_renamed()
            {
                unstaged_paths.insert(path.clone());
            }

            if status.is_index_new()
                || status.is_index_modified()
                || status.is_index_deleted()
                || status.is_index_typechange()
                || status.is_index_renamed()
            {
                staged_paths.insert(path.clone());
            }

            file_entries.push((path, status, head_path));
        }

        let head_tree = self.repo.head().ok().and_then(|h| h.peel_to_tree().ok());

        let mut index = self.repo.index()?;
        let index_tree = index
            .write_tree()
            .ok()
            .and_then(|oid| self.repo.find_tree(oid).ok());

        let mut staged_file_stats: std::collections::HashMap<String, (usize, usize)> =
            std::collections::HashMap::new();
        if let (Some(head), Some(index_t)) = (&head_tree, &index_tree) {
            let mut diff_opts = git2::DiffOptions::new();
            if let Ok(diff) =
                self.repo
                    .diff_tree_to_tree(Some(head), Some(index_t), Some(&mut diff_opts))
            {
                let mut current_path = String::new();
                let mut current_adds = 0usize;
                let mut current_dels = 0usize;
                diff.print(git2::DiffFormat::Patch, |delta, _hunk, diff_line| {
                    let path = delta
                        .new_file()
                        .path()
                        .or_else(|| delta.old_file().path())
                        .and_then(|p| p.to_str())
                        .map(|s| s.to_string());
                    if let Some(p) = path {
                        if p != current_path && !current_path.is_empty() {
                            *staged_file_stats
                                .entry(current_path.clone())
                                .or_insert((0, 0)) = (current_adds, current_dels);
                        }
                        if p != current_path {
                            current_path = p;
                            current_adds = 0;
                            current_dels = 0;
                        }
                    }
                    match diff_line.origin() {
                        '+' => current_adds += 1,
                        '-' => current_dels += 1,
                        _ => {}
                    }
                    true
                })
                .ok();
                if !current_path.is_empty() {
                    *staged_file_stats.entry(current_path).or_insert((0, 0)) =
                        (current_adds, current_dels);
                }
            }
        }

        let mut unstaged_file_stats: std::collections::HashMap<String, (usize, usize)> =
            std::collections::HashMap::new();
        if let Some(index_t) = &index_tree {
            if let Ok(diff) = self
                .repo
                .diff_tree_to_workdir_with_index(Some(index_t), None)
            {
                let mut current_path = String::new();
                let mut current_adds = 0usize;
                let mut current_dels = 0usize;
                diff.print(git2::DiffFormat::Patch, |delta, _hunk, diff_line| {
                    let path = delta
                        .new_file()
                        .path()
                        .or_else(|| delta.old_file().path())
                        .and_then(|p| p.to_str())
                        .map(|s| s.to_string());
                    if let Some(p) = path {
                        if p != current_path && !current_path.is_empty() {
                            *unstaged_file_stats
                                .entry(current_path.clone())
                                .or_insert((0, 0)) = (current_adds, current_dels);
                        }
                        if p != current_path {
                            current_path = p;
                            current_adds = 0;
                            current_dels = 0;
                        }
                    }
                    match diff_line.origin() {
                        '+' => current_adds += 1,
                        '-' => current_dels += 1,
                        _ => {}
                    }
                    true
                })
                .ok();
                if !current_path.is_empty() {
                    *unstaged_file_stats.entry(current_path).or_insert((0, 0)) =
                        (current_adds, current_dels);
                }
            }
        }

        let mut staged_files: Vec<FileStatus> = Vec::new();
        let mut unstaged_files: Vec<FileStatus> = Vec::new();
        for (path, status, old_path) in &file_entries {
            let is_staged = status.is_index_new()
                || status.is_index_modified()
                || status.is_index_deleted()
                || status.is_index_typechange()
                || status.is_index_renamed();

            let is_unstaged = status.is_wt_new()
                || status.is_wt_modified()
                || status.is_wt_deleted()
                || status.is_wt_typechange()
                || status.is_wt_renamed();

            if is_staged {
                let kind = if status.is_index_new() {
                    FileChangeKind::Added
                } else if status.is_index_modified() {
                    FileChangeKind::Modified
                } else if status.is_index_deleted() {
                    FileChangeKind::Deleted
                } else if status.is_index_renamed() {
                    FileChangeKind::Renamed
                } else {
                    FileChangeKind::TypeChanged
                };

                let (additions, deletions) = staged_file_stats.get(path).copied().unwrap_or((0, 0));

                staged_files.push(FileStatus {
                    path: path.clone(),
                    old_path: old_path.clone(),
                    kind,
                    staged: true,
                    additions,
                    deletions,
                });
            }

            if is_unstaged {
                let kind = if status.is_wt_new() {
                    FileChangeKind::Added
                } else if status.is_wt_modified() {
                    FileChangeKind::Modified
                } else if status.is_wt_deleted() {
                    FileChangeKind::Deleted
                } else if status.is_wt_renamed() {
                    FileChangeKind::Renamed
                } else {
                    FileChangeKind::TypeChanged
                };

                let (additions, deletions) =
                    unstaged_file_stats.get(path).copied().unwrap_or((0, 0));

                unstaged_files.push(FileStatus {
                    path: path.clone(),
                    old_path: old_path.clone(),
                    kind,
                    staged: false,
                    additions,
                    deletions,
                });
            }
        }

        let staged_count = staged_paths.len();
        let unstaged_count = unstaged_paths.len();

        let (additions, deletions) = self
            .repo
            .diff_index_to_workdir(None, None)
            .ok()
            .and_then(|diff| diff.stats().ok())
            .map(|s| (s.insertions(), s.deletions()))
            .unwrap_or((0, 0));

        let files_changed = staged_paths.union(&unstaged_paths).count();

        let result = RepoStatus {
            branch,
            staged_count,
            unstaged_count,
            staged_files,
            unstaged_files,
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

    pub fn stage_file(&self, path: &str) -> Result<(), GitError> {
        let mut index = self.repo.index()?;
        index.add_path(std::path::Path::new(path))?;
        index.write()?;
        Ok(())
    }

    pub fn unstage_file(&self, path: &str) -> Result<(), GitError> {
        let mut index = self.repo.index()?;
        index.remove_path(std::path::Path::new(path))?;
        index.write()?;
        Ok(())
    }

    pub fn discard_file(&self, path: &str) -> Result<(), GitError> {
        let head = self.repo.head()?;
        let tree = head.peel_to_tree()?;
        let mut opts = git2::build::CheckoutBuilder::new();
        opts.path(path);
        opts.force();
        self.repo.checkout_tree(tree.as_object(), Some(&mut opts))?;
        Ok(())
    }

    pub fn stage_all(&self) -> Result<(), GitError> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        let statuses = self.repo.statuses(Some(&mut opts))?;

        let mut index = self.repo.index()?;
        for entry in statuses.iter() {
            let status = entry.status();
            if status.is_wt_new()
                || status.is_wt_modified()
                || status.is_wt_deleted()
                || status.is_wt_typechange()
                || status.is_wt_renamed()
            {
                if let Some(path) = entry.path() {
                    index.add_path(std::path::Path::new(path))?;
                }
            }
        }
        index.write()?;
        Ok(())
    }

    pub fn discard_all(&self) -> Result<(), GitError> {
        let mut opts = StatusOptions::new();
        opts.include_untracked(true);
        let statuses = self.repo.statuses(Some(&mut opts))?;

        for entry in statuses.iter() {
            let status = entry.status();
            if status.is_wt_new() {
                if let Some(path) = entry.path() {
                    let full_path = self.repo.workdir().map(|w| w.join(path));
                    if let Some(full_path) = full_path {
                        if full_path.is_file() {
                            std::fs::remove_file(&full_path).ok();
                        } else if full_path.is_dir() {
                            std::fs::remove_dir_all(&full_path).ok();
                        }
                    }
                }
            } else if status.is_wt_modified()
                || status.is_wt_deleted()
                || status.is_wt_typechange()
                || status.is_wt_renamed()
            {
                if let Some(path) = entry.path() {
                    let head = self.repo.head()?;
                    let tree = head.peel_to_tree()?;
                    let mut checkout_opts = git2::build::CheckoutBuilder::new();
                    checkout_opts.path(path);
                    checkout_opts.force();
                    self.repo
                        .checkout_tree(tree.as_object(), Some(&mut checkout_opts))
                        .ok();
                }
            }
        }
        Ok(())
    }
}

fn secs_to_system_time(secs: i64) -> SystemTime {
    if secs >= 0 {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
    } else {
        SystemTime::UNIX_EPOCH
    }
}
