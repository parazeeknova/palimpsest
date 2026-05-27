use std::collections::HashSet;
use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

use git2::{
    Cred, FetchOptions, PushOptions, RemoteCallbacks, Repository, Sort, StashFlags, StatusOptions,
};

use crate::cdv::{
    CommitDiffFile, CommitDiffFileKind, CommitDiffHunk, CommitDiffLine, CommitDiffLineKind,
    CommitDiffSummary, CommitDiffViewModel,
};
use crate::git::error::GitError;
use crate::git::models::{
    Branch, Commit, CommitSignatureInfo, FileChangeKind, FileStatus, Remote, RepoStatus, Stash, Tag,
};

pub struct GitRepo {
    repo: Repository,
}

impl GitRepo {
    pub fn open(path: &str) -> Result<Self, GitError> {
        tracing::debug!(path = %path, "Attempting to open git repository");
        let repo = Repository::open(path)?;
        tracing::debug!(path = %path, "Git repository opened successfully");
        Ok(Self { repo })
    }

    pub fn repo_name(&self) -> Option<String> {
        self.repo
            .workdir()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
    }

    pub fn workdir_path(&self) -> Option<PathBuf> {
        self.repo.workdir().map(|p| p.to_path_buf())
    }

    pub fn git_dir_path(&self) -> PathBuf {
        self.repo.path().to_path_buf()
    }

    pub fn head_branch(&self) -> Result<String, GitError> {
        match self.repo.head() {
            Ok(head) => {
                if head.is_branch() {
                    let name = head.shorthand().unwrap_or("HEAD").to_string();
                    Ok(name)
                } else {
                    tracing::debug!("HEAD is detached");
                    Ok("HEAD (detached)".to_string())
                }
            }
            Err(e)
                if e.code() == git2::ErrorCode::UnbornBranch
                    || e.code() == git2::ErrorCode::NotFound =>
            {
                if let Ok(head_ref) = self.repo.find_reference("HEAD") {
                    if let Ok(Some(target)) = head_ref.symbolic_target() {
                        if let Some(stripped) = target.strip_prefix("refs/heads/") {
                            return Ok(stripped.to_string());
                        }
                    }
                }
                Ok("master".to_string())
            }
            Err(e) => Err(GitError::from(e)),
        }
    }

