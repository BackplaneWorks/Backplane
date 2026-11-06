//! OpenSSL-compatible DER and PEM encodings.
//!
//! Public keys use the SubjectPublicKeyInfo structure, and secret keys the
//! minimal PKCS#8 v1 structure produced by OpenSSL: version 0, no algorithm
//! parameters, no attributes and no embedded public key.
//! PKCS#8 v2 (RFC 5958 OneAsymmetricKey with a public key) and other extended
//! forms are rejected.

#[cfg(feature = "std")]
use ct_codecs::Encoder;
use ct_codecs::{Base64, Decoder};

use super::common::Mem;
use super::Error;

const PEM_PRIVATE_BEGIN: &str = "-----BEGIN PRIVATE KEY-----";
const PEM_PRIVATE_END: &str = "-----END PRIVATE KEY-----";
const PEM_PUBLIC_BEGIN: &str = "-----BEGIN PUBLIC KEY-----";
const PEM_PUBLIC_END: &str = "-----END PUBLIC KEY-----";

const KEY_BYTES: usize = 32;

// DER prefixes for a 32-byte key with the algorithm OID 1.3.101.<oid>.
const fn secret_key_header(oid: u8) -> [u8; 16] {
    [48, 46, 2, 1, 0, 48, 5, 6, 3, 43, 101, oid, 4, 34, 4, 32]
}

const fn public_key_header(oid: u8) -> [u8; 12] {
    [48, 42, 48, 5, 6, 3, 43, 101, oid, 3, 33, 0]
}

fn key_from_der<'a>(der: &'a [u8], header: &[u8]) -> Result<&'a [u8], Error> {
    if der.len() != header.len() + KEY_BYTES || !der.starts_with(header) {
        return Err(Error::ParseError);
    }
    Ok(&der[header.len()..])
}

#[cfg(feature = "std")]
fn key_to_der(header: &[u8], key: &[u8]) -> Vec<u8> {
    [header, key].concat()
}

/// Decodes the base64 body of the first PEM block with the given markers into
/// `der`, returning only the bytes that were actually decoded.
fn decode_pem<'t>(pem: &str, begin: &str, end: &str, der: &'t mut [u8]) -> Result<&'t [u8], Error> {
    let mut it = pem.split(begin);
    let _ = it.next().ok_or(Error::ParseError)?;
    let inner = it.next().ok_or(Error::ParseError)?;
    let mut it = inner.split(end);
    let b64 = it.next().ok_or(Error::ParseError)?;
    let _ = it.next().ok_or(Error::ParseError)?;
    Base64::decode(der, b64, Some(b"\r\n\t ")).map_err(|_| Error::ParseError)
}

fn secret_key_from_pem<T>(
    pem: &str,
    from_der: impl FnOnce(&[u8]) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut der = [0u8; 16 + KEY_BYTES];
    let key = decode_pem(pem, PEM_PRIVATE_BEGIN, PEM_PRIVATE_END, &mut der).and_then(from_der);
    Mem::wipe(&mut der);
    key
}

fn public_key_from_pem<T>(
    pem: &str,
    from_der: impl FnOnce(&[u8]) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut der = [0u8; 12 + KEY_BYTES];
    decode_pem(pem, PEM_PUBLIC_BEGIN, PEM_PUBLIC_END, &mut der).and_then(from_der)
}

// The string is allocated once, so no partial copy of a secret key is left
// behind in a reallocated buffer.
#[cfg(feature = "std")]
fn encode_pem(der: &[u8], begin: &str, end: &str) -> String {
    let b64 = Base64::encode_to_string(der).unwrap();
    let mut pem = String::with_capacity(begin.len() + b64.len() + end.len() + 3);
    for line in [begin, b64.as_str(), end] {
        pem.push_str(line);
        pem.push('\n');
    }
    Mem::wipe(&mut b64.into_bytes());
    pem
}

#[cfg(feature = "std")]
fn secret_key_to_pem(mut der: Vec<u8>) -> String {
    let pem = encode_pem(&der, PEM_PRIVATE_BEGIN, PEM_PRIVATE_END);
    Mem::wipe(&mut der);
    pem
}

#[cfg(feature = "std")]
fn public_key_to_pem(der: &[u8]) -> String {
    encode_pem(der, PEM_PUBLIC_BEGIN, PEM_PUBLIC_END)
}

#[cfg(not(feature = "disable-signatures"))]
mod ed25519 {
    use super::*;
    use crate::{KeyPair, PublicKey, SecretKey, Seed};

    const SECRET_KEY_HEADER: [u8; 16] = secret_key_header(112);
    const PUBLIC_KEY_HEADER: [u8; 12] = public_key_header(112);

