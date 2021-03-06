mod matcher;

use std::convert::{TryFrom, TryInto};
use std::fs::ReadDir;
use std::io::Result as IoResult;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;

use crate::config::Sorter;

pub use self::matcher::{Error as MatcherError, Matcher};
pub(crate) use self::matcher::MatcherRepr;

enum FileOrDir {
    File,
    Dir,
}

/// A type that represents included and excluded item files and directories.
#[derive(Debug)]
pub struct Selection {
    include_files: Matcher,
    exclude_files: Matcher,
    include_dirs: Matcher,
    exclude_dirs: Matcher,
}

impl Default for Selection {
    fn default() -> Self {
        let include_files = Matcher::any();
        let exclude_files = Matcher::empty();
        let include_dirs = Matcher::any();
        let exclude_dirs = Matcher::empty();

        Self::new(include_files, exclude_files, include_dirs, exclude_dirs)
    }
}

impl Selection {
    pub fn new(
        include_files: Matcher,
        exclude_files: Matcher,
        include_dirs: Matcher,
        exclude_dirs: Matcher,
    ) -> Self {
        Self {
            include_files,
            exclude_files,
            include_dirs,
            exclude_dirs,
        }
    }

    pub fn from_patterns<'a, IA, SA, IB, SB, IC, SC, ID, SD>(
        include_file_patterns: IA,
        exclude_file_patterns: IB,
        include_dir_patterns: IC,
        exclude_dir_patterns: ID,
    ) -> Result<Self, MatcherError>
    where
        IA: IntoIterator<Item = &'a SA>,
        SA: AsRef<str> + 'a,
        IB: IntoIterator<Item = &'a SB>,
        SB: AsRef<str> + 'a,
        IC: IntoIterator<Item = &'a SC>,
        SC: AsRef<str> + 'a,
        ID: IntoIterator<Item = &'a SD>,
        SD: AsRef<str> + 'a,
    {
        let include_files = Matcher::build(include_file_patterns)?;
        let exclude_files = Matcher::build(exclude_file_patterns)?;
        let include_dirs = Matcher::build(include_dir_patterns)?;
        let exclude_dirs = Matcher::build(exclude_dir_patterns)?;

        Ok(Self::new(
            include_files,
            exclude_files,
            include_dirs,
            exclude_dirs,
        ))
    }

    fn is_pattern_match<P: AsRef<Path>>(&self, path: &P, fod: FileOrDir) -> bool {
        let (inc, exc) = match fod {
            FileOrDir::File => (&self.include_files, &self.exclude_files),
            FileOrDir::Dir => (&self.include_dirs, &self.exclude_dirs),
        };

        inc.is_match(&path) && !exc.is_match(&path)
    }

    /// Returns true if the path matches according to the file matcher.
    /// In order to be a pattern match, the path must match the include filter,
    /// and must NOT match the exclude filter.
    /// Note that this method assumes the path is a file, and uses only the
    /// lexical content of the path; it does not access the filesystem.
    pub fn is_file_pattern_match<P: AsRef<Path>>(&self, path: &P) -> bool {
        self.is_pattern_match(path, FileOrDir::File)
    }

    /// Returns true if the path matches according to the directory matcher.
    /// In order to be a pattern match, the path must match the include filter,
    /// and must NOT match the exclude filter.
    /// Note that this method assumes the path is a directory, and uses only the
    /// lexical content of the path; it does not access the filesystem.
    pub fn is_dir_pattern_match<P: AsRef<Path>>(&self, path: &P) -> bool {
        self.is_pattern_match(path, FileOrDir::Dir)
    }

    /// Returns true if a path is selected.
    /// This accesses the filesystem to tell if the path is a file or directory.
    pub fn is_selected<P: AsRef<Path>>(&self, path: &P) -> IoResult<bool> {
        let file_info = std::fs::metadata(&path)?;

        Ok(if file_info.is_file() {
            self.is_file_pattern_match(path)
        } else if file_info.is_dir() {
            self.is_dir_pattern_match(path)
        } else {
            false
        })
    }

    /// Selects paths inside a directory that match this `Selection`.
    // NOTE: This returns two "levels" of `Error`, a top-level one for any error
    //       relating to accessing the passed-in directory path, and a `Vec` of
    //       `Result`s for errors encountered when iterating over sub-paths.
    pub fn select_in_dir(&self, dir_path: &Path) -> IoResult<SelectedSubPaths> {
        // Try to open the path as a directory, handle the error as appropriate.
        let dir_reader = dir_path.read_dir()?;

        Ok(SelectedSubPaths(dir_reader, &self))
    }

    /// Selects paths inside a directory that match this `Selection`, and sorts them.
    pub fn select_in_dir_sorted(
        &self,
        dir_path: &Path,
        sorter: &Sorter,
    ) -> IoResult<Vec<IoResult<PathBuf>>> {
        let mut res_paths = self.select_in_dir(dir_path)?.collect::<Vec<_>>();

        sorter.sort_path_results(&mut res_paths);

        Ok(res_paths)
    }
}

