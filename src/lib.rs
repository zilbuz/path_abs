/* Copyright (c) 2018 Garrett Berg, vitiral@gmail.com
 *
 * Licensed under the Apache License, Version 2.0, <LICENSE-APACHE or
 * http://apache.org/licenses/LICENSE-2.0> or the MIT license <LICENSE-MIT or
 * http://opensource.org/licenses/MIT>, at your option. This file may not be
 * copied, modified, or distributed except according to those terms.
 */
//! Absolute serializable path types and associated methods.
//!
//! This library provides the following types:
//! - [`PathAbs`](struct.PathAbs.html): an absolute (canonicalized) path that is guaranteed (when
//!   created) to exist.
//! - [`PathFile`](struct.PathFile.html): a `PathAbs` that is guaranteed to be a file, with
//!   associated methods.
//! - [`PathDir`](struct.PathDir.html): a `PathAbs` that is guaranteed to be a directory, with
//!   associated methods.
//! - [`PathType`](struct.PathType.html): an enum containing either a file or a directory. Returned
//!   by `PathDir::list`.
//!
//! In addition, all types are serializable through serde (even on windows!) by using the crate
//! [`stfu8`](https://crates.io/crates/stfu8) to encode/decode, allowing ill-formed UTF-16.
//! See that crate for more details on how the resulting encoding can be edited (by hand)
//! even in the case of what *would be* ill-formed UTF-16.
//!
//! Also see the [project repo](https://github.com/vitiral/path_abs) and consider leaving a star!
//!
//! # Examples
//! Recreating `Cargo.init` in `target/example`
//!
//! ```rust
//! # extern crate path_abs;
//! use std::collections::HashSet;
//! use path_abs::{PathAbs, PathDir, PathFile, PathType};
//!
//! # fn main() {
//!
//! let example = "target/example";
//!
//! # let _ = ::std::fs::remove_dir_all(example);
//!
//! // Create your paths
//! let project = PathDir::create_all(example).unwrap();
//! let src = PathDir::create(project.join("src")).unwrap();
//! let lib = PathFile::create(src.join("lib.rs")).unwrap();
//! let cargo = PathFile::create(project.join("Cargo.toml")).unwrap();
//!
//! // Write the templates
//! lib.write_str(r#"
//! #[cfg(test)]
//! mod tests {
//!     #[test]
//!     fn it_works() {
//!         assert_eq!(2 + 2, 4);
//!     }
//! }"#).unwrap();
//!
//! cargo.write_str(r#"
//! [package]
//! name = "example"
//! version = "0.1.0"
//! authors = ["Garrett Berg <googberg@gmail.com>"]
//!
//! [dependencies]
//! "#).unwrap();
//!
//! let mut result = HashSet::new();
//! for p in project.list().unwrap() {
//!     result.insert(p.unwrap());
//! }
//!
//! let mut expected = HashSet::new();
//! expected.insert(PathType::Dir(src));
//! expected.insert(PathType::File(cargo));
//!
//! assert_eq!(expected, result);
//!
//! // Get a file
//! let abs = PathAbs::new("target/example/src/lib.rs").unwrap();
//!
//! // or get the file of known type
//! let file = PathType::new("target/example/src/lib.rs")
//!     .unwrap()
//!     .unwrap_file();
//!
//! // or use `into_file`
//! let file2 = abs.clone().into_file().unwrap();
//!
//! assert!(abs.is_file());
//! assert!(file.is_file());
//! assert!(file2.is_file());
//! # }
//! ```

#[cfg(feature = "serialize")]
extern crate serde;
#[macro_use]
#[cfg(feature = "serialize")]
extern crate serde_derive;
#[cfg(feature = "serialize")]
extern crate stfu8;

#[macro_use]
#[cfg(test)]
extern crate pretty_assertions;
#[cfg(test)]
extern crate serde_json;
#[cfg(test)]
extern crate tempdir;