    impl KeyPair {
        /// Import a key pair from an OpenSSL-compatible DER file.
        ///
        /// Returns `Err(Error::InvalidSeed)` if the seed is all zeros.
        pub fn from_der(der: &[u8]) -> Result<Self, Error> {
            let mut seed = Seed::from_slice(key_from_der(der, &SECRET_KEY_HEADER)?)?;
            let kp = KeyPair::try_from_seed(seed);
            seed.wipe_mut();
            kp
        }

        /// Import a key pair from an OpenSSL-compatible PEM file.
        pub fn from_pem(pem: &str) -> Result<Self, Error> {
            secret_key_from_pem(pem, Self::from_der)
        }

        /// Export a key pair as an OpenSSL-compatible PEM file.
        #[cfg(feature = "std")]
        pub fn to_pem(&self) -> String {
            self.sk.to_pem() + &self.pk.to_pem()
        }
    }

    impl SecretKey {
        /// Import a secret key from an OpenSSL-compatible DER file.
        pub fn from_der(der: &[u8]) -> Result<Self, Error> {
            KeyPair::from_der(der).map(|kp| kp.sk)
        }

        /// Import a secret key from an OpenSSL-compatible PEM file.
        pub fn from_pem(pem: &str) -> Result<Self, Error> {
            KeyPair::from_pem(pem).map(|kp| kp.sk)
        }

        /// Export a secret key as an OpenSSL-compatible DER file.
        #[cfg(feature = "std")]
        pub fn to_der(&self) -> Vec<u8> {
            key_to_der(&SECRET_KEY_HEADER, &self[..Seed::BYTES])
        }

        /// Export a secret key as an OpenSSL-compatible PEM file.
        #[cfg(feature = "std")]
        pub fn to_pem(&self) -> String {
            secret_key_to_pem(self.to_der())
        }
    }

    impl PublicKey {
        /// Import a public key from an OpenSSL-compatible DER file.
        ///
        /// Only the length and the structure are checked.
        /// Use `validate()` to reject non-canonical and weak keys.
        pub fn from_der(der: &[u8]) -> Result<Self, Error> {
            PublicKey::from_slice(key_from_der(der, &PUBLIC_KEY_HEADER)?)
        }

        /// Import a public key from an OpenSSL-compatible PEM file.
        ///
        /// Only the length and the structure are checked.
        /// Use `validate()` to reject non-canonical and weak keys.
        pub fn from_pem(pem: &str) -> Result<Self, Error> {
            public_key_from_pem(pem, Self::from_der)
        }

        /// Export a public key as an OpenSSL-compatible DER file.
        #[cfg(feature = "std")]
        pub fn to_der(&self) -> Vec<u8> {
            key_to_der(&PUBLIC_KEY_HEADER, &self[..])
        }

        /// Export a public key as an OpenSSL-compatible PEM file.
        #[cfg(feature = "std")]
        pub fn to_pem(&self) -> String {
            public_key_to_pem(&self.to_der())
        }
    }
}

#[cfg(feature = "x25519")]
mod x25519 {
    use super::*;
    use crate::x25519::{KeyPair, PublicKey, SecretKey};

    const SECRET_KEY_HEADER: [u8; 16] = secret_key_header(110);
    const PUBLIC_KEY_HEADER: [u8; 12] = public_key_header(110);

    impl KeyPair {
        /// Import a key pair from an OpenSSL-compatible DER file.
        ///
        /// The public key is recomputed from the secret key.
        pub fn from_der(der: &[u8]) -> Result<Self, Error> {
            let sk = SecretKey::from_der(der)?;
            let pk = sk.recover_public_key()?;
            Ok(KeyPair { pk, sk })
        }

        /// Import a key pair from an OpenSSL-compatible PEM file.
        ///
        /// The public key is recomputed from the secret key.
        pub fn from_pem(pem: &str) -> Result<Self, Error> {
            secret_key_from_pem(pem, Self::from_der)
        }
    }

    impl SecretKey {
        /// Import a secret key from an OpenSSL-compatible DER file.
        ///
        /// The scalar is kept as encoded; clamping only happens when it is used.
        pub fn from_der(der: &[u8]) -> Result<Self, Error> {
            SecretKey::from_slice(key_from_der(der, &SECRET_KEY_HEADER)?)
        }

        /// Import a secret key from an OpenSSL-compatible PEM file.
        pub fn from_pem(pem: &str) -> Result<Self, Error> {
            secret_key_from_pem(pem, Self::from_der)
        }

        /// Export a secret key as an OpenSSL-compatible DER file.
        #[cfg(feature = "std")]
        pub fn to_der(&self) -> Vec<u8> {
            key_to_der(&SECRET_KEY_HEADER, &self[..])
        }

