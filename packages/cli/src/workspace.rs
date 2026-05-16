//! Workspace root enforcement for filesystem-touching tools.
//!
//! A [`Workspace`] carries a canonical root directory plus a policy flag.
//! Every filesystem tool resolves caller-supplied paths through
//! [`Workspace::resolve`] (for paths that must already exist) or
//! [`Workspace::resolve_for_create`] (for paths that may not yet exist,
//! like `write`'s target file). Both refuse paths that escape the root
//! after symlink resolution.
//!
//! The [`Workspace::unrestricted`] constructor disables the check and
//! exists solely so legacy callers and the existing unit tests keep
//! working without change. Production entry points (`main.rs`) build a
//! restricted workspace via [`Workspace::detect`].

use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

use anyhow::{Context, Result};

/// Shared handle passed into every filesystem-touching tool.
#[derive(Debug, Clone)]
pub struct Workspace {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    /// Canonical absolute path to the workspace root. For an
    /// unrestricted workspace this is still set (to cwd) but
    /// `restricted` is false so checks are skipped.
    root: PathBuf,
    restricted: bool,
}

impl Workspace {
    /// Build a restricted workspace. If `override_root` is `Some`, use
    /// it; otherwise walk up from cwd looking for a `.git` directory,
    /// and fall back to cwd when none is found.
    pub fn detect(override_root: Option<&Path>) -> Result<Self> {
        let root = match override_root {
            Some(p) => p.to_path_buf(),
            None => find_git_root().unwrap_or(std::env::current_dir()?),
        };
        let root = std::fs::canonicalize(&root)
            .with_context(|| format!("canonicalizing workspace root {}", root.display()))?;
        if !root.is_dir() {
            anyhow::bail!("workspace root is not a directory: {}", root.display());
        }
        Ok(Self {
            inner: Arc::new(Inner { root, restricted: true }),
        })
    }

    /// Build an unrestricted workspace rooted at cwd. No path checks
    /// are performed. Kept for backward compatibility with callers and
    /// tests that predate workspace enforcement; new production code
    /// should use [`Workspace::detect`].
    pub fn unrestricted() -> Self {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self {
            inner: Arc::new(Inner { root, restricted: false }),
        }
    }

    /// Canonical workspace root (always absolute).
    pub fn root(&self) -> &Path {
        &self.inner.root
    }

    /// Whether this workspace enforces the root boundary.
    pub fn is_restricted(&self) -> bool {
        self.inner.restricted
    }

    /// Resolve a caller-provided path that is expected to already exist.
    /// Returns the canonical absolute path. On a restricted workspace,
    /// the result must live under `root` after symlink resolution.
    pub fn resolve(&self, input: &str) -> Result<PathBuf> {
        let joined = self.join(input);
        // `canonicalize` requires the path to exist. We want a clean
        // error for non-existent paths rather than "escapes workspace",
        // so let the caller handle the not-found case via their normal
        // I/O error path. Here we only enforce the boundary when the
        // path resolves successfully.
        let canonical = std::fs::canonicalize(&joined)
            .with_context(|| format!("resolving path {}", joined.display()))?;
        self.ensure_within(&canonical, input)?;
        Ok(canonical)
    }

    /// Resolve a caller-provided path that does not need to exist yet
    /// (e.g. a `write` target). The parent directory must exist *or*
    /// be creatable inside the workspace. We canonicalize the nearest
    /// existing ancestor and confirm it lives under `root`, then
    /// re-attach the non-existent tail.
    pub fn resolve_for_create(&self, input: &str) -> Result<PathBuf> {
        let joined = self.join(input);
        let (existing, tail) = split_existing(&joined);
        let canonical_existing = std::fs::canonicalize(&existing).with_context(|| {
            format!(
                "resolving nearest existing ancestor {} for {}",
                existing.display(),
                joined.display()
            )
        })?;
        self.ensure_within(&canonical_existing, input)?;
        let mut out = canonical_existing;
        for c in tail.components() {
            match c {
                Component::Normal(s) => out.push(s),
                // split_existing only keeps Normal components in the tail,
                // so anything else is a bug on our side.
                other => anyhow::bail!("unexpected path component in tail: {:?}", other),
            }
        }
        // The tail couldn't have introduced a symlink (it doesn't exist
        // yet), so a lexical check is sufficient here.
        if self.inner.restricted && !out.starts_with(&self.inner.root) {
            anyhow::bail!(
                "path {} escapes workspace root {}",
                out.display(),
                self.inner.root.display()
            );
        }
        Ok(out)
    }

