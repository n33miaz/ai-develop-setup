//! Stable supervisor that starts the other processes and applies updates.

fn banner() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

fn main() {
    println!("{}", banner());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banner_names_the_package() {
        assert!(banner().starts_with("ads-launcher "));
    }
}
