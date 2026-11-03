//! Merging finished task branches into one integration branch. The user's own
//! checkout is never touched: merges happen in a dedicated worktree on
//! `orchestra/<run>`, and a conflict is left in place for the supervisor to
//! resolve before `continue`.

use std::path::{Path, PathBuf};

use super::worktree::{git, git_raw};

#[derive(Debug, PartialEq)]
pub enum Merged {
    /// The branch was merged; `files` are the files it changed.
    Done {
        files: usize,
    },
    AlreadyIn,
    /// Git stopped on these files; the merge is still open in the worktree.
    Conflict {
        files: Vec<String>,
    },
}

pub fn integration_branch(run_id: &str) -> String {
    format!("orchestra/{run_id}")
}

pub fn integration_worktree(run_dir: &Path) -> PathBuf {
    run_dir.join("integration").join("worktree")
}

/// Creates the integration worktree on first use, cut from the run's base.
pub fn ensure_worktree(
    repo: &Path,
    run_dir: &Path,
    run_id: &str,
    base: &str,
) -> Result<PathBuf, String> {
    let path = integration_worktree(run_dir);
    if path.exists() {
        return Ok(path);
    }
    if base.starts_with('-') {
        return Err("refusing an option-like git ref".to_string());
    }
    let dir = path.to_str().ok_or("integration path is not valid UTF-8")?;
    std::fs::create_dir_all(run_dir.join("integration"))
        .map_err(|error| format!("create integration dir: {error}"))?;
    let branch = integration_branch(run_id);
    // A branch left by an earlier run of the same id is reused, not recreated.
    let exists = git_raw(
        repo,
        &[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ],
    )?
    .0;
    if exists {
        git(repo, &["worktree", "add", "--quiet", dir, &branch])?;
    } else {
        git(
            repo,
            &["worktree", "add", "--quiet", "-b", &branch, dir, base],
        )?;
    }
    Ok(path)
}

pub fn rev(cwd: &Path, name: &str) -> Option<String> {
    git(cwd, &["rev-parse", "--verify", "--quiet", name])
        .ok()
        .map(|out| out.trim().to_string())
        .filter(|sha| !sha.is_empty())
}

pub fn commits_ahead(repo: &Path, base: &str, branch: &str) -> usize {
    git(repo, &["rev-list", "--count", &format!("{base}..{branch}")])
        .ok()
        .and_then(|out| out.trim().parse().ok())
        .unwrap_or(0)
}

/// The task branch has commits and the integration branch already holds them.
pub fn is_merged(worktree: &Path, base: &str, branch: &str) -> bool {
    commits_ahead(worktree, base, branch) > 0
        && git_raw(worktree, &["merge-base", "--is-ancestor", branch, "HEAD"])
            .map(|(ok, _, _)| ok)
            .unwrap_or(false)
}

pub fn conflicted_files(worktree: &Path) -> Result<Vec<String>, String> {
    let out = git(worktree, &["diff", "--name-only", "--diff-filter=U"])?;
    Ok(out.lines().map(str::to_string).collect())
}

pub fn merge_in_progress(worktree: &Path) -> bool {
    git_raw(
        worktree,
        &["rev-parse", "--verify", "--quiet", "MERGE_HEAD"],
    )
    .map(|(ok, _, _)| ok)
    .unwrap_or(false)
}

/// Tracked or untracked changes a worker left uncommitted would be lost by a
/// branch merge, so they block it.
pub fn uncommitted(task_worktree: &Path) -> Result<Vec<String>, String> {
    let out = git(task_worktree, &["status", "--porcelain"])?;
    Ok(out
        .lines()
        .map(|line| line.get(3..).unwrap_or(line).to_string())
        .collect())
}

