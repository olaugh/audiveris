// SPDX-License-Identifier: AGPL-3.0-or-later

//! Small, dependency-free helpers for cross-runtime fixtures and snapshots.
//!
//! This crate deliberately contains no recognition logic. It provides a stable
//! key/value interchange format, useful first-difference diagnostics, and a
//! fixture-root boundary that rejects absolute paths and directory traversal.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// A deterministic collection of single-line canonical vectors.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CanonicalVectors {
    entries: BTreeMap<String, String>,
}

impl CanonicalVectors {
    /// Create an empty collection.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
        }
    }

    /// Insert one unique key/value pair.
    ///
    /// Keys must be non-empty and may not contain `=`, line feeds, or carriage
    /// returns. Values may be empty but must fit on one line.
    pub fn insert(
        &mut self,
        key: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<(), VectorError> {
        let key = key.into();
        let value = value.into();
        validate_key(&key)?;
        if value.contains(['\n', '\r']) {
            return Err(VectorError::MultilineValue(key));
        }
        if self.entries.contains_key(&key) {
            return Err(VectorError::DuplicateKey(key));
        }
        self.entries.insert(key, value);
        Ok(())
    }

    /// Number of vectors in the collection.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the collection contains no vectors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Value for `key`, when present.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(String::as_str)
    }

    /// Iterate in canonical lexicographic key order.
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&str, &str)> {
        self.entries
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
    }

    /// Render sorted `key=value` lines with one final newline when non-empty.
    #[must_use]
    pub fn render(&self) -> String {
        let mut rendered = String::new();
        for (key, value) in &self.entries {
            rendered.push_str(key);
            rendered.push('=');
            rendered.push_str(value);
            rendered.push('\n');
        }
        rendered
    }

    /// Report the first difference in canonical key order.
    #[must_use]
    pub fn first_difference(&self, actual: &Self) -> Option<VectorDifference> {
        let mut expected_entries = self.entries.iter().peekable();
        let mut actual_entries = actual.entries.iter().peekable();

        loop {
            match (expected_entries.peek(), actual_entries.peek()) {
                (None, None) => return None,
                (Some((key, expected)), None) => {
                    return Some(VectorDifference::MissingActual {
                        key: (*key).clone(),
                        expected: (*expected).clone(),
                    });
                }
                (None, Some((key, value))) => {
                    return Some(VectorDifference::UnexpectedActual {
                        key: (*key).clone(),
                        actual: (*value).clone(),
                    });
                }
                (Some((expected_key, expected)), Some((actual_key, actual_value))) => {
                    match expected_key.cmp(actual_key) {
                        std::cmp::Ordering::Less => {
                            return Some(VectorDifference::MissingActual {
                                key: (*expected_key).clone(),
                                expected: (*expected).clone(),
                            });
                        }
                        std::cmp::Ordering::Greater => {
                            return Some(VectorDifference::UnexpectedActual {
                                key: (*actual_key).clone(),
                                actual: (*actual_value).clone(),
                            });
                        }
                        std::cmp::Ordering::Equal => {
                            if expected != actual_value {
                                return Some(VectorDifference::ValueMismatch {
                                    key: (*expected_key).clone(),
                                    expected: (*expected).clone(),
                                    actual: (*actual_value).clone(),
                                });
                            }
                            expected_entries.next();
                            actual_entries.next();
                        }
                    }
                }
            }
        }
    }
}

/// Invalid canonical-vector input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VectorError {
    /// A key is empty or contains format delimiters.
    InvalidKey(String),
    /// A value contains a line terminator.
    MultilineValue(String),
    /// The key was already inserted.
    DuplicateKey(String),
}

impl fmt::Display for VectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidKey(key) => write!(formatter, "invalid canonical vector key {key:?}"),
            Self::MultilineValue(key) => {
                write!(formatter, "canonical vector {key:?} has a multiline value")
            }
            Self::DuplicateKey(key) => write!(formatter, "duplicate canonical vector key {key:?}"),
        }
    }
}

impl Error for VectorError {}

/// The earliest mismatch between expected and actual vectors.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VectorDifference {
    /// Expected vector is absent from the actual collection.
    MissingActual {
        /// Canonical vector key.
        key: String,
        /// Expected value.
        expected: String,
    },
    /// Actual collection contains an unexpected vector.
    UnexpectedActual {
        /// Canonical vector key.
        key: String,
        /// Unexpected value.
        actual: String,
    },
    /// Both collections contain a key but its values differ.
    ValueMismatch {
        /// Canonical vector key.
        key: String,
        /// Expected value.
        expected: String,
        /// Actual value.
        actual: String,
    },
}