    pub fn commits(&self, limit: Option<usize>) -> Result<Vec<Commit>, GitError> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TOPOLOGICAL | Sort::TIME)?;

        let pushed_head = revwalk.push_head();
        let _ = revwalk.push_glob("refs/heads/*");
        let _ = revwalk.push_glob("refs/remotes/*");

        if let Err(e) = pushed_head {
            if self.repo.references()?.count() == 0 {
                return Err(GitError::from(e));
            }
        }

        let mut commits = Vec::new();
        for oid_result in revwalk {
            if let Some(l) = limit {
                if commits.len() >= l {
                    break;
                }
            }
            let oid = oid_result?;
            let commit = self.repo.find_commit(oid)?;
            commits.push(self.commit_from_git2(&commit));
        }

        tracing::debug!(count = commits.len(), "Commits fetched");
        Ok(commits)
    }

    pub fn history_stats(&self) -> Result<(usize, Option<Commit>), GitError> {
        let mut revwalk = self.repo.revwalk()?;
        revwalk.set_sorting(Sort::TOPOLOGICAL)?;
        if revwalk.push_head().is_err() {
            return Ok((0, None));
        }

        let mut count = 0;
        let mut last_oid = None;
        for oid_result in revwalk {
            let oid = oid_result?;
            last_oid = Some(oid);
            count += 1;
        }

        let oldest_commit = if let Some(oid) = last_oid {
            let commit = self.repo.find_commit(oid)?;
            Some(self.commit_from_git2(&commit))
        } else {
            None
        };

        Ok((count, oldest_commit))
    }

    pub fn commit_by_hash(&self, hash: &str) -> Result<Commit, GitError> {
        let oid = self.repo.revparse_single(hash)?.id();
        let commit = self.repo.find_commit(oid)?;
        Ok(self.commit_from_git2(&commit))
    }

    pub fn commit_signature_info(
        &self,
        hash: &str,
    ) -> Result<Option<CommitSignatureInfo>, GitError> {
        let output = match Command::new("git")
            .args(["verify-commit", "--raw", hash])
            .current_dir(self.repo.workdir().unwrap_or(self.repo.path()))
            .output()
        {
            Ok(out) => out,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(None);
                }
                return Err(GitError::from(e));
            }
        };

        let raw_output = format!(
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let parsed = parse_commit_signature_output(&raw_output);
        if let Some(sig) = parsed.as_ref() {
            tracing::debug!(
                hash = %hash,
                status = %sig.status,
                key_id = sig.key_id.as_deref().unwrap_or(""),
                trust = sig.trust.as_deref().unwrap_or(""),
                "Parsed commit signature metadata"
            );
        } else {
            tracing::debug!(hash = %hash, "No commit signature metadata parsed");
        }
        Ok(parsed)
    }

    pub fn commit_files(&self, hash: &str) -> Result<Vec<FileStatus>, GitError> {
        let commit = self.repo.revparse_single(hash)?.peel_to_commit()?;
        let tree = commit.tree()?;
        let parent_tree = commit
            .parents()
            .next()
            .and_then(|parent| parent.tree().ok());

        let mut diff_opts = git2::DiffOptions::new();
        let diff = match parent_tree {
            Some(parent_tree) => self.repo.diff_tree_to_tree(
                Some(&parent_tree),
                Some(&tree),
                Some(&mut diff_opts),
            )?,
            None => self
                .repo
                .diff_tree_to_tree(None, Some(&tree), Some(&mut diff_opts))?,
        };

        let mut file_stats: std::collections::HashMap<String, (usize, usize)> =
            std::collections::HashMap::new();
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
                        *file_stats.entry(current_path.clone()).or_insert((0, 0)) =
                            (current_adds, current_dels);
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
            })?;
            if !current_path.is_empty() {
                *file_stats.entry(current_path).or_insert((0, 0)) = (current_adds, current_dels);
            }
        }

        let mut files = Vec::new();
        diff.print(git2::DiffFormat::NameStatus, |delta, _hunk, _line| {
            let path = delta
                .new_file()
                .path()
                .or_else(|| delta.old_file().path())
                .and_then(|p| p.to_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let stats = file_stats.get(&path).copied().unwrap_or((0, 0));
            let old_path = delta
                .old_file()
                .path()
                .and_then(|p| p.to_str())
                .map(|s| s.to_string());

            let kind = match delta.status() {
                git2::Delta::Added => FileChangeKind::Added,
                git2::Delta::Deleted => FileChangeKind::Deleted,
                git2::Delta::Renamed => FileChangeKind::Renamed,
                git2::Delta::Typechange => FileChangeKind::TypeChanged,
                _ => FileChangeKind::Modified,
            };

            files.push(FileStatus {
                path,
                old_path,
                kind,
                staged: true,
                additions: stats.0,
                deletions: stats.1,
            });
            true
        })?;

        Ok(files)
    }

    pub fn repo_files(&self) -> Result<Vec<FileStatus>, GitError> {
        let head = match self.repo.head() {
            Ok(h) => h,
            Err(_) => return Ok(Vec::new()),
        };
        let commit = match head.peel_to_commit() {
            Ok(c) => c,
            Err(_) => return Ok(Vec::new()),
        };
        let tree = commit.tree()?;
        let mut files = Vec::new();
        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            if entry.kind() == Some(git2::ObjectType::Blob) {
                let name = entry.name().unwrap_or("");
                let path = if root.is_empty() {
                    name.to_string()
                } else {
                    format!("{}{}", root, name)
                };
                files.push(FileStatus {
                    path,
                    old_path: None,
                    kind: FileChangeKind::Modified,
                    staged: false,
                    additions: 0,
                    deletions: 0,
                });
            }
            git2::TreeWalkResult::Ok
        })?;
        Ok(files)
    }

    pub fn commit_diff_view(&self, hash: &str) -> Result<CommitDiffViewModel, GitError> {
        let commit = self.repo.revparse_single(hash)?.peel_to_commit()?;
        let tree = commit.tree()?;
        let parent_tree = commit
            .parents()
            .next()
            .and_then(|parent| parent.tree().ok());

        let mut diff_opts = git2::DiffOptions::new();
        diff_opts.show_binary(true);
        diff_opts.context_lines(3);
        diff_opts.interhunk_lines(0);

        let mut diff = match parent_tree {
            Some(parent_tree) => self.repo.diff_tree_to_tree(
                Some(&parent_tree),
                Some(&tree),
                Some(&mut diff_opts),
            )?,
            None => self
                .repo
                .diff_tree_to_tree(None, Some(&tree), Some(&mut diff_opts))?,
        };

        let mut find_opts = git2::DiffFindOptions::new();
        find_opts
            .renames(true)
            .copies(true)
            .break_rewrites(true)
            .rename_limit(512);
        diff.find_similar(Some(&mut find_opts))?;

        struct DiffAccumulator {
            files: Vec<CommitDiffFile>,
            current_file_index: Option<usize>,
            current_hunk_index: Option<usize>,
            file_is_binary: bool,
            file_additions: usize,
            file_deletions: usize,
            file_lines: usize,
            file_hunks: usize,
            file_truncated: bool,
            diff_truncated: bool,
        }

        let acc = std::cell::RefCell::new(DiffAccumulator {
            files: Vec::new(),
            current_file_index: None,
            current_hunk_index: None,
            file_is_binary: false,
            file_additions: 0,
            file_deletions: 0,
            file_lines: 0,
            file_hunks: 0,
            file_truncated: false,
            diff_truncated: false,
        });

        let mut summary = CommitDiffSummary::default();
        let start = std::time::Instant::now();
        let foreach_result = diff.foreach(
            &mut |delta, _progress| {
                let mut acc = acc.borrow_mut();
                if acc.files.len() >= 250 {
                    acc.diff_truncated = true;
                    return false;
                }

                let path = delta
                    .new_file()
                    .path()
                    .or_else(|| delta.old_file().path())
                    .and_then(|p| p.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let old_path = delta
                    .old_file()
                    .path()
                    .and_then(|p| p.to_str())
                    .map(|s| s.to_string());

                let kind = match delta.status() {
                    git2::Delta::Added => CommitDiffFileKind::Added,
                    git2::Delta::Deleted => CommitDiffFileKind::Deleted,
                    git2::Delta::Renamed => CommitDiffFileKind::Renamed,
                    git2::Delta::Typechange => CommitDiffFileKind::TypeChanged,
                    git2::Delta::Copied => CommitDiffFileKind::Copied,
                    git2::Delta::Untracked => CommitDiffFileKind::Untracked,
                    _ => CommitDiffFileKind::Modified,
                };

                acc.files.push(CommitDiffFile {
                    path,
                    old_path,
                    kind,
                    staged: true,
                    additions: 0,
                    deletions: 0,
                    is_binary: false,
                    hunks: Vec::new(),
                });
                acc.current_file_index = Some(acc.files.len() - 1);
                acc.current_hunk_index = None;
                acc.file_is_binary = delta.flags().contains(git2::DiffFlags::BINARY);
                acc.file_additions = 0;
                acc.file_deletions = 0;
                acc.file_lines = 0;
                acc.file_hunks = 0;
                acc.file_truncated = false;
                true
            },
            Some(&mut |_delta, _binary| {
                let mut acc = acc.borrow_mut();
                if let Some(index) = acc.current_file_index {
                    acc.files[index].is_binary = true;
                }
                true
            }),
            Some(&mut |_delta, hunk| {
                let mut acc = acc.borrow_mut();
                if acc.current_file_index.is_none() {
                    return true;
                }
                if acc.file_hunks >= 8_000 {
                    acc.file_truncated = true;
                    acc.diff_truncated = true;
                    return false;
                }

                let header = String::from_utf8_lossy(hunk.header()).to_string();
                let file_index = acc.current_file_index.expect("file index set before hunk");
                acc.files[file_index].hunks.push(CommitDiffHunk {
                    header,
                    old_start: hunk.old_start(),
                    old_lines: hunk.old_lines(),
                    new_start: hunk.new_start(),
                    new_lines: hunk.new_lines(),
                    lines: Vec::new(),
                });
                acc.current_hunk_index = Some(acc.files[file_index].hunks.len() - 1);
                acc.file_hunks += 1;
                true
            }),
            Some(&mut |_delta, _hunk, line| {
                let mut acc = acc.borrow_mut();
                let Some(file_index) = acc.current_file_index else {
                    return true;
                };
                let Some(hunk_index) = acc.current_hunk_index else {
                    return true;
                };
                if acc.file_lines >= 20_000 {
                    acc.file_truncated = true;
                    acc.diff_truncated = true;
                    return false;
                }

                let kind = match line.origin() {
                    '+' => CommitDiffLineKind::Addition,
                    '-' => CommitDiffLineKind::Deletion,
                    ' ' => CommitDiffLineKind::Context,
                    '=' => CommitDiffLineKind::Context,
                    '>' => CommitDiffLineKind::EofAddition,
                    '<' => CommitDiffLineKind::EofDeletion,
                    'B' => CommitDiffLineKind::Binary,
                    _ => CommitDiffLineKind::Context,
                };

                let content = String::from_utf8_lossy(line.content())
                    .trim_end_matches('\n')
                    .to_string();
                let file = &mut acc.files[file_index];
                if hunk_index >= file.hunks.len() {
                    return true;
                }
                file.hunks[hunk_index].lines.push(CommitDiffLine {
                    old_lineno: line.old_lineno(),
                    new_lineno: line.new_lineno(),
                    kind,
                    content,
                });
                acc.file_lines += 1;
                match line.origin() {
                    '+' | '>' => acc.file_additions += 1,
                    '-' | '<' => acc.file_deletions += 1,
                    _ => {}
                }
                true
            }),
        );

        if let Err(err) = foreach_result {
            if !acc.borrow().diff_truncated {
                return Err(GitError::from(err));
            }
        }

        let acc = acc.into_inner();
        let mut files = acc.files;
        let diff_truncated = acc.diff_truncated;
        for file in &mut files {
            file.additions = file
                .hunks
                .iter()
                .map(|h| {
                    h.lines
                        .iter()
                        .filter(|l| {
                            matches!(
                                l.kind,
                                CommitDiffLineKind::Addition | CommitDiffLineKind::EofAddition
                            )
                        })
                        .count()
                })
                .sum();
            file.deletions = file
                .hunks
                .iter()
                .map(|h| {
                    h.lines
                        .iter()
                        .filter(|l| {
                            matches!(
                                l.kind,
                                CommitDiffLineKind::Deletion | CommitDiffLineKind::EofDeletion
                            )
                        })
                        .count()
                })
                .sum();
        }

        if !files.is_empty() {
            summary.files_changed = files.len();
            summary.additions = files.iter().map(|file| file.additions).sum();
            summary.deletions = files.iter().map(|file| file.deletions).sum();
            summary.hunks = files.iter().map(|file| file.hunks.len()).sum();
            summary.lines = files
                .iter()
                .map(|file| file.hunks.iter().map(|h| h.lines.len()).sum::<usize>())
                .sum();
            summary.truncated = diff_truncated;
        }

        if summary.truncated {
            tracing::warn!(
                hash = %hash,
                files = summary.files_changed,
                hunks = summary.hunks,
                lines = summary.lines,
                "Commit diff view truncated for large commit"
            );
        }

        tracing::debug!(
            hash = %hash,
            files = summary.files_changed,
            hunks = summary.hunks,
            lines = summary.lines,
            truncated = summary.truncated,
            elapsed_ms = start.elapsed().as_millis(),
            "Commit diff view built"
        );

        Ok(CommitDiffViewModel {
            commit_hash: hash.to_string(),
            files,
            summary,
        })
    }

    pub fn branches(&self) -> Result<Vec<Branch>, GitError> {
        let head_name = self.head_branch().ok();

        let mut branches = Vec::new();

        for branch_result in self.repo.branches(Some(git2::BranchType::Local))? {
            let (branch, _) = branch_result?;
            let name = branch.name()?.unwrap_or("unknown").to_string();
            let tip = branch.get().peel_to_commit()?;
            let is_current = head_name.as_deref() == Some(&name);

            let upstream_branch = branch.upstream().ok();
            let upstream = upstream_branch
                .as_ref()
                .map(|b| b.name().ok().flatten().unwrap_or("unknown").to_string());

            let mut ahead = None;
            let mut behind = None;
            if let Some(ref ub) = upstream_branch {
                if let (Ok(local_commit), Ok(upstream_commit)) =
                    (branch.get().peel_to_commit(), ub.get().peel_to_commit())
                {
                    if let Ok((a, b)) = self
                        .repo
                        .graph_ahead_behind(local_commit.id(), upstream_commit.id())
                    {
                        ahead = Some(a);
                        behind = Some(b);
                    }
                }
            }

            branches.push(Branch {
                name,
                is_current,
                is_remote: false,
                upstream,
                tip_hash: tip.id().to_string()[..7].to_string(),
                ahead,
                behind,
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
                ahead: None,
                behind: None,
            });
        }

        tracing::debug!(count = branches.len(), "Branches fetched");
        Ok(branches)
    }

    pub fn remotes(&self) -> Result<Vec<Remote>, GitError> {
        let remotes = self.repo.remotes()?;
        let result: Vec<Remote> = remotes
            .iter()
            .filter_map(|r| r.ok().flatten())
            .map(|name| {
                let remote = self.repo.find_remote(name)?;
                let url = remote.url().unwrap_or("").to_string();
                Ok(Remote {
                    name: name.to_string(),
                    url,
                })
            })
            .collect::<Result<Vec<_>, git2::Error>>()?;

        tracing::debug!(count = result.len(), "Remotes fetched");
        Ok(result)
    }

    pub fn tags(&self) -> Result<Vec<Tag>, GitError> {
        let tags = self.repo.tag_names(None)?;
        let mut result: Vec<Tag> = tags
            .iter()
            .filter_map(|r| r.ok().flatten())
            .map(|name| {
                let oid = self.repo.revparse_single(&format!("refs/tags/{}", name))?;
                let target = oid.peel_to_commit()?;
                let (author, timestamp) = if let Ok(tag) = self.repo.find_tag(oid.id()) {
                    if let Some(tagger) = tag.tagger() {
                        (
                            tagger.name().unwrap_or("Unknown").to_string(),
                            secs_to_system_time(tagger.when().seconds()),
                        )
                    } else {
                        let author = target.author();
                        (
                            author.name().unwrap_or("Unknown").to_string(),
                            secs_to_system_time(author.when().seconds()),
                        )
                    }
                } else {
                    let author = target.author();
                    (
                        author.name().unwrap_or("Unknown").to_string(),
                        secs_to_system_time(author.when().seconds()),
                    )
                };
                Ok(Tag {
                    name: name.to_string(),
                    target_hash: target.id().to_string()[..7].to_string(),
                    author,
                    timestamp,
                })
            })
            .collect::<Result<Vec<_>, git2::Error>>()?;

        result.sort_by(|a, b| {
            let va = parse_tag_name_version(&a.name);
            let vb = parse_tag_name_version(&b.name);
            match vb.cmp(&va) {
                std::cmp::Ordering::Equal => b.name.cmp(&a.name),
                other => other,
            }
        });

        tracing::debug!(count = result.len(), "Tags fetched");
        Ok(result)
    }

    pub fn tags_limit(&self, limit: Option<usize>) -> Result<Vec<Tag>, GitError> {
        let tags = self.repo.tag_names(None)?;
        let mut tag_names: Vec<String> = tags
            .iter()
            .filter_map(|r| r.ok().flatten())
            .map(|s| s.to_string())
            .collect();

        tag_names.sort_by(|a, b| {
            let va = parse_tag_name_version(a);
            let vb = parse_tag_name_version(b);
            match vb.cmp(&va) {
                std::cmp::Ordering::Equal => b.cmp(a),
                other => other,
            }
        });

        let names_to_peel = if let Some(l) = limit {
            if tag_names.len() > l {
                &tag_names[..l]
            } else {
                &tag_names[..]
            }
        } else {
            &tag_names[..]
        };

        let mut result = Vec::new();
        for name in names_to_peel {
            let oid = self.repo.revparse_single(&format!("refs/tags/{}", name))?;
            let target = oid.peel_to_commit()?;
            let (author, timestamp) = if let Ok(tag) = self.repo.find_tag(oid.id()) {
                if let Some(tagger) = tag.tagger() {
                    (
                        tagger.name().unwrap_or("Unknown").to_string(),
                        secs_to_system_time(tagger.when().seconds()),
                    )
                } else {
                    let author = target.author();
                    (
                        author.name().unwrap_or("Unknown").to_string(),
                        secs_to_system_time(author.when().seconds()),
                    )
                }
            } else {
                let author = target.author();
                (
                    author.name().unwrap_or("Unknown").to_string(),
                    secs_to_system_time(author.when().seconds()),
                )
            };
            result.push(Tag {
                name: name.clone(),
                target_hash: target.id().to_string()[..7].to_string(),
                author,
                timestamp,
            });
        }

        tracing::debug!(count = result.len(), "Tags fetched (limited)");
        Ok(result)
    }

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

        tracing::debug!(count = stashes.len(), "Stashes fetched");
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

        tracing::debug!(
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
        let workdir_path = self
            .repo
            .workdir()
            .map(|w| w.join(path))
            .unwrap_or_else(|| std::path::PathBuf::from(path));
        if !workdir_path.exists() {
            index.remove_path(std::path::Path::new(path))?;
        } else {
            index.add_path(std::path::Path::new(path))?;
        }
        index.write()?;
        Ok(())
    }

    pub fn unstage_file(&self, path: &str) -> Result<(), GitError> {
        let head_tree = match self.repo.head().and_then(|h| h.peel_to_tree()) {
            Ok(tree) => Some(tree),
            Err(e) if e.code() == git2::ErrorCode::UnbornBranch => None,
            Err(e) if e.code() == git2::ErrorCode::NotFound => None,
            Err(e) => return Err(GitError::from(e)),
        };

        let mut index = self.repo.index()?;
        index.remove_path(std::path::Path::new(path))?;

        if let Some(tree) = head_tree {
            match tree.get_path(std::path::Path::new(path)) {
                Ok(tree_entry) => {
                    let entry = git2::IndexEntry {
                        id: tree_entry.id(),
                        mode: tree_entry.filemode() as u32,
                        path: path.as_bytes().to_vec(),
                        ctime: git2::IndexTime::new(0, 0),
                        mtime: git2::IndexTime::new(0, 0),
                        dev: 0,
                        ino: 0,
                        uid: 0,
                        gid: 0,
                        file_size: 0,
                        flags: 0,
                        flags_extended: 0,
                    };
                    index.add(&entry)?;
                }
                Err(e) if e.code() == git2::ErrorCode::NotFound => {}
                Err(e) => return Err(GitError::from(e)),
            }
        }

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
            if let Ok(path) = entry.path() {
                if status.is_wt_deleted() {
                    index.remove_path(std::path::Path::new(path))?;
                } else if status.is_wt_new()
                    || status.is_wt_modified()
                    || status.is_wt_typechange()
                    || status.is_wt_renamed()
                {
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

        let mut errors: Vec<GitError> = Vec::new();

        for entry in statuses.iter() {
            let status = entry.status();
            if status.is_wt_new() {
                if let Ok(path) = entry.path() {
                    let full_path = self.repo.workdir().map(|w| w.join(path));
                    if let Some(full_path) = full_path {
                        let remove_result = if full_path.is_file() {
                            std::fs::remove_file(&full_path).map_err(|e| {
                                GitError::Git(format!("Failed to remove file {}: {}", path, e))
                            })
                        } else if full_path.is_dir() {
                            std::fs::remove_dir_all(&full_path).map_err(|e| {
                                GitError::Git(format!("Failed to remove dir {}: {}", path, e))
                            })
                        } else {
                            Ok(())
                        };
                        if let Err(e) = remove_result {
                            errors.push(e);
                        }
                    }
                }
            } else if status.is_wt_modified()
                || status.is_wt_deleted()
                || status.is_wt_typechange()
                || status.is_wt_renamed()
            {
                if let Ok(path) = entry.path() {
                    let head = self.repo.head()?;
                    let tree = head.peel_to_tree()?;
                    let mut checkout_opts = git2::build::CheckoutBuilder::new();
                    checkout_opts.path(path);
                    checkout_opts.force();
                    if let Err(e) = self
                        .repo
                        .checkout_tree(tree.as_object(), Some(&mut checkout_opts))
                    {
                        errors.push(GitError::Git(format!("Failed to checkout {}: {}", path, e)));
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            let messages: Vec<String> = errors.iter().map(|e| format!("{}", e)).collect();
            Err(GitError::Git(format!(
                "discard_all had {} error(s): {}",
                errors.len(),
                messages.join("; ")
            )))
        }
    }

    fn resolve_upstream_or_fallback(&self) -> Result<(String, String, bool), GitError> {
        let mut remote_name = None;
        let mut remote_branch = None;
        let mut has_upstream = false;

        if let Ok(head) = self.repo.head() {
            if head.is_branch() {
                if let Ok(local_ref_name) = head.name() {
                    if let (Ok(remote_buf), Ok(merge_buf)) = (
                        self.repo.branch_upstream_remote(local_ref_name),
                        self.repo.branch_upstream_merge(local_ref_name),
                    ) {
                        if let (Ok(r), Ok(m)) = (remote_buf.as_str(), merge_buf.as_str()) {
                            remote_name = Some(r.to_string());
                            let branch_part = if let Some(stripped) = m.strip_prefix("refs/heads/")
                            {
                                stripped.to_string()
                            } else {
                                m.to_string()
                            };
                            remote_branch = Some(branch_part);
                            has_upstream = true;
                        }
                    }
                }
            }
        }

        let remote_name = match remote_name {
            Some(r) => r,
            None => {
                let remotes = self.repo.remotes()?;
                match remotes.get(0) {
                    Ok(Some(name)) => name.to_string(),
                    Ok(None) | Err(_) => {
                        return Err(GitError::Git("No remotes configured".to_string()));
                    }
                }
            }
        };

        let remote_branch = match remote_branch {
            Some(b) => b,
            None => {
                if let Ok(head) = self.repo.head() {
                    if head.is_branch() {
                        head.shorthand().unwrap_or("main").to_string()
                    } else {
                        "main".to_string()
                    }
                } else {
                    "main".to_string()
                }
            }
        };

        Ok((remote_name, remote_branch, has_upstream))
    }

    pub fn fetch(&self) -> Result<(), GitError> {
        let (remote_name, remote_branch, has_upstream) = self.resolve_upstream_or_fallback()?;
        let mut remote = self.repo.find_remote(&remote_name)?;
        let mut fo = self.remote_fetch_options()?;
        if has_upstream {
            let refspec =
                format!("refs/heads/{remote_branch}:refs/remotes/{remote_name}/{remote_branch}");
            remote.fetch(&[&refspec], Some(&mut fo), None)?;
        } else {
            remote.fetch(&[] as &[&str], Some(&mut fo), None)?;
        }
        Ok(())
    }

    pub fn pull(&self) -> Result<(), GitError> {
        let head = self.repo.head()?;
        if !head.is_branch() {
            return Err(GitError::Git("Cannot pull from detached HEAD".to_string()));
        }
        let branch_name = head.shorthand()?;
        let (remote_name, remote_branch, _has_upstream) = self.resolve_upstream_or_fallback()?;
        let mut remote = self.repo.find_remote(&remote_name)?;
        let refspec =
            format!("refs/heads/{remote_branch}:refs/remotes/{remote_name}/{remote_branch}");
        let mut fo = self.remote_fetch_options()?;
        remote.fetch(&[&refspec], Some(&mut fo), None)?;
        let remote_ref_name = format!("refs/remotes/{remote_name}/{remote_branch}");
        let remote_ref = self.repo.find_reference(&remote_ref_name)?;
        let remote_commit = remote_ref.peel_to_commit()?;
        let refname = format!("refs/heads/{}", branch_name);
        let mut local_ref = self.repo.find_reference(&refname)?;
        let local_commit = local_ref.peel_to_commit()?;
        let ancestor = self
            .repo
            .merge_base(local_commit.id(), remote_commit.id())?;
        if ancestor == remote_commit.id() {
            return Ok(());
        }
        if ancestor != local_commit.id() {
            return Err(GitError::Git(
                "Local and remote histories have diverged. Requires explicit merge or rebase."
                    .to_string(),
            ));
        }

        let mut status_opts = StatusOptions::new();
        status_opts.include_untracked(true);
        status_opts.renames_head_to_index(true);
        status_opts.renames_index_to_workdir(true);
        if !self.repo.statuses(Some(&mut status_opts))?.is_empty() {
            return Err(GitError::Git(
                "Cannot fast-forward pull with uncommitted changes".to_string(),
            ));
        }

        let tree = self.repo.find_commit(remote_commit.id())?.tree()?;
        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.force();
        self.repo
            .checkout_tree(tree.as_object(), Some(&mut checkout_opts))?;
        local_ref.set_target(remote_commit.id(), "pull: Fast-forward")?;
        Ok(())
    }

    pub fn push(&self) -> Result<(), GitError> {
        let head = self.repo.head()?;
        if !head.is_branch() {
            return Err(GitError::Git("Cannot push from detached HEAD".to_string()));
        }
        let branch_name = head.shorthand()?;
        let (remote_name, remote_branch, _has_upstream) = self.resolve_upstream_or_fallback()?;
        let refspec = format!("refs/heads/{}:refs/heads/{}", branch_name, remote_branch);
        let mut remote = self.repo.find_remote(&remote_name)?;
        let mut po = self.remote_push_options()?;
        remote.push(&[&refspec], Some(&mut po))?;
        Ok(())
    }

    fn remote_callbacks(&self) -> Result<RemoteCallbacks<'static>, GitError> {
        let mut callbacks = RemoteCallbacks::new();

        callbacks.credentials(move |url, username_from_url, allowed_types| {
            if allowed_types.is_username() {
                return Cred::username(username_from_url.unwrap_or("git"));
            }

            if allowed_types.is_ssh_key()
                || allowed_types.is_ssh_memory()
                || allowed_types.is_ssh_custom()
            {
                let username = username_from_url.unwrap_or("git");
                if let Ok(cred) = git_credential_helper(url, Some(username)) {
                    return Ok(cred);
                }
                if let Ok(cred) = Cred::ssh_key_from_agent(username) {
                    return Ok(cred);
                }

                if let Some(home) = std::env::var_os("HOME") {
                    let ssh_dir = std::path::Path::new(&home).join(".ssh");
                    let key_candidates = ["id_ed25519", "id_rsa", "id_ecdsa", "id_dsa"];
                    for candidate in key_candidates {
                        let private_key = ssh_dir.join(candidate);
                        let public_key = ssh_dir.join(format!("{candidate}.pub"));
                        if private_key.exists() {
                            let public_key_opt =
                                public_key.exists().then_some(public_key.as_path());
                            return Cred::ssh_key(username, public_key_opt, &private_key, None);
                        }
                    }
                }
            }

            if allowed_types.is_user_pass_plaintext() {
                if let Ok(cred) = git_credential_helper(url, username_from_url) {
                    return Ok(cred);
                }
            }

            Err(git2::Error::from_str(
                "No usable Git credentials found; load your SSH key into ssh-agent or use an unencrypted key",
            ))
        });

        callbacks.transfer_progress(|stats| {
            tracing::debug!(
                received_objects = stats.received_objects(),
                total_objects = stats.total_objects(),
                "Git transfer progress"
            );
            true
        });

        Ok(callbacks)
    }

    fn remote_fetch_options(&self) -> Result<FetchOptions<'static>, GitError> {
        let mut options = FetchOptions::new();
        options.remote_callbacks(self.remote_callbacks()?);
        Ok(options)
    }

    fn remote_push_options(&self) -> Result<PushOptions<'static>, GitError> {
        let mut options = PushOptions::new();
        options.remote_callbacks(self.remote_callbacks()?);
        Ok(options)
    }

    pub fn signature(&self) -> Result<git2::Signature<'static>, GitError> {
        Ok(self.repo.signature()?)
    }

    pub fn commit(&self, message: &str, amend: bool) -> Result<(), GitError> {
        let mut index = self.repo.index()?;
        let tree_id = index.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;
        let signature = self.repo.signature()?;

        if amend {
            let head = self.repo.head()?;
            let head_commit = head.peel_to_commit()?;
            let parents: Vec<git2::Commit> = head_commit.parents().collect();
            let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

            self.repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )?;
        } else {
            let parents = match self.repo.head() {
                Ok(head) => {
                    let parent_commit = head.peel_to_commit()?;
                    vec![parent_commit]
                }
                Err(e)
                    if e.code() == git2::ErrorCode::UnbornBranch
                        || e.code() == git2::ErrorCode::NotFound =>
                {
                    Vec::new()
                }
                Err(e) => return Err(GitError::from(e)),
            };
            let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

            self.repo.commit(
                Some("HEAD"),
                &signature,
                &signature,
                message,
                &tree,
                &parent_refs,
            )?;
        }

        Ok(())
    }

    pub fn create_branch(&self, name: &str) -> Result<(), GitError> {
        let head = self.repo.head()?;
        let target = head.peel_to_commit()?;
        self.repo.branch(name, &target, false)?;
        Ok(())
    }

    pub fn tag_lightweight(&self, name: &str) -> Result<(), GitError> {
        let head = self.repo.head()?;
        let commit = head.peel_to_commit()?;
        self.repo
            .tag_lightweight(name, &commit.into_object(), false)?;
        Ok(())
    }

    pub fn checkout_branch(&self, name: &str) -> Result<(), GitError> {
        let refname = format!("refs/heads/{}", name);
        let obj = self.repo.revparse_single(&refname)?;
        let commit = obj.peel_to_commit()?;

        let mut opts = git2::build::CheckoutBuilder::new();
        opts.safe();
        self.repo
            .checkout_tree(commit.as_object(), Some(&mut opts))?;

        self.repo.set_head(&refname)?;
        Ok(())
    }

    pub fn checkout_remote_branch(
        &self,
        local_name: &str,
        remote_name: &str,
    ) -> Result<(), GitError> {
        let remote_branch = self
            .repo
            .find_branch(remote_name, git2::BranchType::Remote)?;
        let target_commit = remote_branch.get().peel_to_commit()?;

        let mut local_branch = self.repo.branch(local_name, &target_commit, false)?;
        local_branch.set_upstream(Some(remote_name))?;

        self.checkout_branch(local_name)?;
        Ok(())
    }

    pub fn delete_branch(&self, name: &str) -> Result<(), GitError> {
        let mut branch = self.repo.find_branch(name, git2::BranchType::Local)?;
        branch.delete()?;
        Ok(())
    }

    pub fn unstage_all(&self) -> Result<(), GitError> {
        let head_commit = match self.repo.head().and_then(|h| h.peel_to_commit()) {
            Ok(c) => Some(c),
            Err(e)
                if e.code() == git2::ErrorCode::UnbornBranch
                    || e.code() == git2::ErrorCode::NotFound =>
            {
                None
            }
            Err(e) => return Err(GitError::from(e)),
        };

        if let Some(commit) = head_commit {
            self.repo
                .reset(commit.as_object(), git2::ResetType::Mixed, None)?;
        } else {
            let mut index = self.repo.index()?;
            index.clear()?;
            index.write()?;
        }
        Ok(())
    }

    pub fn stash_save(&self, message: Option<&str>) -> Result<(), GitError> {
        let mut repo = Repository::open(self.repo.path())?;
        let signature = repo.signature()?;
        let msg = message.unwrap_or("WIP on stash");
        repo.stash_save(&signature, msg, Some(StashFlags::DEFAULT))?;
        Ok(())
    }

    pub fn stash_apply(&self, index: usize) -> Result<(), GitError> {
        let mut repo = Repository::open(self.repo.path())?;
        repo.stash_apply(index, None)?;
        Ok(())
    }

    pub fn stash_drop(&self, index: usize) -> Result<(), GitError> {
        let mut repo = Repository::open(self.repo.path())?;
        repo.stash_drop(index)?;
        Ok(())
    }

    pub fn stash_pop(&self, index: usize) -> Result<(), GitError> {
        let mut repo = Repository::open(self.repo.path())?;
        repo.stash_apply(index, None)?;
        repo.stash_drop(index)?;
        Ok(())
    }
}