pub fn merge_branch(worktree: &Path, branch: &str, message: &str) -> Result<Merged, String> {
    if branch.starts_with('-') {
        return Err("refusing an option-like git ref".to_string());
    }
    if merge_in_progress(worktree) {
        return Err("a merge is still open; resolve it and run merge --continue".to_string());
    }
    let (already, _, _) = git_raw(worktree, &["merge-base", "--is-ancestor", branch, "HEAD"])?;
    if already {
        return Ok(Merged::AlreadyIn);
    }
    let (ok, _, stderr) = git_raw(worktree, &["merge", "--no-ff", "-m", message, branch])?;
    if ok {
        return Ok(Merged::Done {
            files: changed_files(worktree)?,
        });
    }
    let files = conflicted_files(worktree)?;
    if files.is_empty() {
        let _ = git_raw(worktree, &["merge", "--abort"]);
        return Err(format!("git merge: {}", stderr.trim()));
    }
    Ok(Merged::Conflict { files })
}

fn changed_files(worktree: &Path) -> Result<usize, String> {
    let out = git(worktree, &["diff", "--name-only", "HEAD~1", "HEAD"])?;
    Ok(out.lines().count())
}

/// Commits a resolved conflict. Refuses while any file is still unmerged or
/// still carries a conflict marker.
pub fn merge_continue(worktree: &Path) -> Result<(), String> {
    let merging = merge_in_progress(worktree);
    if !merging && git(worktree, &["status", "--porcelain"])?.trim().is_empty() {
        return Err("no merge is open and nothing is waiting to be committed".to_string());
    }
    git(worktree, &["add", "-A"])?;
    let (found, hits, _) = git_raw(
        worktree,
        &[
            "grep",
            "--cached",
            "-n",
            "-E",
            "^(<<<<<<<|>>>>>>>) ",
            "--",
            ".",
        ],
    )?;
    if found && !hits.trim().is_empty() {
        let first: Vec<&str> = hits.lines().take(5).collect();
        return Err(format!(
            "conflict markers are still in the files:\n{}",
            first.join("\n")
        ));
    }
    if merging {
        git(worktree, &["commit", "--no-edit"]).map(|_| ())
    } else {
        git(worktree, &["commit", "-m", "Integration fixes"]).map(|_| ())
    }
}

