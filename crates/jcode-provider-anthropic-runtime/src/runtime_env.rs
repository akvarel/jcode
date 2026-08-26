pub(super) fn nonempty_env(name: &str) -> Option<String> {
    match std::env::var(name) {
        Ok(value) if !value.trim().is_empty() => Some(value),
        Ok(_) | Err(_) => None,
    }
}

pub(super) fn optional_u32(name: &str) -> Option<u32> {
    let value = nonempty_env(name)?;
    match value.trim().parse() {
        Ok(value) => Some(value),
        Err(error) => {
            jcode_base::logging::warn(&format!("Ignoring invalid {name}: {error}"));
            None
        }
    }
}