#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct SelectionRepr {
    pub exclude_sources: bool,
    pub include_files: MatcherRepr,
    pub exclude_files: MatcherRepr,
    pub include_dirs: MatcherRepr,
    pub exclude_dirs: MatcherRepr,
}

impl Default for SelectionRepr {
    fn default() -> Self {
        Self {
            exclude_sources: true,
            include_files: MatcherRepr::Any,
            exclude_files: MatcherRepr::Empty,
            include_dirs: MatcherRepr::Any,
            exclude_dirs: MatcherRepr::Empty,
        }
    }
}

impl TryFrom<SelectionRepr> for Selection {
    type Error = MatcherError;

    fn try_from(value: SelectionRepr) -> Result<Self, Self::Error> {
        Ok(Self {
            include_files: value.include_files.try_into()?,
            exclude_files: value.exclude_files.try_into()?,
            include_dirs: value.include_dirs.try_into()?,
            exclude_dirs: value.exclude_dirs.try_into()?,
        })
    }
}

pub struct SelectedSubPaths<'a>(ReadDir, &'a Selection);

impl<'a> Iterator for SelectedSubPaths<'a> {
    type Item = IoResult<PathBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        // LEARN: Unable to inline these, had to use `let`, why is that?
        let read_dir = &mut self.0;
        let selection = &self.1;