impl fmt::Display for VectorDifference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingActual { key, expected } => {
                write!(
                    formatter,
                    "missing actual vector {key:?}; expected {expected:?}"
                )
            }
            Self::UnexpectedActual { key, actual } => {
                write!(formatter, "unexpected actual vector {key:?} = {actual:?}")
            }
            Self::ValueMismatch {
                key,
                expected,
                actual,
            } => write!(
                formatter,
                "vector {key:?} differs: expected {expected:?}, actual {actual:?}"
            ),
        }
    }
}

/// Canonical directory boundary for read-only fixture lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FixtureRoot {
    canonical: PathBuf,
}

impl FixtureRoot {
    /// Open and canonicalize an existing fixture directory.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, FixtureError> {
        let requested = path.as_ref();
        let canonical = fs::canonicalize(requested).map_err(|source| FixtureError::Io {
            path: requested.to_path_buf(),
            source: source.kind(),
        })?;
        if !canonical.is_dir() {
            return Err(FixtureError::NotDirectory(canonical));
        }
        Ok(Self { canonical })
    }

    /// Canonical fixture root.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.canonical
    }

    /// Resolve an existing regular file below this root.
    ///
    /// Absolute paths, empty paths, traversal, and symlink escapes are rejected.
    pub fn resolve_file(&self, relative: impl AsRef<Path>) -> Result<PathBuf, FixtureError> {
        let relative = relative.as_ref();
        validate_relative_fixture_path(relative)?;
        let joined = self.canonical.join(relative);
        let resolved = fs::canonicalize(&joined).map_err(|source| FixtureError::Io {
            path: joined,
            source: source.kind(),
        })?;
        if !resolved.starts_with(&self.canonical) {
            return Err(FixtureError::EscapesRoot(relative.to_path_buf()));
        }
        if !resolved.is_file() {
            return Err(FixtureError::NotFile(resolved));
        }
        Ok(resolved)
    }
}

/// Invalid fixture-root or fixture-path input.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FixtureError {
    /// Filesystem operation failed; the stable I/O category is retained.
    Io {
        /// Path involved in the failure.
        path: PathBuf,
        /// Stable I/O error category.
        source: std::io::ErrorKind,
    },
    /// Fixture root exists but is not a directory.
    NotDirectory(PathBuf),
    /// Resolved fixture exists but is not a regular file.
    NotFile(PathBuf),
    /// Fixture path is empty, absolute, or contains a non-normal component.
    InvalidRelativePath(PathBuf),
    /// Canonical target escapes the fixture root, for example through a symlink.
    EscapesRoot(PathBuf),
}

impl fmt::Display for FixtureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(
                    formatter,
                    "fixture I/O error ({source:?}) at {}",
                    path.display()
                )
            }
            Self::NotDirectory(path) => {
                write!(
                    formatter,
                    "fixture root is not a directory: {}",
                    path.display()
                )
            }
            Self::NotFile(path) => write!(formatter, "fixture is not a file: {}", path.display()),
            Self::InvalidRelativePath(path) => {
                write!(
                    formatter,
                    "invalid relative fixture path: {}",
                    path.display()
                )
            }
            Self::EscapesRoot(path) => {
                write!(
                    formatter,
                    "fixture path escapes its root: {}",
                    path.display()
                )
            }
        }
    }
}

impl Error for FixtureError {}

fn validate_key(key: &str) -> Result<(), VectorError> {
    if key.is_empty() || key.contains(['=', '\n', '\r']) {
        return Err(VectorError::InvalidKey(key.to_owned()));
    }
    Ok(())
}

