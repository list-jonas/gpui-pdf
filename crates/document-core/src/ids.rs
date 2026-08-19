use std::num::NonZeroU64;

macro_rules! stable_id {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(NonZeroU64);

        impl $name {
            pub const fn new(value: NonZeroU64) -> Self {
                Self(value)
            }

            pub const fn get(self) -> u64 {
                self.0.get()
            }
        }

        impl From<NonZeroU64> for $name {
            fn from(value: NonZeroU64) -> Self {
                Self::new(value)
            }
        }
    };
}

stable_id!(DocumentId);
stable_id!(PageId);
stable_id!(RevisionId);