        // Get next entry from the directory reader.
        read_dir.find_map(|res| match res {
            Ok(dir_entry) => {
                let sub_path = dir_entry.path();
                match selection.is_selected(&sub_path) {
                    Ok(true) => Some(Ok(sub_path)),
                    Ok(false) => None,
                    Err(err) => Some(Err(err)),
                }
            }
            Err(err) => Some(Err(err)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use maplit::hashset;

    use crate::config::Sorter;
    use crate::test_util::TestUtil;

    const SAMPLE_FILE_NAMES: &[&str] = &[
        "music.flac",
        "music.wav",
        "music.aac",
        "music.mp3",
        "music.ogg",
        "item",
        "self",
        "item.yml",
        "self.yml",
        "item.flac",
        "self.flac",
    ];

    #[test]
    fn default() {
        let selection = Selection::default();

        assert!(selection.is_file_pattern_match(&"all"));
        assert!(selection.is_file_pattern_match(&"files"));
        assert!(selection.is_file_pattern_match(&"should"));
        assert!(selection.is_file_pattern_match(&"pass"));

        assert!(selection.is_dir_pattern_match(&"all"));
        assert!(selection.is_dir_pattern_match(&"dirs"));
        assert!(selection.is_dir_pattern_match(&"should"));
        assert!(selection.is_dir_pattern_match(&"pass"));
    }

    #[test]
    fn deserialization() {
        // A single pattern for each of include and exclude.
        let text = r#"
            include_files = "*.flac"
            exclude_files = "*.mp3"
        "#;
        let selection_repr: SelectionRepr = toml::from_str(&text).unwrap();
        let selection: Selection = selection_repr.try_into().unwrap();

        assert_eq!(selection.is_file_pattern_match(&"path/to/music.flac"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mp3"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.wav"), false);

        // Multiple patterns for each of include and exclude.
        let text = r#"
            include_files = ["*.flac", "*.wav"]
            exclude_files = ["*.mp3", "*.ogg"]
        "#;
        let selection_repr: SelectionRepr = toml::from_str(&text).unwrap();
        let selection: Selection = selection_repr.try_into().unwrap();

        assert_eq!(selection.is_file_pattern_match(&"path/to/music.flac"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.wav"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mp3"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.ogg"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.aac"), false);

        // Using a default value for missing include patterns.
        let text = r#"
            exclude_files = ["*.mp3", "*.ogg"]
        "#;
        let selection_repr: SelectionRepr = toml::from_str(&text).unwrap();
        let selection: Selection = selection_repr.try_into().unwrap();

        assert_eq!(selection.is_file_pattern_match(&"path/to/music.flac"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.wav"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.aac"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mpc"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mp3"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.ogg"), false);

        // Using a default value for missing exclude patterns.
        let text = r#"
            include_files = ["*.flac", "*.wav"]
        "#;
        let selection_repr: SelectionRepr = toml::from_str(&text).unwrap();
        let selection: Selection = selection_repr.try_into().unwrap();

        assert_eq!(selection.is_file_pattern_match(&"path/to/music.flac"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.wav"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.aac"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mpc"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mp3"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.ogg"), false);
    }

    #[test]
    fn is_pattern_match() {
        let selection = Selection::new(
            Matcher::build(&["*.flac"]).unwrap(),
            Matcher::build(&["*.mp3"]).unwrap(),
            Matcher::any(),
            Matcher::empty(),
        );

        assert_eq!(selection.is_file_pattern_match(&"path/to/music.flac"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mp3"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.wav"), false);

        let selection = Selection::new(
            Matcher::build(&["*.flac", "*.wav"]).unwrap(),
            Matcher::build(&["*.mp3", "*.ogg"]).unwrap(),
            Matcher::any(),
            Matcher::empty(),
        );

        assert_eq!(selection.is_file_pattern_match(&"path/to/music.flac"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.wav"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mp3"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.ogg"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.aac"), false);

        let selection = Selection::new(
            Matcher::any(),
            Matcher::build(&["*.mp3", "*.ogg"]).unwrap(),
            Matcher::any(),
            Matcher::empty(),
        );

        assert_eq!(selection.is_file_pattern_match(&"path/to/music.flac"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.wav"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.aac"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mpc"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mp3"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.ogg"), false);

        let selection = Selection::new(
            Matcher::build(&["*.flac", "*.wav"]).unwrap(),
            Matcher::build(&["item*", "self*"]).unwrap(),
            Matcher::any(),
            Matcher::empty(),
        );

        assert_eq!(selection.is_file_pattern_match(&"path/to/music.flac"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.wav"), true);
        assert_eq!(selection.is_file_pattern_match(&"path/to/item.flac"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/self.flac"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.aac"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mpc"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.mp3"), false);
        assert_eq!(selection.is_file_pattern_match(&"path/to/music.ogg"), false);
    }

    #[test]
    fn select_in_dir() {
        let temp_dir = TestUtil::create_simple_dir("select_in_dir", SAMPLE_FILE_NAMES);
        let path = temp_dir.path();

        let selection = Selection::new(
            Matcher::build(&["music*"]).unwrap(),
            Matcher::build(&["*.mp3", "*.ogg", "*.aac"]).unwrap(),
            Matcher::any(),
            Matcher::empty(),
        );
        let expected = hashset![path.join("music.flac"), path.join("music.wav"),];
        let produced = selection
            .select_in_dir(&path)
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(expected, produced);

        let selection = Selection::new(
            Matcher::build(&["*.flac"]).unwrap(),
            Matcher::empty(),
            Matcher::any(),
            Matcher::empty(),
        );
        let expected = hashset![
            path.join("music.flac"),
            path.join("item.flac"),
            path.join("self.flac"),
        ];
        let produced = selection
            .select_in_dir(&path)
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(expected, produced);

        let selection = Selection::new(
            Matcher::build(&["music*"]).unwrap(),
            Matcher::empty(),
            Matcher::any(),
            Matcher::empty(),
        );
        let expected = hashset![
            path.join("music.flac"),
            path.join("music.wav"),
            path.join("music.aac"),
            path.join("music.mp3"),
            path.join("music.ogg"),
        ];
        let produced = selection
            .select_in_dir(&path)
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(expected, produced);

        let selection = Selection::new(
            Matcher::build(&["item.*", "self.*"]).unwrap(),
            Matcher::build(&["*.flac"]).unwrap(),
            Matcher::any(),
            Matcher::empty(),
        );
        let expected = hashset![path.join("item.yml"), path.join("self.yml"),];
        let produced = selection
            .select_in_dir(&path)
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(expected, produced);
    }

    #[test]
    fn select_in_dir_sorted() {
        let temp_dir = TestUtil::create_simple_dir("select_in_dir_sorted", SAMPLE_FILE_NAMES);
        let path = temp_dir.path();
        let sorter = Sorter::default();

        let selection = Selection::new(
            Matcher::build(&["music*"]).unwrap(),
            Matcher::build(&["*.mp3", "*.ogg", "*.aac"]).unwrap(),
            Matcher::any(),
            Matcher::empty(),
        );
        let expected = vec![path.join("music.flac"), path.join("music.wav")];
        let produced = selection
            .select_in_dir_sorted(&path, &sorter)
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert_eq!(expected, produced);

        let selection = Selection::new(
            Matcher::build(&["*.flac"]).unwrap(),
            Matcher::empty(),
            Matcher::any(),
            Matcher::empty(),
        );
        let expected = vec![
            path.join("item.flac"),
            path.join("music.flac"),
            path.join("self.flac"),
        ];
        let produced = selection
            .select_in_dir_sorted(&path, &sorter)
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert_eq!(expected, produced);

        let selection = Selection::new(
            Matcher::build(&["music*"]).unwrap(),
            Matcher::empty(),
            Matcher::any(),
            Matcher::empty(),
        );
        let expected = vec![
            path.join("music.aac"),
            path.join("music.flac"),
            path.join("music.mp3"),
            path.join("music.ogg"),
            path.join("music.wav"),
        ];
        let produced = selection
            .select_in_dir_sorted(&path, &sorter)
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert_eq!(expected, produced);

        let selection = Selection::new(
            Matcher::build(&["item.*", "self.*"]).unwrap(),
            Matcher::build(&["*.flac"]).unwrap(),
            Matcher::any(),
            Matcher::empty(),
        );
        let expected = vec![path.join("item.yml"), path.join("self.yml")];
        let produced = selection
            .select_in_dir_sorted(&path, &sorter)
            .unwrap()
            .into_iter()
            .map(Result::unwrap)
            .collect::<Vec<_>>();
        assert_eq!(expected, produced);
    }
}
