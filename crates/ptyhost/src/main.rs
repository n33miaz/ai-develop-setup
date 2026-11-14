//! Stable process that holds terminal sessions and the replay ring.

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
        assert!(banner().starts_with("ads-ptyhost "));
    }
}
