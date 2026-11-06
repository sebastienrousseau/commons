//! Path containment: join untrusted relative paths under a root.
//!
//! Several of the maintainer's crates (a static site generator, front
//! matter and data generators, an HTTP server) each carried their own
//! traversal guard, with different rules: one rejected any `..` but never
//! checked containment, one rejected names such as `a..b.md`, one rewrote
//! bad paths instead of refusing them. This module is the one shared rule.
//!
//! - [`safe_join`] is lexical: it never touches the filesystem, so it works
//!   for paths that do not exist yet (output files, upload targets).
//! - [`is_within`] canonicalises both sides, so it also sees through
//!   symlinks, but needs the path to exist.
//!
//! Use `safe_join` to build a path from untrusted input, and `is_within`
//! before reading through it when the tree may contain symlinks.
//!
//! # Example
//!
//! ```rust
//! use commons::path::{safe_join, PathError};
//!
//! let page = safe_join("/srv/site", "blog/post.html").unwrap();
//! assert_eq!(page, std::path::Path::new("/srv/site/blog/post.html"));
//!
//! assert_eq!(safe_join("/srv/site", "../etc/passwd"), Err(PathError::EscapesRoot));
//! assert_eq!(safe_join("/srv/site", "/etc/passwd"), Err(PathError::Absolute));
//! ```

use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Why an untrusted path was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PathError {
    /// The path is absolute, or carries a Windows prefix such as `C:` or
    /// `\\server\share`, so joining it would discard the root.
    Absolute,
    /// A `..` component climbs above the root.
    EscapesRoot,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Absolute => f.write_str("path is absolute or has a drive or share prefix"),
            Self::EscapesRoot => f.write_str("path climbs above its root"),
        }
    }
}

impl std::error::Error for PathError {}

/// Join `untrusted` under `root`, refusing anything that would leave it.
///
/// The result is `root` followed by the normal components of `untrusted`,
/// with `.` dropped and each `..` removing the component before it. Names
/// that merely contain dots, such as `a..b.md` or `.hidden`, are ordinary
/// names. The filesystem is never consulted, so symlinks inside `root` are
/// not resolved; check an existing path with [`is_within`] when that
/// matters.
///
/// # Errors
///
/// - [`PathError::Absolute`] if `untrusted` is absolute or has a Windows
///   drive or share prefix.
/// - [`PathError::EscapesRoot`] if a `..` would climb above `root`, even
///   transiently (`a/../../b`).
pub fn safe_join(
    root: impl AsRef<Path>,
    untrusted: impl AsRef<Path>,
) -> Result<PathBuf, PathError> {
    let mut kept: Vec<&std::ffi::OsStr> = Vec::new();
    for component in untrusted.as_ref().components() {
        match component {
            Component::Prefix(_) | Component::RootDir => return Err(PathError::Absolute),
            Component::CurDir => {}
            Component::ParentDir => {
                if kept.pop().is_none() {
                    return Err(PathError::EscapesRoot);
                }
            }
            Component::Normal(name) => kept.push(name),
        }
    }
    let mut joined = root.as_ref().to_path_buf();
    joined.extend(kept);
    Ok(joined)
}