fn parse_commit_signature_output(output: &str) -> Option<CommitSignatureInfo> {
    let mut status = None;
    let mut summary = None;
    let mut key_id = None;
    let mut trust = None;

    for line in output.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("[GNUPG:]") {
            let tokens: Vec<&str> = value.split_whitespace().collect();
            match tokens.as_slice() {
                ["GOODSIG", key, _user @ ..] => {
                    status = Some("GOODSIG".to_string());
                    key_id = Some((*key).to_string());
                    summary = Some("Verification completed successfully".to_string());
                }
                ["VALIDSIG", key, ..] => {
                    key_id.get_or_insert_with(|| (*key).to_string());
                    status = Some("VALIDSIG".to_string());
                }
                ["TRUST_ULTIMATE", ..] => trust = Some("ULTIMATE".to_string()),
                ["TRUST_FULLY", ..] => trust = Some("FULLY".to_string()),
                ["TRUST_MARGINAL", ..] => trust = Some("MARGINAL".to_string()),
                ["TRUST_NEVER", ..] => trust = Some("NEVER".to_string()),
                ["TRUST_UNDEFINED", ..] => trust = Some("UNDEFINED".to_string()),
                ["TRUST_UNKNOWN", ..] => trust = Some("UNKNOWN".to_string()),
                ["ERRSIG", ..] => {
                    status = Some("ERRSIG".to_string());
                    summary = Some("Signature verification failed".to_string());
                }
                ["EXPSIG", ..] => {
                    status = Some("EXPSIG".to_string());
                    summary = Some("Signature expired".to_string());
                }
                _ => {}
            }
        }
    }

    let status = status?;
    let summary = summary.or_else(|| Some("Verification completed successfully".to_string()));
    Some(CommitSignatureInfo {
        status,
        summary,
        key_id,
        trust,
    })
}

