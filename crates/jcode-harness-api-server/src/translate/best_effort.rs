pub(super) fn option<T, E>(result: Result<T, E>) -> Option<T> {
    result.into_iter().next()
}

pub(super) fn result_or<T, E>(result: Result<T, E>, fallback: T) -> T {
    result.into_iter().next().unwrap_or(fallback)
}

pub(super) fn option_or<T>(value: Option<T>, fallback: T) -> T {
    value.unwrap_or(fallback)
}
