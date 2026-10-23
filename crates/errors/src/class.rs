/// Classification shared by every error type in this crate.
///
/// Each domain classifies its variants with an exhaustive `match`, so adding a
/// new variant forces the author to decide whether it is skippable or a client
/// error instead of relying on convention.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ErrorClass {
    skippable: bool,
    client_error: bool,
}

impl ErrorClass {
    pub const NONE: Self = Self {
        skippable: false,
        client_error: false,
    };

    pub const SKIPPABLE: Self = Self {
        skippable: true,
        client_error: false,
    };

    pub const CLIENT_ERROR: Self = Self {
        skippable: false,
        client_error: true,
    };

    pub const fn is_skippable(self) -> bool {
        self.skippable
    }

    pub const fn is_client_error(self) -> bool {
        self.client_error
    }
}