use std::convert::AsRef;
use std::io;
use std::fmt;
use std::ops::Deref;
use std::path::{Path, PathBuf};

mod dir;
mod file;
#[cfg(feature = "serialize")]
mod ser;
mod ty;

pub use dir::PathDir;
pub use file::PathFile;
pub use ty::PathType;

#[derive(Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
/// An absolute ([canonicalized][1]) path that is guaranteed (when created) to exist.
///
/// [1]: https://doc.rust-lang.org/std/path/struct.Path.html?search=#method.canonicalize
pub struct PathAbs(PathBuf);

impl PathAbs {
    /// Instantiate a new `PathAbs`. The path must exist or `io::Error` will be returned.
    ///
    /// # Examples
    /// ```rust
    /// # extern crate path_abs;
    /// use path_abs::PathAbs;
    ///
    /// # fn main() {
    /// let lib = PathAbs::new("src/lib.rs").unwrap();
    /// # }
    /// ```
    pub fn new<P: AsRef<Path>>(path: P) -> io::Result<PathAbs> {
        Ok(PathAbs(path.as_ref().canonicalize()?))
    }

    /// Resolve the `PathAbs` as a `PathFile`. Return an error if it is not a file.
    pub fn into_file(self) -> io::Result<PathFile> {
        PathFile::from_abs(self)
    }

    /// Resolve the `PathAbs` as a `PathDir`. Return an error if it is not a directory.
    pub fn into_dir(self) -> io::Result<PathDir> {
        PathDir::from_abs(self)
    }

    /// Get the parent directory of this path as a `PathDir`.
    ///
    /// > This does not make additinal syscalls, as the parent by definition must be a directory
    /// > and exist.
    ///
    /// # Examples
    /// ```rust
    /// # extern crate path_abs;
    /// use path_abs::{PathDir, PathFile};
    ///
    /// # fn main() {
    /// let lib = PathFile::new("src/lib.rs").unwrap();
    /// let src = lib.parent_dir().unwrap();
    /// assert_eq!(PathDir::new("src").unwrap(), src);
    /// # }
    /// ```
    pub fn parent_dir(&self) -> Option<PathDir> {
        match self.parent() {
            Some(p) => Some(PathDir(PathAbs(p.to_path_buf()))),
            None => None,
        }
    }

    /// For constructing mocked paths during tests. This is effectively the same as a `PathBuf`.
    ///
    /// This is NOT checked for validity so the file may or may not actually exist and will
    /// NOT be, in any way, an absolute or canonicalized path.
    ///
    /// # Examples
    /// ```rust
    /// # extern crate path_abs;
    /// use path_abs::PathAbs;
    ///
    /// # fn main() {
    /// // this file exist
    /// let lib = PathAbs::new("src/lib.rs").unwrap();
    ///
    /// let lib_mocked = PathAbs::mock("src/lib.rs");
    ///
    /// // in this case, the mocked file exists
    /// assert!(lib_mocked.exists());
    ///
    /// // However, it is NOT equivalent to `lib`
    /// assert_ne!(lib, lib_mocked);
    ///
    /// // this file doesn't exist at all
    /// let dne = PathAbs::mock("src/dne.rs");
    /// assert!(!dne.exists());
    /// # }
    /// ```
    pub fn mock<P: AsRef<Path>>(fake_path: P) -> PathAbs {
        PathAbs(fake_path.as_ref().to_path_buf())
    }
}

impl fmt::Debug for PathAbs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl AsRef<PathBuf> for PathAbs {
    fn as_ref(&self) -> &PathBuf {
        &self.0
    }
}

impl AsRef<Path> for PathAbs {
    fn as_ref(&self) -> &Path {
        self.0.as_path()
    }
}

impl Deref for PathAbs {
    type Target = PathBuf;

    fn deref(&self) -> &PathBuf {
        &self.0
    }
}
