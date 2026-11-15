//! Text-only normalization of absolute paths, Windows and POSIX. No I/O and no `std::path`:
//! a Windows path must be judged the same way on any host.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum PathStyle {
    Windows,
    Posix,
}

/// Why a path was refused. Nothing is repaired: a refused path invalidates the whole plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PathError {
    Empty,
    /// No root: `a/b`, `.\a`, `\a`.
    Relative,
    /// `C:a`: relative to the current folder of the drive.
    DriveRelative,
    /// `\\server\share` or `//server/share`.
    Network,
    /// `\\?\` and `\\.\` prefixes.
    DevicePrefix,
    /// Any `..` component. Resolving it as text is wrong when a link is involved.
    ParentComponent,
    /// `$NAME`, `${NAME}` or `%NAME%`: any `$` or `%`.
    UnexpandedVariable,
    /// A path or a component starting with `~`.
    UnexpandedTilde,
    /// `*` or `?` before the last component.
    WildcardNotLast,
    /// `*` or `?` where only a literal path fits (work root, destination, policy paths).
    WildcardNotAllowed,
    /// Windows 8.3 alias (`NAME~1`): may stand for a protected folder and needs I/O to resolve.
    ShortName,
    /// Windows drops a trailing dot or space, so the text is not the path that would be touched.
    TrailingDotOrSpace,
    /// `:` after the drive (alternate data stream).
    AlternateStream,
    /// `< > " |` on Windows, `\` inside a POSIX component.
    ReservedCharacter,
    /// `NUL`, `CON`, `COM1` and the like on Windows.
    DeviceName,
    ControlCharacter,
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::Empty => "empty path",
            Self::Relative => "relative path",
            Self::DriveRelative => "drive-relative path",
            Self::Network => "network path",
            Self::DevicePrefix => "device path prefix",
            Self::ParentComponent => "`..` component",
            Self::UnexpandedVariable => "unexpanded variable",
            Self::UnexpandedTilde => "unexpanded tilde",
            Self::WildcardNotLast => "wildcard before the last component",
            Self::WildcardNotAllowed => "wildcard where a literal path is required",
            Self::ShortName => "short (8.3) name",
            Self::TrailingDotOrSpace => "component ending in dot or space",
            Self::AlternateStream => "colon inside a component",
            Self::ReservedCharacter => "reserved character",
            Self::DeviceName => "device name",
            Self::ControlCharacter => "control character",
        };
        f.write_str(text)
    }
}

/// An absolute, normalized, literal path. Only [`AbsPath::parse`] builds one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AbsPath {
    style: PathStyle,
    /// Upper-case drive letter; `None` on POSIX.
    drive: Option<char>,
    components: Vec<String>,
}

/// A target of an action: a literal path, or a wildcard pattern over the children of a folder.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TargetPath {
    base: AbsPath,
    pattern: Option<String>,
}

