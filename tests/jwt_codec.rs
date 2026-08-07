#![cfg(feature = "jwt")]
#![allow(dead_code)]

mod jwt {
    mod algorithm {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/jwt/algorithm.rs"));
    }
    mod claims {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/jwt/claims.rs"));
    }
    mod clock {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/jwt/clock.rs"));
    }
    mod codec {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/jwt/codec.rs"));
    }
    mod config {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/jwt/config.rs"));
    }
    mod error {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/jwt/error.rs"));
    }
    mod header {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/jwt/header.rs"));
    }
    mod key {
        include!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/jwt/key.rs"));
    }

    pub use algorithm::JwtAlgorithm;
    pub use config::{JwtConfig, JwtValidation};
    pub use error::JwtError;
    pub use key::{JwtSigningKey, JwtVerificationKey};

    pub(crate) use algorithm::{EcCurve, KeyFamily};
    pub(crate) use codec::JwtCodec;
    pub(crate) use config::JwtConfigParts;

    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/support/jwt_codec_tests.rs"
    ));
}