    fn join(&self, input: &str) -> PathBuf {
        let p = Path::new(input);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.inner.root.join(p)
        }
    }

    fn ensure_within(&self, canonical: &Path, original: &str) -> Result<()> {
        if !self.inner.restricted {
            return Ok(());
        }
        if canonical.starts_with(&self.inner.root) {
            return Ok(());
        }
        anyhow::bail!(
            "path {} (resolved to {}) is outside workspace root {}",
            original,
            canonical.display(),
            self.inner.root.display()
        );
    }
}

/// Walk upward from cwd looking for a directory that contains `.git`.
/// Returns `None` if none is found before hitting the filesystem root.
fn find_git_root() -> Option<PathBuf> {
    let mut cur = std::env::current_dir().ok()?;
    loop {
        if cur.join(".git").exists() {
            return Some(cur);
        }
        if !cur.pop() {
            return None;
        }
    }
}

/// Split a path into (nearest existing ancestor, remaining tail).
/// For `/a/b/c/d.txt` where `/a/b` exists but `/a/b/c` doesn't, returns
/// (`/a/b`, `c/d.txt`). Always returns an existing ancestor; worst case
/// it's the filesystem root, which exists by definition.
fn split_existing(p: &Path) -> (PathBuf, PathBuf) {
    let mut existing = p.to_path_buf();
    let mut tail_parts: Vec<PathBuf> = Vec::new();
    while !existing.exists() {
        match existing.file_name() {
            Some(name) => tail_parts.push(PathBuf::from(name)),
            None => break, // hit root
        }
        if !existing.pop() {
            break;
        }
    }
    let mut tail = PathBuf::new();
    for part in tail_parts.into_iter().rev() {
        tail.push(part);
    }
    (existing, tail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ws(dir: &Path) -> Workspace {
        Workspace::detect(Some(dir)).unwrap()
    }

    #[test]
    fn resolve_existing_file_inside_root() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("a.txt");
        std::fs::write(&f, "x").unwrap();
        let w = ws(tmp.path());
        let r = w.resolve("a.txt").unwrap();
        assert_eq!(r, std::fs::canonicalize(&f).unwrap());
    }

    #[test]
    fn resolve_absolute_inside_root() {
        let tmp = TempDir::new().unwrap();
        let f = tmp.path().join("a.txt");
        std::fs::write(&f, "x").unwrap();
        let w = ws(tmp.path());
        let r = w.resolve(f.to_str().unwrap()).unwrap();
        assert_eq!(r, std::fs::canonicalize(&f).unwrap());
    }

    #[test]
    fn resolve_rejects_parent_escape() {
        let tmp = TempDir::new().unwrap();
        let sub = tmp.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let outside = tmp.path().join("outside.txt");
        std::fs::write(&outside, "x").unwrap();
        let w = ws(&sub);
        let err = w.resolve("../outside.txt").unwrap_err().to_string();
        assert!(err.contains("outside workspace root"), "got: {err}");
    }

    #[test]
    fn resolve_rejects_absolute_outside() {
        let tmp = TempDir::new().unwrap();
        let w = ws(tmp.path());
        let err = w.resolve("/etc/hostname").unwrap_err().to_string();
        // Either "outside workspace root" (if it exists) or a resolve
        // error (if it doesn't) — both are acceptable rejections.
        assert!(
            err.contains("outside workspace root") || err.contains("resolving path"),
            "got: {err}"
        );
    }

    #[test]
    #[cfg(unix)]
    fn resolve_rejects_symlink_escape() {
        let tmp = TempDir::new().unwrap();
        let outside_dir = TempDir::new().unwrap();
        let secret = outside_dir.path().join("secret.txt");
        std::fs::write(&secret, "top-secret").unwrap();

        let link = tmp.path().join("escape");
        std::os::unix::fs::symlink(&secret, &link).unwrap();

        let w = ws(tmp.path());
        let err = w.resolve("escape").unwrap_err().to_string();
        assert!(err.contains("outside workspace root"), "got: {err}");
    }

    #[test]
    fn resolve_for_create_new_file_inside_root() {
        let tmp = TempDir::new().unwrap();
        let w = ws(tmp.path());
        let r = w.resolve_for_create("newdir/new.txt").unwrap();
        assert!(r.starts_with(std::fs::canonicalize(tmp.path()).unwrap()));
        assert!(r.ends_with("newdir/new.txt"));
    }

    #[test]
    fn resolve_for_create_rejects_outside() {
        let tmp = TempDir::new().unwrap();
        let w = ws(tmp.path());
        let err = w
            .resolve_for_create("../evil.txt")
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("escapes workspace root") || err.contains("outside workspace root"),
            "got: {err}"
        );
    }

    #[test]
    fn unrestricted_accepts_anything() {
        let w = Workspace::unrestricted();
        // Just pick a path that definitely exists.
        assert!(w.resolve("/").is_ok() || w.resolve(".").is_ok());
        assert!(!w.is_restricted());
    }
}