impl AbsPath {
    /// Normalizes `raw`: separators collapsed, `.` dropped, drive letter in upper case. Refuses
    /// everything listed in [`PathError`], wildcards included.
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        let (path, pattern) = parse(raw)?;
        match pattern {
            Some(_) => Err(PathError::WildcardNotAllowed),
            None => Ok(path),
        }
    }

    pub fn style(&self) -> PathStyle {
        self.style
    }

    /// `C:\` or `/`.
    pub fn is_root(&self) -> bool {
        self.components.is_empty()
    }

    pub fn depth(&self) -> usize {
        self.components.len()
    }

    pub(crate) fn components(&self) -> &[String] {
        &self.components
    }

    /// Equal to `base` or inside it, compared component by component (`proj2` is not inside
    /// `proj`). This is the comparison that grants: on Windows only the case of ASCII letters
    /// is ignored, and elsewhere nothing is. Names a wider fold would make equal (`K` and the
    /// Kelvin sign) are different folders, so they are outside.
    pub fn is_within(&self, base: &AbsPath) -> bool {
        match self.style {
            PathStyle::Windows => self.within(base, |a, b| a.eq_ignore_ascii_case(b)),
            PathStyle::Posix => self.within(base, |a, b| a == b),
        }
    }

    pub fn is_strictly_within(&self, base: &AbsPath) -> bool {
        self.depth() > base.depth() && self.is_within(base)
    }

    /// Like [`Self::is_within`], with the widest comparison of names, in both styles. For deny
    /// rules only: some file systems ignore case, and a deny rule must not be dodged by
    /// changing it. Never use it to grant.
    pub(crate) fn is_loosely_within(&self, base: &AbsPath) -> bool {
        self.within(base, same_name_loosely)
    }

    fn within(&self, base: &AbsPath, same_name: fn(&str, &str) -> bool) -> bool {
        self.style == base.style
            && self.drive == base.drive
            && self.components.len() >= base.components.len()
            && self
                .components
                .iter()
                .zip(&base.components)
                .all(|(mine, theirs)| same_name(mine, theirs))
    }

    pub(crate) fn join(&self, names: &[String]) -> AbsPath {
        let mut joined = self.clone();
        joined.components.extend(names.iter().cloned());
        joined
    }
}

/// Whether two names could be the same folder on some file system: equal after either Unicode
/// case mapping. Wider than any real file system on purpose, so it only ever serves to deny.
pub(crate) fn same_name_loosely(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase() || a.to_uppercase() == b.to_uppercase()
}

impl fmt::Display for AbsPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.style {
            PathStyle::Windows => {
                write!(f, "{}:\\", self.drive.unwrap_or('?'))?;
                f.write_str(&self.components.join("\\"))
            }
            PathStyle::Posix => {
                f.write_str("/")?;
                f.write_str(&self.components.join("/"))
            }
        }
    }
}

impl TargetPath {
    /// Like [`AbsPath::parse`], accepting `*` and `?` in the last component.
    pub fn parse(raw: &str) -> Result<Self, PathError> {
        let (base, pattern) = parse(raw)?;
        Ok(Self { base, pattern })
    }

    /// The path the rules judge: the target itself, or the folder a wildcard ranges over
    /// (`folder/*` can remove everything in `folder`, so it is judged as `folder`).
    pub fn scope(&self) -> &AbsPath {
        &self.base
    }

    pub fn pattern(&self) -> Option<&str> {
        self.pattern.as_deref()
    }
}

impl fmt::Display for TargetPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.base)?;
        if let Some(pattern) = &self.pattern {
            if !self.base.is_root() {
                f.write_str(match self.base.style {
                    PathStyle::Windows => "\\",
                    PathStyle::Posix => "/",
                })?;
            }
            f.write_str(pattern)?;
        }
        Ok(())
    }
}

/// A relative sub-path in the shape of `urn:ads:common:relpath:1`: `/` as separator, letters,
/// digits and `. _ -` only, no `.` or `..` component.
pub(crate) fn parse_relative(raw: &str) -> Option<Vec<String>> {
    if raw.is_empty() || raw.len() > 260 {
        return None;
    }
    raw.split('/')
        .map(|name| {
            let allowed = name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'));
            (!name.is_empty() && allowed && !name.ends_with('.')).then(|| name.to_owned())
        })
        .collect()
}

fn is_separator(byte: u8) -> bool {
    byte == b'/' || byte == b'\\'
}