fn secs_to_system_time(secs: i64) -> SystemTime {
    if secs >= 0 {
        SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs as u64)
    } else {
        SystemTime::UNIX_EPOCH
    }
}

fn parse_tag_name_version(tag: &str) -> (u64, u64, u64) {
    let stripped = tag.strip_prefix('v').unwrap_or(tag);
    let mut parts = stripped.split('.');

    let parse_part = |s: &str| -> u64 {
        let digits: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse().ok().unwrap_or(0)
    };

    let major = parts.next().map(parse_part).unwrap_or(0);
    let minor = parts.next().map(parse_part).unwrap_or(0);
    let patch = parts.next().map(parse_part).unwrap_or(0);
    (major, minor, patch)
}

fn git_credential_helper(url_str: &str, username: Option<&str>) -> Result<Cred, git2::Error> {
    use std::io::Write;
    let mut child = Command::new("git")
        .args(["credential", "fill"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| git2::Error::from_str(&format!("failed to spawn git credential: {}", e)))?;

    {
        let mut stdin = child.stdin.take().unwrap();
        if let Ok(url_parsed) = url::Url::parse(url_str) {
            writeln!(stdin, "protocol={}", url_parsed.scheme()).ok();
            writeln!(stdin, "host={}", url_parsed.host_str().unwrap_or("")).ok();
            let path = url_parsed.path().trim_start_matches('/');
            if !path.is_empty() {
                writeln!(stdin, "path={}", path).ok();
            }
            if let Some(user) = username {
                writeln!(stdin, "username={}", user).ok();
            } else if !url_parsed.username().is_empty() {
                writeln!(stdin, "username={}", url_parsed.username()).ok();
            }
        } else {
            writeln!(stdin, "url={}", url_str).ok();
            if let Some(user) = username {
                writeln!(stdin, "username={}", user).ok();
            }
        }
    }

    let output = child
        .wait_with_output()
        .map_err(|e| git2::Error::from_str(&format!("failed to wait for git credential: {}", e)))?;

    if !output.status.success() {
        return Err(git2::Error::from_str("git credential helper failed"));
    }

    let mut out_username = String::new();
    let mut out_password = String::new();
    let stdout_str = String::from_utf8_lossy(&output.stdout);
    for line in stdout_str.lines() {
        if let Some(val) = line.strip_prefix("username=") {
            out_username = val.to_string();
        } else if let Some(val) = line.strip_prefix("password=") {
            out_password = val.to_string();
        }
    }

    if !out_password.is_empty() {
        let user = if out_username.is_empty() {
            username.unwrap_or("git").to_string()
        } else {
            out_username
        };
        Cred::userpass_plaintext(&user, &out_password)
    } else {
        Err(git2::Error::from_str("no credentials returned from helper"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn init_temp_repo(prefix: &str) -> (std::path::PathBuf, git2::Repository) {
        let temp_dir = std::env::temp_dir().join(format!(
            "{}_{}",
            prefix,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let repo = git2::Repository::init(&temp_dir).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();
        (temp_dir, repo)
    }

    fn commit_file(repo: &GitRepo, path: &std::path::Path, contents: &str, message: &str) {
        std::fs::write(path, contents).unwrap();
        let relative = path.file_name().unwrap().to_str().unwrap();
        repo.stage_file(relative).unwrap();
        repo.commit(message, false).unwrap();
    }

    #[test]
    fn test_commits_limit_zero_and_history_stats() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let repo = git2::Repository::init(&temp_dir).unwrap();
        let git_repo = GitRepo::open(temp_dir.to_str().unwrap()).unwrap();

        // With no commits, history stats should be zero
        let (count, oldest) = git_repo.history_stats().unwrap();
        assert_eq!(count, 0);
        assert!(oldest.is_none());

        // commits() on an empty repo will return an Err because HEAD points to a non-existent ref
        assert!(git_repo.commits(Some(0)).is_err());
        assert!(git_repo.commits(None).is_err());

        // Create a commit
        let signature = git2::Signature::now("Test User", "test@example.com").unwrap();
        let tree_id = repo.index().unwrap().write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();

        let oid = repo
            .commit(
                Some("HEAD"),
                &signature,
                &signature,
                "Initial commit",
                &tree,
                &[],
            )
            .unwrap();

        // Test commits(Some(0)) returns 0 commits
        let commits = git_repo.commits(Some(0)).unwrap();
        assert_eq!(commits.len(), 0);

        // Test commits(Some(1)) returns 1 commit
        let commits_one = git_repo.commits(Some(1)).unwrap();
        assert_eq!(commits_one.len(), 1);
        assert_eq!(commits_one[0].hash, oid.to_string());

        // Test history_stats returns (1, Some(commit))
        let (count, oldest) = git_repo.history_stats().unwrap();
        assert_eq!(count, 1);
        assert_eq!(oldest.unwrap().hash, oid.to_string());

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn test_git_operations() {
        let temp_dir = std::env::temp_dir().join(format!(
            "palimpsest_test_ops_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&temp_dir).unwrap();

        let repo = git2::Repository::init(&temp_dir).unwrap();
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "Test User").unwrap();
        config.set_str("user.email", "test@example.com").unwrap();

        let git_repo = GitRepo::open(temp_dir.to_str().unwrap()).unwrap();
        let initial_branch = git_repo.status().unwrap().branch;

        // 1. Create a dummy file and stage it
        let dummy_path = temp_dir.join("dummy.txt");
        std::fs::write(&dummy_path, "initial content").unwrap();
        git_repo.stage_file("dummy.txt").unwrap();

        // 2. Commit it
        git_repo.commit("Initial Commit", false).unwrap();
        let (count, _) = git_repo.history_stats().unwrap();
        assert_eq!(count, 1);

        // 3. Create branch and verify
        git_repo.create_branch("feature-branch").unwrap();
        let branches = git_repo.branches().unwrap();
        assert!(branches.iter().any(|b| b.name == "feature-branch"));

        // 4. Checkout branch
        git_repo.checkout_branch("feature-branch").unwrap();
        let status = git_repo.status().unwrap();
        assert_eq!(status.branch, "feature-branch");

        // 5. Unstage all
        std::fs::write(&dummy_path, "modified content").unwrap();
        git_repo.stage_file("dummy.txt").unwrap();
        let status = git_repo.status().unwrap();
        assert_eq!(status.staged_count, 1);

        git_repo.unstage_all().unwrap();
        let status = git_repo.status().unwrap();
        assert_eq!(status.staged_count, 0);
        assert_eq!(status.unstaged_count, 1);

        // 6. Stash save, apply, drop, pop
        git_repo.stage_file("dummy.txt").unwrap();
        git_repo.stash_save(Some("stash-msg")).unwrap();
        let status = git_repo.status().unwrap();
        assert_eq!(status.staged_count, 0);
        assert_eq!(status.unstaged_count, 0);

        let stashes = git_repo.stashes().unwrap();
        assert_eq!(stashes.len(), 1);
        assert!(stashes[0].message.contains("stash-msg"));

        git_repo.stash_pop(0).unwrap();
        let status = git_repo.status().unwrap();
        assert_eq!(status.unstaged_count, 1);
        assert_eq!(git_repo.stashes().unwrap().len(), 0);

        git_repo.stage_file("dummy.txt").unwrap();
        git_repo.stash_save(None).unwrap();
        assert_eq!(git_repo.stashes().unwrap().len(), 1);
        git_repo.stash_apply(0).unwrap();
        git_repo.stash_drop(0).unwrap();
        assert_eq!(git_repo.stashes().unwrap().len(), 0);

        // 7. Delete branch (must checkout initial branch first)
        git_repo.checkout_branch(&initial_branch).unwrap();
        git_repo.delete_branch("feature-branch").unwrap();
        let branches = git_repo.branches().unwrap();
        assert!(!branches.iter().any(|b| b.name == "feature-branch"));

        std::fs::remove_dir_all(&temp_dir).unwrap();
    }

    #[test]
    fn parse_commit_signature_output_parses_goodsig() {
        let output = "[GNUPG:] GOODSIG ABCDEF1234567890ABCDEF1234567890ABCDEF12 Example User <example@invalid>\n[GNUPG:] VALIDSIG ABCDEF1234567890ABCDEF1234567890ABCDEF12 2026-05-25 2026-05-25 0 4 0 1 10 00 0 0 0 0 0\n[GNUPG:] TRUST_ULTIMATE 0 pgp";
        let parsed = parse_commit_signature_output(output).unwrap();
        assert_eq!(parsed.status, "VALIDSIG");
        assert_eq!(
            parsed.key_id.as_deref(),
            Some("ABCDEF1234567890ABCDEF1234567890ABCDEF12")
        );
        assert_eq!(parsed.trust.as_deref(), Some("ULTIMATE"));
        assert_eq!(
            parsed.summary.as_deref(),
            Some("Verification completed successfully")
        );
    }

    #[test]
    fn parse_commit_signature_output_handles_unsigned() {
        assert!(parse_commit_signature_output("").is_none());
    }

    #[test]
    fn commit_diff_view_reports_modified_file() {
        let (temp_dir, repo) = init_temp_repo("palimpsest_diff_modify");
        let git_repo = GitRepo::open(temp_dir.to_str().unwrap()).unwrap();

        let file_path = temp_dir.join("file.txt");
        commit_file(&git_repo, &file_path, "hello\n", "initial");

        std::fs::write(&file_path, "hello\nworld\n").unwrap();
        git_repo.stage_file("file.txt").unwrap();
        git_repo.commit("modify", false).unwrap();

        let diff = git_repo.commit_diff_view("HEAD").unwrap();
        assert_eq!(diff.summary.files_changed, 1);
        assert_eq!(diff.files.len(), 1);
        assert!(matches!(diff.files[0].kind, CommitDiffFileKind::Modified));
        assert_eq!(diff.files[0].path, "file.txt");
        assert!(diff.files[0].additions >= 1);

        drop(repo);
        std::fs::remove_dir_all(&temp_dir).unwrap();
    }
}