/// Whether the existing `path` lies inside the existing `root`, after
/// resolving symlinks and `..` on both.
///
/// # Errors
///
/// Returns the I/O error if either path cannot be canonicalised, for
/// example because it does not exist.
pub fn is_within(root: impl AsRef<Path>, path: impl AsRef<Path>) -> io::Result<bool> {
    let root = root.as_ref().canonicalize()?;
    let path = path.as_ref().canonicalize()?;
    Ok(path.starts_with(&root))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn joined(rel: &str) -> Result<PathBuf, PathError> {
        safe_join("root", rel)
    }

    #[test]
    fn joins_plain_relative_paths() {
        assert_eq!(joined("a/b.md"), Ok(PathBuf::from("root/a/b.md")));
        assert_eq!(joined(""), Ok(PathBuf::from("root")));
        assert_eq!(joined("./a/./b"), Ok(PathBuf::from("root/a/b")));
    }

    #[test]
    fn names_with_dots_are_ordinary() {
        // A substring check for ".." used to reject these.
        assert_eq!(joined("a..b.md"), Ok(PathBuf::from("root/a..b.md")));
        assert_eq!(joined("..hidden"), Ok(PathBuf::from("root/..hidden")));
        assert_eq!(
            joined(".well-known/x"),
            Ok(PathBuf::from("root/.well-known/x"))
        );
    }

    #[test]
    fn parent_components_stay_inside() {
        assert_eq!(joined("a/../b"), Ok(PathBuf::from("root/b")));
        assert_eq!(joined("a/b/../../c"), Ok(PathBuf::from("root/c")));
    }

    #[test]
    fn climbing_out_is_refused() {
        assert_eq!(joined(".."), Err(PathError::EscapesRoot));
        assert_eq!(joined("../x"), Err(PathError::EscapesRoot));
        assert_eq!(joined("a/../../x"), Err(PathError::EscapesRoot));
        // Escaping and coming back is still an escape.
        assert_eq!(joined("../root/x"), Err(PathError::EscapesRoot));
    }

    #[test]
    fn absolute_paths_are_refused() {
        assert_eq!(joined("/etc/passwd"), Err(PathError::Absolute));
        #[cfg(windows)]
        {
            assert_eq!(joined(r"C:\Windows"), Err(PathError::Absolute));
            assert_eq!(joined(r"C:relative"), Err(PathError::Absolute));
            assert_eq!(joined(r"\\server\share\x"), Err(PathError::Absolute));
        }
    }

    #[test]
    fn errors_display() {
        assert!(PathError::Absolute.to_string().contains("absolute"));
        assert!(PathError::EscapesRoot.to_string().contains("above"));
    }

    #[test]
    fn is_within_follows_symlinks() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let inside = root.join("page.md");
        std::fs::write(&inside, "x").unwrap();
        assert!(is_within(&root, &inside).unwrap());

        let outside = dir.path().join("secret");
        std::fs::write(&outside, "x").unwrap();
        assert!(!is_within(&root, &outside).unwrap());

        #[cfg(unix)]
        {
            let link = root.join("link");
            std::os::unix::fs::symlink(&outside, &link).unwrap();
            // safe_join accepts the name; is_within sees where it points.
            assert_eq!(safe_join(&root, "link"), Ok(link.clone()));
            assert!(!is_within(&root, &link).unwrap());
        }

        assert!(is_within(&root, root.join("missing")).is_err());
    }

    mod properties {
        use super::super::*;
        use proptest::prelude::*;

        fn component() -> impl Strategy<Value = String> {
            prop_oneof![
                Just("..".to_string()),
                Just(".".to_string()),
                Just(String::new()),
                "[a-z.]{1,4}",
            ]
        }

        proptest! {
            /// Whatever the input, an accepted path starts with the root
            /// and contains no `..` component.
            #[test]
            fn accepted_paths_stay_under_root(parts in prop::collection::vec(component(), 0..8)) {
                let rel = parts.join("/");
                if let Ok(p) = safe_join("root", &rel) {
                    prop_assert!(p.starts_with("root"));
                    prop_assert!(p.components().all(|c| c != Component::ParentDir));
                }
            }

            /// A path is refused exactly when it is absolute or some prefix
            /// has climbed above the root.
            #[test]
            fn refused_exactly_when_depth_goes_negative(parts in prop::collection::vec(component(), 0..8)) {
                let rel = parts.join("/");
                let mut depth: i32 = 0;
                let mut escaped = false;
                for c in Path::new(&rel).components() {
                    match c {
                        Component::ParentDir => { depth -= 1; if depth < 0 { escaped = true; } }
                        Component::Normal(_) => depth += 1,
                        Component::RootDir | Component::Prefix(_) => escaped = true,
                        Component::CurDir => {}
                    }
                }
                prop_assert_eq!(safe_join("root", &rel).is_err(), escaped);
            }
        }
    }
}