fn parse(raw: &str) -> Result<(AbsPath, Option<String>), PathError> {
    if raw.trim().is_empty() {
        return Err(PathError::Empty);
    }
    if raw.chars().any(char::is_control) {
        return Err(PathError::ControlCharacter);
    }
    if raw.starts_with('~') {
        return Err(PathError::UnexpandedTilde);
    }
    if raw.contains(['$', '%']) {
        return Err(PathError::UnexpandedVariable);
    }

    let bytes = raw.as_bytes();
    let (style, drive, rest) =
        if bytes.len() >= 2 && is_separator(bytes[0]) && is_separator(bytes[1]) {
            let device = bytes.len() >= 3
                && matches!(bytes[2], b'?' | b'.')
                && bytes.get(3).is_none_or(|b| is_separator(*b));
            return Err(if device {
                PathError::DevicePrefix
            } else {
                PathError::Network
            });
        } else if bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' {
            if !bytes.get(2).is_some_and(|b| is_separator(*b)) {
                return Err(PathError::DriveRelative);
            }
            let drive = char::from(bytes[0].to_ascii_uppercase());
            (PathStyle::Windows, Some(drive), &raw[2..])
        } else if bytes[0] == b'/' {
            (PathStyle::Posix, None, raw)
        } else {
            return Err(PathError::Relative);
        };

    let names: Vec<&str> = match style {
        PathStyle::Windows => rest.split(['/', '\\']).collect(),
        PathStyle::Posix => rest.split('/').collect(),
    };
    let names: Vec<&str> = names
        .into_iter()
        .filter(|name| !name.is_empty() && *name != ".")
        .collect();

    let mut components = Vec::with_capacity(names.len());
    let mut pattern = None;
    for (index, name) in names.iter().enumerate() {
        if *name == ".." {
            return Err(PathError::ParentComponent);
        }
        if name.starts_with('~') {
            return Err(PathError::UnexpandedTilde);
        }
        match style {
            PathStyle::Windows => check_windows_name(name)?,
            PathStyle::Posix => {
                if name.contains('\\') {
                    return Err(PathError::ReservedCharacter);
                }
            }
        }
        if name.contains(['*', '?']) {
            if index + 1 != names.len() {
                return Err(PathError::WildcardNotLast);
            }
            pattern = Some((*name).to_owned());
        } else {
            components.push((*name).to_owned());
        }
    }

    Ok((
        AbsPath {
            style,
            drive,
            components,
        },
        pattern,
    ))
}

fn check_windows_name(name: &str) -> Result<(), PathError> {
    if name.ends_with(['.', ' ']) {
        return Err(PathError::TrailingDotOrSpace);
    }
    if name.contains(':') {
        return Err(PathError::AlternateStream);
    }
    if name.contains(['<', '>', '"', '|']) {
        return Err(PathError::ReservedCharacter);
    }
    if is_short_name(name) {
        return Err(PathError::ShortName);
    }
    let stem = name.split('.').next().unwrap_or(name).trim_end();
    let stem = stem.to_ascii_uppercase();
    let numbered = |prefix: &str| {
        stem.strip_prefix(prefix).is_some_and(|rest| {
            rest.len() == 1 && rest != "0" && rest.bytes().all(|b| b.is_ascii_digit())
        })
    };
    if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL") || numbered("COM") || numbered("LPT")
    {
        return Err(PathError::DeviceName);
    }
    Ok(())
}

