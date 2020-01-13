//! Represents a method of determining whether a potential item path is to be
//! included in metadata lookup.

use std::path::Path;
use std::convert::TryFrom;

use globset::Glob;
use globset::GlobSet;
use globset::GlobSetBuilder;
use globset::Error as GlobError;

#[derive(Debug)]
pub enum Error {
    InvalidPattern(GlobError),
    BuildFailure(GlobError),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::InvalidPattern(ref err) => write!(f, "invalid pattern: {}", err),
            Self::BuildFailure(ref err) => write!(f, "cannot build matcher: {}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidPattern(ref err) => Some(err),
            Self::BuildFailure(ref err) => Some(err),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrManyPatterns {
    One(String),
    Many(Vec<String>),
}

impl TryFrom<OneOrManyPatterns> for Matcher {
    type Error = Error;

    fn try_from(oom: OneOrManyPatterns) -> Result<Self, Self::Error> {
        match oom {
            OneOrManyPatterns::One(p) => Self::build(&[p]),
            OneOrManyPatterns::Many(ps) => Self::build(&ps),
        }
    }
}

/// Filter for file paths that uses zero or more glob patterns to perform matching.
#[derive(Debug, Deserialize)]
#[serde(try_from = "OneOrManyPatterns")]
pub struct Matcher(GlobSet);

impl Matcher {
    /// Attempts to build a matcher out of an iterable of string-likes.
    pub fn build<II, S>(pattern_strs: II) -> Result<Self, Error>
    where
        II: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut builder = GlobSetBuilder::new();

        for pattern_str in pattern_strs.into_iter() {
            let pattern_str = pattern_str.as_ref();
            let pattern = Glob::new(&pattern_str).map_err(Error::InvalidPattern)?;
            builder.add(pattern);
        }

        let matcher = builder.build().map_err(Error::BuildFailure)?;

        Ok(Self(matcher))
    }

    /// Matches a path based on its file name. If the path does not have a file
    /// name (e.g. '/' on Unix systems), returns `false`.
    pub fn is_match<P: AsRef<Path>>(&self, path: P) -> bool {
        // Matching on only file name is needed for patterns such as "self*".
        path.as_ref().file_name().map(|f| self.0.is_match(f)).unwrap_or(false)
    }

    /// Returns a matcher that matches any path that has a file name.
    pub fn any() -> Self {
        // Assume that this is a universal pattern, and will not fail.
        Self::build(&["*"]).unwrap()
    }

    /// Returns a matcher that matches no paths.
    pub fn empty() -> Self {
        Self(GlobSet::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialization() {
        let text = "'*.flac'";
        let matcher: Matcher = serde_yaml::from_str(&text).unwrap();

        assert_eq!(matcher.is_match("music.flac"), true);
        assert_eq!(matcher.is_match("music.mp3"), false);
        assert_eq!(matcher.is_match("photo.png"), false);

        let text = "- '*.flac'\n- '*.mp3'";
        let matcher: Matcher = serde_yaml::from_str(&text).unwrap();

        assert_eq!(matcher.is_match("music.flac"), true);
        assert_eq!(matcher.is_match("music.mp3"), true);
        assert_eq!(matcher.is_match("photo.png"), false);
    }

    #[test]
    fn test_build() {
        // Positive test cases.
        assert!(Matcher::build(&["*"]).is_ok());
        assert!(Matcher::build(&["*.a", "*.b"]).is_ok());
        assert!(Matcher::build(&["?.a", "?.b"]).is_ok());
        assert!(Matcher::build(&["*.a"]).is_ok());
        assert!(Matcher::build(&["**"]).is_ok());
        assert!(Matcher::build(&["a/**/b"]).is_ok());
        assert!(Matcher::build(&[""; 0]).is_ok());
        assert!(Matcher::build(&[""]).is_ok());
        assert!(Matcher::build(&["[a-z]*.a"]).is_ok());
        assert!(Matcher::build(&["**", "[a-z]*.a"]).is_ok());
        assert!(Matcher::build(&["[!abc]"]).is_ok());
        assert!(Matcher::build(&["[*]"]).is_ok());
        assert!(Matcher::build(&["[?]"]).is_ok());
        assert!(Matcher::build(&["{*.a,*.b,*.c}"]).is_ok());

        // Negative test cases.
        // Invalid double star.
        // assert!(Matcher::build(&["a**b"]).is_err());
        // Unclosed character class.
        assert!(Matcher::build(&["[abc"]).is_err());
        // Malformed character range.
        assert!(Matcher::build(&["[z-a]"]).is_err());
        // Unclosed alternates.
        assert!(Matcher::build(&["{*.a,*.b,*.c"]).is_err());
        // Unopened alternates.
        // assert!(Matcher::build(&["*.a,*.b,*.c}"]).is_err());
        // Nested alternates.
        assert!(Matcher::build(&["{*.a,{*.b,*.c}}"]).is_err());
        // Dangling escape.
        assert!(Matcher::build(&["*.a\\"]).is_err());
    }

    #[test]
    fn test_is_match() {
        let matcher = Matcher::build(&["*.a", "*.b"]).unwrap();
        assert_eq!(matcher.is_match("path.a"), true);
        assert_eq!(matcher.is_match("path.b"), true);
        assert_eq!(matcher.is_match("path.c"), false);
        assert_eq!(matcher.is_match("path.ab"), false);
        assert_eq!(matcher.is_match("path"), false);
        assert_eq!(matcher.is_match("extra/path.a"), true);
        assert_eq!(matcher.is_match("extra/path.b"), true);
        assert_eq!(matcher.is_match("extra/path.c"), false);
        assert_eq!(matcher.is_match("/"), false);
        assert_eq!(matcher.is_match(""), false);

        let matcher = Matcher::build(&["*.b"]).unwrap();
        assert_eq!(matcher.is_match("path.a"), false);
        assert_eq!(matcher.is_match("path.b"), true);
        assert_eq!(matcher.is_match("path.c"), false);
        assert_eq!(matcher.is_match("path.ab"), false);
        assert_eq!(matcher.is_match("path"), false);
        assert_eq!(matcher.is_match("extra/path.a"), false);
        assert_eq!(matcher.is_match("extra/path.b"), true);
        assert_eq!(matcher.is_match("extra/path.c"), false);
        assert_eq!(matcher.is_match("/"), false);
        assert_eq!(matcher.is_match(""), false);

        let matcher = Matcher::build(&["*.a", "*.c"]).unwrap();
        assert_eq!(matcher.is_match("path.a"), true);
        assert_eq!(matcher.is_match("path.b"), false);
        assert_eq!(matcher.is_match("path.c"), true);
        assert_eq!(matcher.is_match("path.ab"), false);
        assert_eq!(matcher.is_match("path"), false);
        assert_eq!(matcher.is_match("extra/path.a"), true);
        assert_eq!(matcher.is_match("extra/path.b"), false);
        assert_eq!(matcher.is_match("extra/path.c"), true);
        assert_eq!(matcher.is_match("/"), false);
        assert_eq!(matcher.is_match(""), false);

        let matcher = Matcher::build(&["*"]).unwrap();
        assert_eq!(matcher.is_match("path.a"), true);
        assert_eq!(matcher.is_match("path.b"), true);
        assert_eq!(matcher.is_match("path.c"), true);
        assert_eq!(matcher.is_match("path.ab"), true);
        assert_eq!(matcher.is_match("path"), true);
        assert_eq!(matcher.is_match("extra/path.a"), true);
        assert_eq!(matcher.is_match("extra/path.b"), true);
        assert_eq!(matcher.is_match("extra/path.c"), true);
        assert_eq!(matcher.is_match("/"), false);
        assert_eq!(matcher.is_match(""), false);

        let matcher = Matcher::build(&[] as &[&str]).unwrap();
        assert_eq!(matcher.is_match("path.a"), false);
        assert_eq!(matcher.is_match("path.b"), false);
        assert_eq!(matcher.is_match("path.c"), false);
        assert_eq!(matcher.is_match("path.ab"), false);
        assert_eq!(matcher.is_match("path"), false);
        assert_eq!(matcher.is_match("extra/path.a"), false);
        assert_eq!(matcher.is_match("extra/path.b"), false);
        assert_eq!(matcher.is_match("extra/path.c"), false);
        assert_eq!(matcher.is_match("/"), false);
        assert_eq!(matcher.is_match(""), false);
    }

    #[test]
    fn test_any() {
        let matcher = Matcher::any();
        assert_eq!(matcher.is_match("path"), true);
        assert_eq!(matcher.is_match("path.a"), true);
        assert_eq!(matcher.is_match("path.a.b.c"), true);
        assert_eq!(matcher.is_match("path.ab"), true);
        assert_eq!(matcher.is_match("/extra/path.a"), true);
        assert_eq!(matcher.is_match("extra/path.a"), true);
        assert_eq!(matcher.is_match("/"), false);
        assert_eq!(matcher.is_match(""), false);
    }

    #[test]
    fn test_empty() {
        let matcher = Matcher::empty();
        assert_eq!(matcher.is_match("path"), false);
        assert_eq!(matcher.is_match("path.a"), false);
        assert_eq!(matcher.is_match("path.a.b.c"), false);
        assert_eq!(matcher.is_match("path.ab"), false);
        assert_eq!(matcher.is_match("/extra/path.a"), false);
        assert_eq!(matcher.is_match("extra/path.a"), false);
        assert_eq!(matcher.is_match("/"), false);
        assert_eq!(matcher.is_match(""), false);
    }
}
