pub(super) fn active_is_anthropic() -> bool {
    std::env::var("JCODE_NAMED_PROVIDER_PROFILE").is_ok_and(|name| {
        crate::config::config()
            .providers
            .get(&name)
            .is_some_and(|profile| {
                matches!(
                    profile.provider_type,
                    crate::config::NamedProviderType::AnthropicCompatible
                )
            })
    })
}
