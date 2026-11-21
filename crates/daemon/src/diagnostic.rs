//! Warnings for the owner, written to the daemon's standard error. The `log` facade is
//! compiled out (`max_level_off`), so a warning sent through it would never be seen.

pub(crate) fn warn(message: std::fmt::Arguments<'_>) {
    #[cfg(test)]
    SEEN.with(|seen| seen.borrow_mut().push(message.to_string()));
    eprintln!("ads-daemon: {message}");
}

#[cfg(test)]
thread_local! {
    /// What this thread warned, for the tests.
    pub(crate) static SEEN: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(crate) fn seen() -> Vec<String> {
    SEEN.with(|seen| seen.borrow().clone())
}
