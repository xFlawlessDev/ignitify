use std::sync::Arc;

use ignitify_domain::DomainName;

/// Restricts public routes to operator-owned domain suffixes.
#[derive(Debug, Clone)]
pub struct DomainPolicy {
    allowed_suffixes: Arc<[String]>,
    allow_all: bool,
}

impl DomainPolicy {
    pub fn from_suffixes(values: impl IntoIterator<Item = String>) -> Self {
        let allowed_suffixes = values
            .into_iter()
            .map(|value| value.trim().to_ascii_lowercase())
            .filter_map(|value| DomainName::new(&value).ok().map(|_| value))
            .collect();
        Self {
            allowed_suffixes,
            allow_all: false,
        }
    }

    pub fn allows(&self, hostname: &DomainName) -> bool {
        self.allow_all
            || self.allowed_suffixes.iter().any(|suffix| {
                hostname.as_str() == suffix
                    || hostname
                        .as_str()
                        .strip_suffix(suffix)
                        .is_some_and(|prefix| prefix.ends_with('.'))
            })
    }

    pub(crate) fn restricts_to_operator_suffixes(&self) -> bool {
        !self.allow_all && !self.allowed_suffixes.is_empty()
    }

    pub(crate) fn permissive() -> Self {
        Self {
            allowed_suffixes: Arc::new([]),
            allow_all: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DomainPolicy;
    use ignitify_domain::DomainName;

    #[test]
    fn accepts_only_configured_domain_suffixes() {
        let policy = DomainPolicy::from_suffixes(["apps.example.com".to_owned()]);
        assert!(policy.allows(&DomainName::new("web.apps.example.com").unwrap()));
        assert!(policy.allows(&DomainName::new("apps.example.com").unwrap()));
        assert!(!policy.allows(&DomainName::new("web.example.com").unwrap()));
        assert!(
            !DomainPolicy::from_suffixes(Vec::<String>::new())
                .allows(&DomainName::new("web.apps.example.com").unwrap())
        );
        assert!(
            !DomainPolicy::from_suffixes(Vec::<String>::new()).restricts_to_operator_suffixes()
        );
    }
}
