//! Re-exports of [`steam_language`] under their previous paths.

pub use steam_language::emsg::EMsgError;

pub mod enums {
    pub use steam_language::emsg::EMsg;
    pub use steam_language::enums::*;
}

pub mod headers {
    pub use steam_language::headers::*;
}

pub mod messages {
    pub use steam_language::messages::*;
}
