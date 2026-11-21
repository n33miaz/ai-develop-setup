//! Warnings for the owner, written to the daemon's standard error. The `log` facade is
//! compiled out (`max_level_off`), so a warning sent through it would never be seen.

use std::io::Write;

pub(crate) fn warn(message: std::fmt::Arguments<'_>) {
    #[cfg(test)]
    SEEN.with(|seen| seen.borrow_mut().push(message.to_string()));
    write_warning(&mut std::io::stderr().lock(), message);
}

/// A closed or failing output loses the warning; it never panics, since some callers hold
/// the log lock and a panic would poison it.
fn write_warning(out: &mut impl Write, message: std::fmt::Arguments<'_>) {
    let _ = writeln!(out, "ads-daemon: {message}").and_then(|()| out.flush());
}

#[cfg(test)]
thread_local! {
    /// What this thread warned, for the tests.
    static SEEN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(crate) fn seen() -> Vec<String> {
    SEEN.with(|seen| seen.borrow().clone())
}

#[cfg(test)]
mod tests {
    use super::write_warning;

    #[test]
    fn a_warning_is_one_prefixed_line_and_a_failing_output_does_not_panic() {
        let mut bytes = Vec::new();
        write_warning(
            &mut bytes,
            format_args!("record of {} not readable", "act-1"),
        );
        assert_eq!(bytes, b"ads-daemon: record of act-1 not readable\n");
        struct Broken;
        impl std::io::Write for Broken {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Err(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
            }
        }
        write_warning(&mut Broken, format_args!("lost"));
    }
}
