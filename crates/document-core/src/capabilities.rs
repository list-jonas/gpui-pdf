#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[must_use]
pub struct EngineCapabilities(u32);

impl EngineCapabilities {
    pub const READ: Self = Self(1 << 0);
    pub const RENDER: Self = Self(1 << 1);
    pub const EXTRACT_TEXT: Self = Self(1 << 2);
    pub const ENCRYPTED_DOCUMENTS: Self = Self(1 << 3);

    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, capability: Self) -> bool {
        self.0 & capability.0 == capability.0
    }
}

#[cfg(test)]
mod tests {
    use super::EngineCapabilities;

    #[test]
    fn capability_sets_compose_without_external_types() {
        let capabilities = EngineCapabilities::READ.union(EngineCapabilities::RENDER);

        assert!(capabilities.contains(EngineCapabilities::READ));
        assert!(capabilities.contains(EngineCapabilities::RENDER));
        assert!(!capabilities.contains(EngineCapabilities::EXTRACT_TEXT));
    }
}