        /// Export a secret key as an OpenSSL-compatible PEM file.
        #[cfg(feature = "std")]
        pub fn to_pem(&self) -> String {
            secret_key_to_pem(self.to_der())
        }
    }

    impl PublicKey {
        /// Import a public key from an OpenSSL-compatible DER file.
        ///
        /// The same checks as `from_slice()` apply.
        pub fn from_der(der: &[u8]) -> Result<Self, Error> {
            PublicKey::from_slice(key_from_der(der, &PUBLIC_KEY_HEADER)?)
        }

        /// Import a public key from an OpenSSL-compatible PEM file.
        ///
        /// The same checks as `from_slice()` apply.
        pub fn from_pem(pem: &str) -> Result<Self, Error> {
            public_key_from_pem(pem, Self::from_der)
        }

        /// Export a public key as an OpenSSL-compatible DER file.
        #[cfg(feature = "std")]
        pub fn to_der(&self) -> Vec<u8> {
            key_to_der(&PUBLIC_KEY_HEADER, &self[..])
        }

        /// Export a public key as an OpenSSL-compatible PEM file.
        #[cfg(feature = "std")]
        pub fn to_pem(&self) -> String {
            public_key_to_pem(&self.to_der())
        }
    }
}

#[cfg(all(test, feature = "std"))]
fn der_of(pem: &str) -> Vec<u8> {
    let b64: String = pem.lines().filter(|l| !l.starts_with("-----")).collect();
    Base64::decode_to_vec(b64, None).unwrap()
}

// The DER and PEM parsing code is shared by both key types, so the tests for
// malformed input only use Ed25519 keys.
#[cfg(all(test, feature = "std", not(feature = "disable-signatures")))]
mod ed25519_tests {
    use super::*;
    use crate::{KeyPair, PublicKey, SecretKey};

    const SK_PEM: &str = "-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VwBCIEIMXY1NUbUe/3dW2YUoKW5evsnCJPMfj60/q0RzGne3gg
-----END PRIVATE KEY-----
";
    const PK_PEM: &str = "-----BEGIN PUBLIC KEY-----
MCowBQYDK2VwAyEAyrRjJfTnhMcW5igzYvPirFW5eUgMdKeClGzQhd4qw+Y=
-----END PUBLIC KEY-----
";

    #[test]
    fn test_pem() {
        let sk = SecretKey::from_pem(SK_PEM).unwrap();
        let pk = PublicKey::from_pem(PK_PEM).unwrap();
        assert_eq!(sk.public_key(), pk);
        assert_eq!(sk.to_pem(), SK_PEM);
        assert_eq!(pk.to_pem(), PK_PEM);
        assert_eq!(
            KeyPair::from_pem(SK_PEM).unwrap().to_pem(),
            SK_PEM.to_string() + PK_PEM
        );
    }

    #[test]
    fn test_rejected_inputs() {
        let sk_der = der_of(SK_PEM);
        let pk_der = der_of(PK_PEM);

        // A short body must not be padded with zeros into a valid key.
        let mut long_sk = sk_der.clone();
        long_sk.push(0);
        for der in [
            &sk_der[..0],
            &sk_der[..1],
            &sk_der[..16],
            &sk_der[..47],
            &long_sk[..],
        ] {
            assert_eq!(KeyPair::from_der(der), Err(Error::ParseError));
            let pem = secret_key_to_pem(der.to_vec());
            assert_eq!(KeyPair::from_pem(&pem), Err(Error::ParseError));
        }
        let mut long_pk = pk_der.clone();
        long_pk.push(0);
        for der in [
            &pk_der[..0],
            &pk_der[..1],
            &pk_der[..12],
            &pk_der[..43],
            &long_pk[..],
        ] {
            assert_eq!(PublicKey::from_der(der), Err(Error::ParseError));
            let pem = public_key_to_pem(der);
            assert_eq!(PublicKey::from_pem(&pem), Err(Error::ParseError));
        }

        // Bad base64, a missing end marker, and no markers at all.
        for pem in [
            SK_PEM.replace("AQAw", "AQAw!"),
            SK_PEM.replace(PEM_PRIVATE_END, ""),
            SK_PEM.lines().nth(1).unwrap().to_string(),
        ] {
            assert_eq!(KeyPair::from_pem(&pem), Err(Error::ParseError));
        }

        // RFC 8410 section 10.3: PKCS#8 v2, with attributes and a public key.
        let v2 = "-----BEGIN PRIVATE KEY-----
MHICAQEwBQYDK2VwBCIEINTuctv5E1hK1bbY8fdp+K06/nwoy/HU++CXqI9EdVhC
oB8wHQYKKoZIhvcNAQkJFDEPDA1DdXJkbGUgQ2hhaXJzgSEAGb9ECWmEzf6FQbrB
Z9w7lshQhqowtrbLDFw4rXAxZuE=
-----END PRIVATE KEY-----";
        assert_eq!(KeyPair::from_pem(v2), Err(Error::ParseError));

        let mut zero_seed = sk_der;
        zero_seed[16..].fill(0);
        assert_eq!(KeyPair::from_der(&zero_seed), Err(Error::InvalidSeed));
        let pem = secret_key_to_pem(zero_seed);
        assert_eq!(KeyPair::from_pem(&pem), Err(Error::InvalidSeed));
    }
}

