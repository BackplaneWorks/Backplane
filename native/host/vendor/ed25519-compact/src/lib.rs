//! A compact Ed25519 and X25519 implementation for Rust.
//!
//! * Formally-verified Curve25519 field arithmetic
//! * `no_std`-friendly
//! * WebAssembly-friendly
//! * Fastly Compute-friendly
//! * Lightweight
//! * Zero dependencies if randomness is provided by the application
//! * Only one portable dependency (`getrandom`) if not
//! * Supports incremental signatures (streaming API)
//! * Safe and simple Rust interface.
//!
//! Example usage:
//!
//! ```rust
//! # #[cfg(feature = "random")] {
//! use ed25519_compact::*;
//!
//! // A message to sign and verify.
//! let message = b"test";
//!
//! // Generates a new key pair using a random seed.
//! // A given seed will always produce the same key pair.
//! let key_pair = KeyPair::from_seed(Seed::generate());
//!
//! // Computes a signature for this message using the secret part of the key pair.
//! let signature = key_pair.sk.sign(message, Some(Noise::generate()));
//!
//! // Verifies the signature using the public part of the key pair.
//! key_pair
//!     .pk
//!     .verify(message, &signature)
//!     .expect("Signature didn't verify");
//!
//! // Verification of a different message using the same signature and public key fails.
//! key_pair
//!     .pk
//!     .verify(b"A different message", &signature)
//!     .expect_err("Signature shouldn't verify");
//!
//! // All these structures can be viewed as raw bytes simply by dereferencing them:
//! let signature_as_bytes: &[u8] = signature.as_ref();
//! println!("Signature as bytes: {:?}", signature_as_bytes);
//! # }
//! ```
//!
//! ## Incremental API example usage
//!
//! Messages can also be supplied as multiple parts (streaming API) in order to
//! handle large messages without using much memory:
//!
//! ```rust
//! # #[cfg(feature = "random")] {
//! use ed25519_compact::*;
//!
//! /// Creates a new key pair.
//! let kp = KeyPair::generate();
//!
//! /// Create a state for an incremental signer.
//! let mut st = kp.sk.sign_incremental(Noise::default());
//!
//! /// Feed the message as any number of chunks, and sign the concatenation.
//! st.absorb("mes");
//! st.absorb("sage");
//! let signature = st.sign();
//!
//! /// Create a state for an incremental verifier.
//! let mut st = kp.pk.verify_incremental(&signature).unwrap();
//!
//! /// Feed the message as any number of chunks, and verify the concatenation.
//! st.absorb("mess");
//! st.absorb("age");
//! assert!(st.verify().is_ok());
//! # }
//! ```
//!
//! Cargo features:
//!
//! * `self-verify`: after having computed a new signature, verify that is it
//!   valid. This is slower, but improves resilience against fault attacks. It
//!   is enabled by default on WebAssembly targets.
//! * `std`: disables `no_std` compatibility in order to make errors implement
//!   the standard `Error` trait.
//! * `random` (enabled by default): adds `Default` and `generate`
//!   implementations to the `Seed` and `Noise` objects, in order to securely
//!   create random keys and noise.
//! * `traits`: add support for the traits from the ed25519 and signature
//!   crates.
//! * `pem`: add support for importing/exporting keys as OpenSSL-compatible PEM
//!   files.
//! * `blind-keys`: add support for key blinding.
//! * `opt_size`: Enable size optimizations (based on benchmarks, 8-15% size
//!   reduction at the cost of 6.5-7% performance).
//! * `x25519`: Enable support for the X25519 key exchange system.
//! * `disable-signatures`: Disable support for signatures, and only compile
//!   support for X25519.

#![cfg_attr(not(feature = "std"), no_std)]
#![allow(
    clippy::needless_range_loop,
    clippy::many_single_char_names,
    clippy::unreadable_literal,
    clippy::let_and_return,
    clippy::needless_lifetimes,
    clippy::cast_lossless,
    clippy::suspicious_arithmetic_impl,
    clippy::identity_op
)]

mod common;
mod error;
mod field25519;
pub mod sha512;

pub use crate::common::*;
pub use crate::error::*;

#[cfg(not(feature = "disable-signatures"))]
mod ed25519;
#[cfg(not(feature = "disable-signatures"))]
mod edwards25519;

#[cfg(not(feature = "disable-signatures"))]
pub use crate::ed25519::*;

#[cfg(feature = "x25519")]
pub mod x25519;

#[cfg(all(
    feature = "pem",
    any(not(feature = "disable-signatures"), feature = "x25519")
))]
mod pem;

pub mod reexports {
    pub use crate::sha512;

    #[cfg(feature = "pem")]
    pub use ct_codecs;

    #[cfg(feature = "random")]
    pub use getrandom;

    #[cfg(feature = "traits")]
    pub use ed25519;
}