fn validate_relative_fixture_path(path: &Path) -> Result<(), FixtureError> {
    if path.as_os_str().is_empty()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(FixtureError::InvalidRelativePath(path.to_path_buf()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TempFixture {
        path: PathBuf,
    }

    impl TempFixture {
        fn create() -> Self {
            let serial = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("audiveris-testkit-{}-{serial}", std::process::id()));
            fs::create_dir(&path).unwrap();
            Self { path }
        }
    }

    impl Drop for TempFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    fn vectors(entries: &[(&str, &str)]) -> CanonicalVectors {
        let mut vectors = CanonicalVectors::new();
        for (key, value) in entries {
            vectors.insert(*key, *value).unwrap();
        }
        vectors
    }

    #[test]
    fn renders_vectors_in_sorted_order() {
        let vectors = vectors(&[("scale.z", "9"), ("binary.a", "01"), ("load.m", "4x5")]);
        assert_eq!(vectors.render(), "binary.a=01\nload.m=4x5\nscale.z=9\n");
        assert_eq!(vectors.len(), 3);
        assert_eq!(vectors.get("load.m"), Some("4x5"));
        assert_eq!(
            vectors.iter().map(|(key, _)| key).collect::<Vec<_>>(),
            ["binary.a", "load.m", "scale.z"]
        );
    }

    #[test]
    fn rejects_ambiguous_or_duplicate_vectors() {
        let mut vectors = CanonicalVectors::new();
        assert_eq!(
            vectors.insert("", "value").unwrap_err(),
            VectorError::InvalidKey(String::new())
        );
        assert_eq!(
            vectors.insert("bad=key", "value").unwrap_err(),
            VectorError::InvalidKey("bad=key".to_owned())
        );
        assert_eq!(
            vectors.insert("good", "two\nlines").unwrap_err(),
            VectorError::MultilineValue("good".to_owned())
        );
        vectors.insert("unique", "first").unwrap();
        assert_eq!(
            vectors.insert("unique", "second").unwrap_err(),
            VectorError::DuplicateKey("unique".to_owned())
        );
    }

    #[test]
    fn reports_earliest_value_missing_and_unexpected_differences() {
        let expected = vectors(&[("a", "same"), ("b", "expected"), ("d", "last")]);
        let mismatch = vectors(&[("a", "same"), ("b", "actual"), ("d", "last")]);
        assert_eq!(
            expected.first_difference(&mismatch),
            Some(VectorDifference::ValueMismatch {
                key: "b".to_owned(),
                expected: "expected".to_owned(),
                actual: "actual".to_owned(),
            })
        );
        assert_eq!(
            expected.first_difference(&vectors(&[("a", "same"), ("d", "last")])),
            Some(VectorDifference::MissingActual {
                key: "b".to_owned(),
                expected: "expected".to_owned(),
            })
        );
        assert_eq!(
            vectors(&[("a", "same"), ("d", "last")]).first_difference(&expected),
            Some(VectorDifference::UnexpectedActual {
                key: "b".to_owned(),
                actual: "expected".to_owned(),
            })
        );
        assert_eq!(expected.first_difference(&expected), None);
    }

    #[test]
    fn resolves_existing_files_inside_canonical_root() {
        let fixture = TempFixture::create();
        fs::create_dir(fixture.path.join("nested")).unwrap();
        fs::write(fixture.path.join("nested/vector.txt"), b"x=1\n").unwrap();

        let root = FixtureRoot::open(&fixture.path).unwrap();
        let resolved = root.resolve_file("nested/vector.txt").unwrap();
        assert_eq!(
            resolved,
            fs::canonicalize(fixture.path.join("nested/vector.txt")).unwrap()
        );
        assert_eq!(root.path(), fs::canonicalize(&fixture.path).unwrap());
    }

    #[test]
    fn rejects_non_normal_and_non_file_fixture_paths() {
        let fixture = TempFixture::create();
        fs::create_dir(fixture.path.join("directory")).unwrap();
        let root = FixtureRoot::open(&fixture.path).unwrap();

        for invalid in ["", "../escape", "directory/../escape"] {
            assert_eq!(
                root.resolve_file(invalid).unwrap_err(),
                FixtureError::InvalidRelativePath(PathBuf::from(invalid))
            );
        }
        assert!(matches!(
            root.resolve_file("directory").unwrap_err(),
            FixtureError::NotFile(_)
        ));
        assert!(matches!(
            root.resolve_file("missing").unwrap_err(),
            FixtureError::Io {
                source: std::io::ErrorKind::NotFound,
                ..
            }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        use std::os::unix::fs::symlink;

        let fixture = TempFixture::create();
        let outside = TempFixture::create();
        fs::write(outside.path.join("secret"), b"not a fixture").unwrap();
        symlink(&outside.path, fixture.path.join("outside-link")).unwrap();

        let root = FixtureRoot::open(&fixture.path).unwrap();
        assert_eq!(
            root.resolve_file("outside-link/secret").unwrap_err(),
            FixtureError::EscapesRoot(PathBuf::from("outside-link/secret"))
        );
    }
}