#[cfg(all(test, feature = "std", feature = "x25519"))]
mod x25519_tests {
    use ct_codecs::Hex;

    use super::*;
    use crate::x25519::{KeyPair, PublicKey, SecretKey};

    // `openssl genpkey -algorithm X25519` with OpenSSL 3.6.4.
    const OPENSSL_SK_PEM: &str = "-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VuBCIEIDh9sYQUi6bsE4+FzTMYeh4I5ds0Muj7yI2YdN3RQvl4
-----END PRIVATE KEY-----
";
    const OPENSSL_PK_PEM: &str = "-----BEGIN PUBLIC KEY-----
MCowBQYDK2VuAyEAXjm28Nw5gWp1PWDAiK4LlgncW9uiX7ZZhpuTLkBkAh8=
-----END PUBLIC KEY-----
";
    const OPENSSL_SK: &str = "387db184148ba6ec138f85cd33187a1e08e5db3432e8fbc88d9874ddd142f978";
    const OPENSSL_PK: &str = "5e39b6f0dc39816a753d60c088ae0b9609dc5bdba25fb659869b932e4064021f";

    // crypto/x509 MarshalPKCS8PrivateKey and MarshalPKIXPublicKey with Go 1.27.1.
    const GO_SK_PEM: &str = "-----BEGIN PRIVATE KEY-----
MC4CAQAwBQYDK2VuBCIEIOLNilJ0nZiyInBEyRPq/J5cGaMbNJxBch8448U+Y6yM
-----END PRIVATE KEY-----
";
    const GO_PK_PEM: &str = "-----BEGIN PUBLIC KEY-----
MCowBQYDK2VuAyEAc8kR/5753RlVzNuVn9ehTBSg9DdhS6RJcIdKnJkFN3s=
-----END PUBLIC KEY-----
";
    const GO_SK: &str = "e2cd8a52749d98b2227044c913eafc9e5c19a31b349c41721f38e3c53e63ac8c";
    const GO_PK: &str = "73c911ff9ef9dd1955ccdb959fd7a14c14a0f437614ba44970874a9c9905377b";

    #[test]
    fn test_x25519_pem() {
        for (sk_pem, pk_pem, sk_hex, pk_hex) in [
            (OPENSSL_SK_PEM, OPENSSL_PK_PEM, OPENSSL_SK, OPENSSL_PK),
            (GO_SK_PEM, GO_PK_PEM, GO_SK, GO_PK),
        ] {
            let sk = SecretKey::from_pem(sk_pem).unwrap();
            let pk = PublicKey::from_pem(pk_pem).unwrap();
            assert_eq!(&sk[..], &Hex::decode_to_vec(sk_hex, None).unwrap()[..]);
            assert_eq!(&pk[..], &Hex::decode_to_vec(pk_hex, None).unwrap()[..]);
            assert_eq!(KeyPair::from_pem(sk_pem).unwrap().pk, pk);
            assert_eq!(sk.to_pem(), sk_pem);
            assert_eq!(pk.to_pem(), pk_pem);
        }

        // Secret keys are stored as encoded, and only clamped when they are used.
        let mut der = der_of(OPENSSL_SK_PEM);
        der[16] |= 7;
        der[47] |= 0x80;
        assert_eq!(&SecretKey::from_der(&der).unwrap()[..], &der[16..]);
    }

    #[test]
    fn test_x25519_rejected_inputs() {
        // 1.3.101.112 is the Ed25519 OID.
        let mut ed25519_sk_der = der_of(OPENSSL_SK_PEM);
        ed25519_sk_der[11] = 112;
        assert_eq!(SecretKey::from_der(&ed25519_sk_der), Err(Error::ParseError));
        let mut ed25519_pk_der = der_of(OPENSSL_PK_PEM);
        ed25519_pk_der[8] = 112;
        assert_eq!(PublicKey::from_der(&ed25519_pk_der), Err(Error::ParseError));

        // p = 2^255 - 19 is not a canonical encoding.
        let mut pk_der = der_of(OPENSSL_PK_PEM);
        pk_der[12] = 0xed;
        pk_der[13..43].fill(0xff);
        pk_der[43] = 0x7f;
        assert_eq!(PublicKey::from_der(&pk_der), Err(Error::NonCanonical));
    }
}