/// `~` followed by digits up to the end of the name or up to a dot.
fn is_short_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    bytes.iter().enumerate().any(|(at, byte)| {
        if *byte != b'~' {
            return false;
        }
        let digits = bytes[at + 1..]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .count();
        let after = bytes.get(at + 1 + digits);
        digits > 0 && after.is_none_or(|b| *b == b'.')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn abs(raw: &str) -> String {
        AbsPath::parse(raw).unwrap().to_string()
    }

    #[test]
    fn windows_path_is_normalized() {
        assert_eq!(abs("c:/work//proj/./src/"), "C:\\work\\proj\\src");
        assert_eq!(abs("C:\\work\\proj"), "C:\\work\\proj");
        assert_eq!(abs("d:\\"), "D:\\");
        assert!(AbsPath::parse("C:/").unwrap().is_root());
    }

    #[test]
    fn posix_path_is_normalized() {
        assert_eq!(abs("/work//proj/./src/"), "/work/proj/src");
        assert_eq!(abs("/"), "/");
        assert!(AbsPath::parse("/").unwrap().is_root());
    }

    #[test]
    fn relative_paths_are_refused() {
        for raw in ["work/proj", "./work", ".\\work", "\\work", "work", "."] {
            assert_eq!(AbsPath::parse(raw), Err(PathError::Relative), "{raw}");
        }
        assert_eq!(AbsPath::parse("C:work"), Err(PathError::DriveRelative));
        assert_eq!(AbsPath::parse("C:"), Err(PathError::DriveRelative));
        assert_eq!(AbsPath::parse(""), Err(PathError::Empty));
        assert_eq!(AbsPath::parse("  "), Err(PathError::Empty));
    }

    #[test]
    fn any_parent_component_is_refused() {
        for raw in [
            "/work/proj/..",
            "/work/../etc",
            "/..",
            "C:\\work\\proj\\..",
            "C:\\work\\..\\Windows",
            "C:/work/proj/../..",
        ] {
            assert_eq!(
                AbsPath::parse(raw),
                Err(PathError::ParentComponent),
                "{raw}"
            );
        }
    }

    #[test]
    fn unexpanded_variables_and_tildes_are_refused() {
        for raw in [
            "$HOME/proj",
            "/work/$DIR/x",
            "/work/${DIR}/x",
            "%USERPROFILE%\\proj",
            "C:\\work\\%NAME%\\x",
            "/work/100%",
        ] {
            assert_eq!(
                AbsPath::parse(raw),
                Err(PathError::UnexpandedVariable),
                "{raw}"
            );
        }
        for raw in ["~", "~/proj", "~user-a/proj", "/work/~/x", "C:\\work\\~tmp"] {
            assert_eq!(
                AbsPath::parse(raw),
                Err(PathError::UnexpandedTilde),
                "{raw}"
            );
        }
    }

    #[test]
    fn wildcard_is_accepted_only_in_the_last_component_of_a_target() {
        let target = TargetPath::parse("/work/proj/build/*.o").unwrap();
        assert_eq!(target.scope().to_string(), "/work/proj/build");
        assert_eq!(target.pattern(), Some("*.o"));
        assert_eq!(target.to_string(), "/work/proj/build/*.o");

        let target = TargetPath::parse("C:\\work\\proj\\*").unwrap();
        assert_eq!(target.scope().to_string(), "C:\\work\\proj");
        assert_eq!(target.to_string(), "C:\\work\\proj\\*");
        assert_eq!(TargetPath::parse("C:\\*").unwrap().to_string(), "C:\\*");

        for raw in ["/work/*/build", "/work/pro?/x", "C:\\*\\proj"] {
            assert_eq!(
                TargetPath::parse(raw),
                Err(PathError::WildcardNotLast),
                "{raw}"
            );
        }
        assert_eq!(
            AbsPath::parse("/work/proj/*"),
            Err(PathError::WildcardNotAllowed)
        );
    }

    #[test]
    fn network_and_device_paths_are_refused() {
        for raw in [
            "\\\\server\\share\\x",
            "//server/share/x",
            "\\\\server",
            "\\/server/share",
        ] {
            assert_eq!(AbsPath::parse(raw), Err(PathError::Network), "{raw}");
        }
        for raw in ["\\\\?\\C:\\work", "\\\\.\\C:\\work", "//?/C:/work", "\\\\?"] {
            assert_eq!(AbsPath::parse(raw), Err(PathError::DevicePrefix), "{raw}");
        }
    }

    #[test]
    fn windows_aliases_of_another_path_are_refused() {
        assert_eq!(AbsPath::parse("C:\\PROGRA~1\\x"), Err(PathError::ShortName));
        assert_eq!(
            AbsPath::parse("C:\\Users\\USER-A~1.B\\x"),
            Err(PathError::ShortName)
        );
        assert_eq!(
            AbsPath::parse("C:\\Windows.\\x"),
            Err(PathError::TrailingDotOrSpace)
        );
        assert_eq!(
            AbsPath::parse("C:\\Windows \\x"),
            Err(PathError::TrailingDotOrSpace)
        );
        assert_eq!(
            AbsPath::parse("C:\\work\\a.txt:stream"),
            Err(PathError::AlternateStream)
        );
        assert_eq!(AbsPath::parse("C:\\work\\nul"), Err(PathError::DeviceName));
        assert_eq!(
            AbsPath::parse("C:\\work\\COM1.txt"),
            Err(PathError::DeviceName)
        );
        assert_eq!(
            AbsPath::parse("C:\\work\\a|b"),
            Err(PathError::ReservedCharacter)
        );
        // Names that only look like the refused ones.
        assert_eq!(abs("C:\\work\\a~b\\console"), "C:\\work\\a~b\\console");
        assert_eq!(abs("C:\\work\\v~1x"), "C:\\work\\v~1x");
    }

    #[test]
    fn control_characters_and_posix_backslash_are_refused() {
        assert_eq!(
            AbsPath::parse("/work/a\nb"),
            Err(PathError::ControlCharacter)
        );
        assert_eq!(
            AbsPath::parse("C:\\work\\a\0b"),
            Err(PathError::ControlCharacter)
        );
        assert_eq!(
            AbsPath::parse("/work/a\\b"),
            Err(PathError::ReservedCharacter)
        );
    }

    #[test]
    fn containment_is_by_component_and_by_style() {
        let base = AbsPath::parse("C:\\work\\proj").unwrap();
        assert!(
            AbsPath::parse("c:\\WORK\\Proj\\src")
                .unwrap()
                .is_within(&base)
        );
        assert!(base.is_within(&base));
        assert!(!base.is_strictly_within(&base));
        assert!(!AbsPath::parse("C:\\work\\proj2").unwrap().is_within(&base));
        assert!(
            !AbsPath::parse("D:\\work\\proj\\src")
                .unwrap()
                .is_within(&base)
        );
        assert!(!AbsPath::parse("C:\\work").unwrap().is_within(&base));

        let base = AbsPath::parse("/work/proj").unwrap();
        assert!(
            AbsPath::parse("/work/proj/src")
                .unwrap()
                .is_strictly_within(&base)
        );
        assert!(!AbsPath::parse("/work/Proj/src").unwrap().is_within(&base));
        assert!(
            AbsPath::parse("/work/Proj/src")
                .unwrap()
                .is_loosely_within(&base)
        );

        // Only the case of ASCII letters is ignored when granting; the deny side is wider.
        let base = AbsPath::parse("C:\\work\\K\\stra\u{df}e").unwrap();
        let within = |raw: &str| AbsPath::parse(raw).unwrap().is_within(&base);
        let loosely = |raw: &str| AbsPath::parse(raw).unwrap().is_loosely_within(&base);
        assert!(within("c:\\WORK\\k\\STRA\u{df}E\\x"));
        for look_alike in [
            "C:\\work\\\u{212A}\\stra\u{df}e",
            "C:\\work\\K\\STRASSE",
            "C:\\work\\K\\stra\u{1E9E}e",
        ] {
            assert!(!within(look_alike), "{look_alike}");
            assert!(loosely(look_alike), "{look_alike}");
        }
        assert!(!AbsPath::parse("/work/proj2").unwrap().is_within(&base));
        assert!(!AbsPath::parse("C:\\work\\proj").unwrap().is_within(&base));
    }

    #[test]
    fn relative_sub_path_follows_the_contract_shape() {
        assert_eq!(
            parse_relative(".ads/quarantine"),
            Some(vec![".ads".to_owned(), "quarantine".to_owned()])
        );
        for raw in ["", "/abs", "a//b", "a/../b", "..", "a\\b", "a/b.", "a b"] {
            assert_eq!(parse_relative(raw), None, "{raw}");
        }
    }
}