pub fn merge_abort(worktree: &Path) -> Result<(), String> {
    if !merge_in_progress(worktree) {
        return Err("no merge is open".to_string());
    }
    git(worktree, &["merge", "--abort"]).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn run(cwd: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(["-c", "user.email=t@example.com", "-c", "user.name=t"])
            .args(args)
            .current_dir(cwd)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    struct Fixture {
        repo: tempfile::TempDir,
        run_dir: tempfile::TempDir,
    }

    fn fixture() -> Fixture {
        let repo = tempfile::tempdir().unwrap();
        run(repo.path(), &["init", "--quiet", "-b", "main"]);
        std::fs::write(repo.path().join("a.txt"), "one\ntwo\nthree\n").unwrap();
        std::fs::write(repo.path().join("b.txt"), "b\n").unwrap();
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "--quiet", "-m", "init"]);
        Fixture {
            repo,
            run_dir: tempfile::tempdir().unwrap(),
        }
    }

    impl Fixture {
        fn branch(&self, name: &str, file: &str, contents: &str) {
            run(
                self.repo.path(),
                &["checkout", "--quiet", "-b", name, "main"],
            );
            std::fs::write(self.repo.path().join(file), contents).unwrap();
            run(self.repo.path(), &["commit", "--quiet", "-am", name]);
            run(self.repo.path(), &["checkout", "--quiet", "main"]);
        }

        fn integration(&self) -> PathBuf {
            ensure_worktree(self.repo.path(), self.run_dir.path(), "r1", "main").unwrap()
        }
    }

    #[test]
    fn merges_disjoint_branches_and_reports_already_merged() {
        let f = fixture();
        f.branch("task/r1-1", "a.txt", "ONE\ntwo\nthree\n");
        f.branch("task/r1-2", "b.txt", "B\n");
        let wt = f.integration();
        assert_eq!(
            merge_branch(&wt, "task/r1-1", "m1").unwrap(),
            Merged::Done { files: 1 }
        );
        assert_eq!(
            merge_branch(&wt, "task/r1-2", "m2").unwrap(),
            Merged::Done { files: 1 }
        );
        assert_eq!(
            merge_branch(&wt, "task/r1-2", "m2").unwrap(),
            Merged::AlreadyIn
        );
        assert_eq!(
            std::fs::read_to_string(wt.join("a.txt")).unwrap(),
            "ONE\ntwo\nthree\n"
        );
        assert_eq!(std::fs::read_to_string(wt.join("b.txt")).unwrap(), "B\n");
        assert_eq!(
            std::fs::read_to_string(f.repo.path().join("a.txt")).unwrap(),
            "one\ntwo\nthree\n",
            "the user's checkout is untouched"
        );
        assert_eq!(
            ensure_worktree(f.repo.path(), f.run_dir.path(), "r1", "main").unwrap(),
            wt
        );
    }

    #[test]
    fn a_conflict_stays_open_until_resolved_then_continues() {
        let f = fixture();
        f.branch("task/r1-1", "a.txt", "ONE\ntwo\nthree\n");
        f.branch("task/r1-2", "a.txt", "uno\ntwo\nthree\n");
        let wt = f.integration();
        merge_branch(&wt, "task/r1-1", "m1").unwrap();
        let Merged::Conflict { files } = merge_branch(&wt, "task/r1-2", "m2").unwrap() else {
            panic!("expected a conflict");
        };
        assert_eq!(files, ["a.txt"]);
        assert!(merge_in_progress(&wt));
        assert!(merge_branch(&wt, "task/r1-2", "m2").is_err());

        let err = merge_continue(&wt).unwrap_err();
        assert!(err.contains("conflict markers"), "{err}");

        std::fs::write(wt.join("a.txt"), "ONE uno\ntwo\nthree\n").unwrap();
        merge_continue(&wt).unwrap();
        assert!(!merge_in_progress(&wt));
        assert_eq!(
            merge_branch(&wt, "task/r1-2", "m2").unwrap(),
            Merged::AlreadyIn
        );
        assert!(merge_continue(&wt).is_err(), "nothing left to commit");

        std::fs::write(wt.join("b.txt"), "fixed\n").unwrap();
        merge_continue(&wt).unwrap();
        assert!(uncommitted(&wt).unwrap().is_empty());
    }

    #[test]
    fn abort_drops_the_conflict() {
        let f = fixture();
        f.branch("task/r1-1", "a.txt", "ONE\ntwo\nthree\n");
        f.branch("task/r1-2", "a.txt", "uno\ntwo\nthree\n");
        let wt = f.integration();
        merge_branch(&wt, "task/r1-1", "m1").unwrap();
        assert!(matches!(
            merge_branch(&wt, "task/r1-2", "m2").unwrap(),
            Merged::Conflict { .. }
        ));
        merge_abort(&wt).unwrap();
        assert!(!merge_in_progress(&wt));
        assert_eq!(
            std::fs::read_to_string(wt.join("a.txt")).unwrap(),
            "ONE\ntwo\nthree\n"
        );
        assert!(merge_abort(&wt).is_err());
    }

    #[test]
    fn uncommitted_work_in_a_task_worktree_blocks_its_merge() {
        let f = fixture();
        assert!(uncommitted(f.repo.path()).unwrap().is_empty());
        std::fs::write(f.repo.path().join("a.txt"), "changed\n").unwrap();
        std::fs::write(f.repo.path().join("new.txt"), "x\n").unwrap();
        let mut dirty = uncommitted(f.repo.path()).unwrap();
        dirty.sort();
        assert_eq!(dirty, ["a.txt", "new.txt"]);
    }

    #[test]
    fn option_like_refs_are_refused() {
        let f = fixture();
        let wt = f.integration();
        assert!(merge_branch(&wt, "--upload-pack=x", "m").is_err());
        let fresh = tempfile::tempdir().unwrap();
        assert!(ensure_worktree(f.repo.path(), fresh.path(), "r2", "-x").is_err());
    }
}
