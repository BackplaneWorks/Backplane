use core::convert::TryFrom;
use core::fmt;
use core::ops::{Deref, DerefMut};

use super::common::*;
#[cfg(feature = "blind-keys")]
use super::edwards25519::{ge_scalarmult, sc_invert, sc_mul};
use super::edwards25519::{
    ge_scalarmult_base, sc_muladd, sc_reduce, sc_reduce32, sc_reject_noncanonical, GeP2, GeP3,
};
use super::error::Error;
use super::sha512;

/// A public key.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct PublicKey([u8; PublicKey::BYTES]);

impl PublicKey {
    /// Number of raw bytes in a public key.
    pub const BYTES: usize = 32;

    /// Creates a public key from raw bytes.
    pub fn new(pk: [u8; PublicKey::BYTES]) -> Self {
        PublicKey(pk)
    }

    /// Creates a public key from a slice.
    pub fn from_slice(pk: &[u8]) -> Result<Self, Error> {
        let mut pk_ = [0u8; PublicKey::BYTES];
        if pk.len() != pk_.len() {
            return Err(Error::InvalidPublicKey);
        }
        pk_.copy_from_slice(pk);
        Ok(PublicKey::new(pk_))
    }

    /// Returns `Ok(())` if the public key is the canonical encoding of a point
    /// that doesn't have a small order.
    ///
    /// Constructors only check the length of a public key, so this should be
    /// called on keys coming from untrusted sources before storing them.
    /// Verification always performs the same checks, so a key that fails here
    /// can never verify a signature.
    ///
    /// Returns `Err(Error::InvalidPublicKey)` if the encoding is not canonical
    /// or not a point, and `Err(Error::WeakPublicKey)` if the point has a
    /// small order.
    pub fn validate(&self) -> Result<(), Error> {
        decode_point_negated(self).map(|_| ())
    }
}

// Verification needs the negated points, so this returns -P.
fn decode_point_negated(bytes: &[u8; 32]) -> Result<GeP3, Error> {
    let p = GeP3::from_bytes_negate_vartime(bytes).ok_or(Error::InvalidPublicKey)?;
    if p.has_small_order() {
        return Err(Error::WeakPublicKey);
    }
    Ok(p)
}

impl Deref for PublicKey {
    type Target = [u8; PublicKey::BYTES];

    /// Returns a public key as bytes.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for PublicKey {
    /// Returns a public key as mutable bytes.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A secret key.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SecretKey([u8; SecretKey::BYTES]);

impl SecretKey {
    /// Number of bytes in a secret key.
    pub const BYTES: usize = 32 + PublicKey::BYTES;

    /// Creates a secret key from raw bytes.
    pub fn new(sk: [u8; SecretKey::BYTES]) -> Self {
        SecretKey(sk)
    }

    /// Creates a secret key from a slice.
    pub fn from_slice(sk: &[u8]) -> Result<Self, Error> {
        let mut sk_ = [0u8; SecretKey::BYTES];
        if sk.len() != sk_.len() {
            return Err(Error::InvalidSecretKey);
        }
        sk_.copy_from_slice(sk);
        Ok(SecretKey::new(sk_))
    }

    /// Returns the public counterpart of a secret key.
    pub fn public_key(&self) -> PublicKey {
        let mut pk = [0u8; PublicKey::BYTES];
        pk.copy_from_slice(&self[Seed::BYTES..]);
        PublicKey(pk)
    }

    /// Returns the seed of a secret key.
    pub fn seed(&self) -> Seed {
        Seed::from_slice(&self[0..Seed::BYTES]).unwrap()
    }

    /// Returns `Ok(())` if the given public key is the public counterpart of
    /// this secret key.
    ///
    /// The public key is recomputed from the seed, and must match both `pk`
    /// and the copy stored in the secret key.
    /// This also catches a corrupted secret key.
    ///
    /// Returns `Err(Error::InvalidPublicKey)` otherwise, or
    /// `Err(Error::InvalidSeed)` if the seed is all zeros.
    pub fn validate_public_key(&self, pk: &PublicKey) -> Result<(), Error> {
        let kp = KeyPair::try_from_seed(self.seed())?;
        if kp.pk != *pk || kp.pk != self.public_key() {
            return Err(Error::InvalidPublicKey);
        }
        Ok(())
    }
}

impl Drop for SecretKey {
    fn drop(&mut self) {
        Mem::wipe(&mut self.0)
    }
}

impl Deref for SecretKey {
    type Target = [u8; SecretKey::BYTES];

    /// Returns a secret key as bytes.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for SecretKey {
    /// Returns a secret key as mutable bytes.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// A key pair.
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct KeyPair {
    /// Public key part of the key pair.
    pub pk: PublicKey,
    /// Secret key part of the key pair.
    pub sk: SecretKey,
}

/// An Ed25519 signature.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Signature([u8; Signature::BYTES]);

impl fmt::Debug for Signature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!("{:x?}", &self.0))
    }
}

impl TryFrom<&[u8]> for Signature {
    type Error = Error;

    fn try_from(slice: &[u8]) -> Result<Self, Self::Error> {
        Signature::from_slice(slice)
    }
}

impl AsRef<[u8]> for Signature {
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

impl Signature {
    /// Number of raw bytes in a signature.
    pub const BYTES: usize = 64;

    /// Creates a signature from raw bytes.
    pub fn new(bytes: [u8; Signature::BYTES]) -> Self {
        Signature(bytes)
    }

    /// Creates a signature key from a slice.
    pub fn from_slice(signature: &[u8]) -> Result<Self, Error> {
        let mut signature_ = [0u8; Signature::BYTES];
        if signature.len() != signature_.len() {
            return Err(Error::InvalidSignature);
        }
        signature_.copy_from_slice(signature);
        Ok(Signature::new(signature_))
    }
}

impl Deref for Signature {
    type Target = [u8; Signature::BYTES];

    /// Returns a signture as bytes.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Signature {
    /// Returns a signature as mutable bytes.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The state of a streaming verification operation.
#[derive(Clone)]
pub struct VerifyingState {
    hasher: sha512::Hash,
    s: [u8; 32],
    minus_a: GeP3,
    minus_r: GeP3,
}

impl Drop for VerifyingState {
    fn drop(&mut self) {
        Mem::wipe(&mut self.s);
    }
}

impl VerifyingState {
    fn new(pk: &PublicKey, signature: &Signature) -> Result<Self, Error> {
        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&signature[0..32]);
        s.copy_from_slice(&signature[32..64]);
        sc_reject_noncanonical(&s)?;
        let minus_a = decode_point_negated(pk)?;
        let minus_r = decode_point_negated(&r).map_err(|_| Error::InvalidSignature)?;
        let mut hasher = sha512::Hash::new();
        hasher.update(r);
        hasher.update(&pk[..]);
        Ok(VerifyingState {
            hasher,
            s,
            minus_a,
            minus_r,
        })
    }

    /// Appends data to the message being verified.
    pub fn absorb(&mut self, chunk: impl AsRef<[u8]>) {
        self.hasher.update(chunk)
    }

    /// Verifies the signature and return it.
    pub fn verify(&self) -> Result<(), Error> {
        let mut hash = self.hasher.finalize();
        sc_reduce(&mut hash);

        let r = GeP2::double_scalarmult_vartime(hash.as_ref(), self.minus_a, &self.s);
        if (r + self.minus_r).has_small_order() {
            Ok(())
        } else {
            Err(Error::SignatureMismatch)
        }
    }
}

impl PublicKey {
    /// Verify the signature of a multi-part message (streaming).
    pub fn verify_incremental(&self, signature: &Signature) -> Result<VerifyingState, Error> {
        VerifyingState::new(self, signature)
    }

    /// Verifies that the signature `signature` is valid for the message
    /// `message`.
    pub fn verify(&self, message: impl AsRef<[u8]>, signature: &Signature) -> Result<(), Error> {
        let mut st = VerifyingState::new(self, signature)?;
        st.absorb(message);
        st.verify()
    }
}

/// The state of a streaming signature operation.
#[derive(Clone)]
pub struct SigningState {
    hasher: sha512::Hash,
    az: [u8; 64],
    nonce: [u8; 64],
    r: [u8; 32],
}

impl Drop for SigningState {
    fn drop(&mut self) {
        Mem::wipe(&mut self.az);
        Mem::wipe(&mut self.nonce);
    }
}

impl SigningState {
    fn new(nonce: [u8; 64], az: [u8; 64], pk_: &[u8]) -> Self {
        let r = ge_scalarmult_base(&nonce[0..32]).to_bytes();

        let mut st = sha512::Hash::new();
        st.update(&r);
        st.update(pk_);

        SigningState {
            hasher: st,
            nonce,
            az,
            r,
        }
    }

    /// Appends data to the message being signed.
    pub fn absorb(&mut self, chunk: impl AsRef<[u8]>) {
        self.hasher.update(chunk)
    }

    /// Computes the signature and return it.
    pub fn sign(&self) -> Signature {
        let mut signature: [u8; 64] = [0; 64];
        signature[0..32].copy_from_slice(&self.r);
        let mut hram = self.hasher.finalize();
        sc_reduce(&mut hram);
        sc_muladd(
            &mut signature[32..64],
            &hram[0..32],
            &self.az[0..32],
            &self.nonce[0..32],
        );
        Signature(signature)
    }
}

impl SecretKey {
    /// Sign a multi-part message (streaming API).
    /// It is critical for `noise` to never repeat.
    pub fn sign_incremental(&self, noise: Noise) -> SigningState {
        let seed = &self[0..32];
        let pk = &self[32..64];
        let az: [u8; 64] = {
            let mut hash_output = sha512::Hash::hash(seed);
            hash_output[0] &= 248;
            hash_output[31] &= 63;
            hash_output[31] |= 64;
            hash_output
        };
        let mut st = sha512::Hash::new();
        #[cfg(feature = "random")]
        {
            let additional_noise = Noise::generate();
            st.update(additional_noise.as_ref());
        }
        st.update(noise.as_ref());
        st.update(seed);
        let nonce = st.finalize();
        SigningState::new(nonce, az, pk)
    }

    /// Computes a signature for the message `message` using the secret key.
    /// The noise parameter is optional, but recommended in order to mitigate
    /// fault attacks.
    pub fn sign(&self, message: impl AsRef<[u8]>, noise: Option<Noise>) -> Signature {
        let message = message.as_ref();
        let seed = &self[0..32];
        let pk = &self[32..64];
        let az: [u8; 64] = {
            let mut hash_output = sha512::Hash::hash(seed);
            hash_output[0] &= 248;
            hash_output[31] &= 63;
            hash_output[31] |= 64;
            hash_output
        };
        let nonce = {
            let mut hasher = sha512::Hash::new();
            if let Some(noise) = noise {
                hasher.update(&noise[..]);
                hasher.update(&az[..]);
            } else {
                hasher.update(&az[32..64]);
            }
            hasher.update(message);
            let mut hash_output = hasher.finalize();
            sc_reduce(&mut hash_output[0..64]);
            hash_output
        };
        let mut st = SigningState::new(nonce, az, pk);
        st.absorb(message);
        let signature = st.sign();

        #[cfg(feature = "self-verify")]
        {
            PublicKey::from_slice(pk)
                .expect("Key length changed")
                .verify(message, &signature)
                .expect("Newly created signature cannot be verified");
        }

        signature
    }
}

impl KeyPair {
    /// Number of bytes in a key pair.
    pub const BYTES: usize = SecretKey::BYTES;

    /// Generates a new key pair.
    #[cfg(feature = "random")]
    pub fn generate() -> KeyPair {
        KeyPair::from_seed(Seed::default())
    }

    /// Generates a new key pair using a secret seed.
    ///
    /// Panics if the seed is all zeros.
    /// Use `try_from_seed()` for seeds that don't come from a random number
    /// generator.
    pub fn from_seed(seed: Seed) -> KeyPair {
        Self::try_from_seed(seed).expect("All-zero seed")
    }

    /// Generates a new key pair using a secret seed.
    ///
    /// Returns `Err(Error::InvalidSeed)` if the seed is all zeros.
    pub fn try_from_seed(seed: Seed) -> Result<KeyPair, Error> {
        if seed.iter().fold(0, |acc, x| acc | x) == 0 {
            return Err(Error::InvalidSeed);
        }
        let (scalar, _) = {
            let hash_output = sha512::Hash::hash(&seed[..]);
            KeyPair::split(&hash_output, false, true)
        };
        let pk = ge_scalarmult_base(&scalar).to_bytes();
        let mut sk = [0u8; 64];
        sk[0..32].copy_from_slice(&*seed);
        sk[32..64].copy_from_slice(&pk);
        Ok(KeyPair {
            pk: PublicKey(pk),
            sk: SecretKey(sk),
        })
    }

    /// Creates a key pair from a slice.
    ///
    /// The public half is not checked against the seed.
    /// Call `validate()` on key pairs coming from untrusted sources.
    pub fn from_slice(bytes: &[u8]) -> Result<Self, Error> {
        let sk = SecretKey::from_slice(bytes)?;
        let pk = sk.public_key();
        Ok(KeyPair { pk, sk })
    }

    /// Clamp a scalar.
    pub fn clamp(scalar: &mut [u8]) {
        scalar[0] &= 248;
        scalar[31] &= 63;
        scalar[31] |= 64;
    }

    /// Split a serialized representation of a key pair into a secret scalar and
    /// a prefix.
    pub fn split(bytes: &[u8; 64], reduce: bool, clamp: bool) -> ([u8; 32], [u8; 32]) {
        let mut scalar = [0u8; 32];
        scalar.copy_from_slice(&bytes[0..32]);
        if clamp {
            Self::clamp(&mut scalar);
        }
        if reduce {
            sc_reduce32(&mut scalar);
        }
        let mut prefix = [0u8; 32];
        prefix.copy_from_slice(&bytes[32..64]);
        (scalar, prefix)
    }

    /// Check that the public key is valid for the secret key.
    ///
    /// Returns `Err(Error::InvalidSeed)` if the seed is all zeros, and
    /// `Err(Error::InvalidPublicKey)` if either half of the key pair doesn't
    /// match the public key computed from the seed.
    pub fn validate(&self) -> Result<(), Error> {
        self.sk.validate_public_key(&self.pk)
    }
}

impl Deref for KeyPair {
    type Target = [u8; KeyPair::BYTES];

    /// Returns a key pair as bytes.
    fn deref(&self) -> &Self::Target {
        &self.sk
    }
}

impl DerefMut for KeyPair {
    /// Returns a key pair as mutable bytes.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.sk
    }
}

/// Noise, for non-deterministic signatures.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Noise([u8; Noise::BYTES]);

impl Noise {
    /// Number of raw bytes for a noise component.
    pub const BYTES: usize = 16;

    /// Creates a new noise component from raw bytes.
    pub fn new(noise: [u8; Noise::BYTES]) -> Self {
        Noise(noise)
    }

    /// Creates noise from a slice.
    pub fn from_slice(noise: &[u8]) -> Result<Self, Error> {
        let mut noise_ = [0u8; Noise::BYTES];
        if noise.len() != noise_.len() {
            return Err(Error::InvalidNoise);
        }
        noise_.copy_from_slice(noise);
        Ok(Noise::new(noise_))
    }
}

impl Deref for Noise {
    type Target = [u8; Noise::BYTES];

    /// Returns the noise as bytes.
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Noise {
    /// Returns the noise as mutable bytes.
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[cfg(feature = "random")]
impl Default for Noise {
    /// Generates random noise.
    fn default() -> Self {
        let mut noise = [0u8; Noise::BYTES];
        getrandom::fill(&mut noise).expect("RNG failure");
        Noise(noise)
    }
}

#[cfg(feature = "random")]
impl Noise {
    /// Generates random noise.
    pub fn generate() -> Self {
        Noise::default()
    }
}

#[cfg(feature = "traits")]
mod ed25519_trait {
    use ::ed25519::signature as ed25519_trait;

    use super::{PublicKey, SecretKey, Signature};

    impl ed25519_trait::SignatureEncoding for Signature {
        type Repr = Signature;
    }

    impl ed25519_trait::Signer<Signature> for SecretKey {
        fn try_sign(&self, message: &[u8]) -> Result<Signature, ed25519_trait::Error> {
            Ok(self.sign(message, None))
        }
    }

    impl ed25519_trait::Verifier<Signature> for PublicKey {
        fn verify(
            &self,
            message: &[u8],
            signature: &Signature,
        ) -> Result<(), ed25519_trait::Error> {
            #[cfg(feature = "std")]
            {
                self.verify(message, signature)
                    .map_err(ed25519_trait::Error::from_source)
            }

            #[cfg(not(feature = "std"))]
            {
                self.verify(message, signature)
                    .map_err(|_| ed25519_trait::Error::new())
            }
        }
    }
}

#[test]
fn test_ed25519() {
    let kp = KeyPair::from_seed([42u8; 32].into());
    let message = b"Hello, World!";
    let signature = kp.sk.sign(message, None);
    assert!(kp.pk.verify(message, &signature).is_ok());
    assert!(kp.pk.verify(b"Hello, world!", &signature).is_err());
    assert_eq!(
        signature.as_ref(),
        [
            196, 182, 1, 15, 182, 182, 231, 166, 227, 62, 243, 85, 49, 174, 169, 9, 162, 196, 98,
            104, 30, 81, 22, 38, 184, 136, 253, 128, 10, 160, 128, 105, 127, 130, 138, 164, 57, 86,
            94, 160, 216, 85, 153, 139, 81, 100, 38, 124, 235, 210, 26, 95, 231, 90, 73, 206, 33,
            216, 171, 15, 188, 181, 136, 7,
        ]
    );
}

#[cfg(feature = "blind-keys")]
mod blind_keys {
    use super::*;

    #[derive(Clone, Debug, Eq, PartialEq, Hash)]
    pub struct Blind([u8; Blind::BYTES]);

    impl From<[u8; 32]> for Blind {
        fn from(blind: [u8; 32]) -> Self {
            Blind(blind)
        }
    }

    impl Blind {
        /// Number of raw bytes in a blind.
        pub const BYTES: usize = 32;

        /// Creates a blind from raw bytes.
        pub fn new(blind: [u8; Blind::BYTES]) -> Self {
            Blind(blind)
        }

        /// Creates a blind from a slice.
        pub fn from_slice(blind: &[u8]) -> Result<Self, Error> {
            let mut blind_ = [0u8; Blind::BYTES];
            if blind.len() != blind_.len() {
                return Err(Error::InvalidBlind);
            }
            blind_.copy_from_slice(blind);
            Ok(Blind::new(blind_))
        }
    }

    impl Drop for Blind {
        fn drop(&mut self) {
            Mem::wipe(&mut self.0)
        }
    }

    #[cfg(feature = "random")]
    impl Default for Blind {
        /// Generates a random blind.
        fn default() -> Self {
            let mut blind = [0u8; Blind::BYTES];
            getrandom::fill(&mut blind).expect("RNG failure");
            Blind(blind)
        }
    }

    #[cfg(feature = "random")]
    impl Blind {
        /// Generates a random blind.
        pub fn generate() -> Self {
            Blind::default()
        }
    }

    impl Deref for Blind {
        type Target = [u8; Blind::BYTES];

        /// Returns a blind as bytes.
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for Blind {
        /// Returns a blind as mutable bytes.
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    /// A blind public key.
    #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
    pub struct BlindPublicKey([u8; PublicKey::BYTES]);

    impl Deref for BlindPublicKey {
        type Target = [u8; BlindPublicKey::BYTES];

        /// Returns a public key as bytes.
        fn deref(&self) -> &Self::Target {
            &self.0
        }
    }

    impl DerefMut for BlindPublicKey {
        /// Returns a public key as mutable bytes.
        fn deref_mut(&mut self) -> &mut Self::Target {
            &mut self.0
        }
    }

    impl BlindPublicKey {
        /// Number of bytes in a blind public key.
        pub const BYTES: usize = PublicKey::BYTES;

        /// Creates a blind public key from raw bytes.
        pub fn new(bpk: [u8; PublicKey::BYTES]) -> Self {
            BlindPublicKey(bpk)
        }

        /// Creates a blind public key from a slice.
        pub fn from_slice(bpk: &[u8]) -> Result<Self, Error> {
            let mut bpk_ = [0u8; PublicKey::BYTES];
            if bpk.len() != bpk_.len() {
                return Err(Error::InvalidPublicKey);
            }
            bpk_.copy_from_slice(bpk);
            Ok(BlindPublicKey::new(bpk_))
        }

        /// Unblinds a public key.
        pub fn unblind(&self, blind: &Blind, ctx: impl AsRef<[u8]>) -> Result<PublicKey, Error> {
            let pk_p3 = GeP3::from_bytes_vartime(&self.0).ok_or(Error::InvalidPublicKey)?;
            let mut hx = sha512::Hash::new();
            hx.update(&blind[..]);
            hx.update([0u8]);
            hx.update(ctx.as_ref());
            let hash_output = hx.finalize();
            let (blind_factor, _) = KeyPair::split(&hash_output, true, false);
            let inverse = sc_invert(&blind_factor);
            Ok(PublicKey(ge_scalarmult(&inverse, &pk_p3).to_bytes()))
        }

        /// Verifies that the signature `signature` is valid for the message
        /// `message`.
        pub fn verify(
            &self,
            message: impl AsRef<[u8]>,
            signature: &Signature,
        ) -> Result<(), Error> {
            PublicKey::new(self.0).verify(message, signature)
        }
    }

    impl From<PublicKey> for BlindPublicKey {
        fn from(pk: PublicKey) -> Self {
            BlindPublicKey(pk.0)
        }
    }

    impl From<BlindPublicKey> for PublicKey {
        fn from(bpk: BlindPublicKey) -> Self {
            PublicKey(bpk.0)
        }
    }

    /// A blind secret key.
    #[derive(Clone, Debug, Eq, PartialEq, Hash)]
    pub struct BlindSecretKey {
        pub prefix: [u8; 2 * Seed::BYTES],
        pub blind_scalar: [u8; 32],
        pub blind_pk: BlindPublicKey,
    }

    #[derive(Clone, Debug, Eq, PartialEq, Hash)]
    pub struct BlindKeyPair {
        /// Public key part of the blind key pair.
        pub blind_pk: BlindPublicKey,
        /// Secret key part of the blind key pair.
        pub blind_sk: BlindSecretKey,
    }

    impl BlindSecretKey {
        /// Computes a signature for the message `message` using the blind
        /// secret key. The noise parameter is optional, but recommended
        /// in order to mitigate fault attacks.
        pub fn sign(&self, message: impl AsRef<[u8]>, noise: Option<Noise>) -> Signature {
            let message = message.as_ref();
            let nonce = {
                let mut hasher = sha512::Hash::new();
                if let Some(noise) = noise {
                    hasher.update(&noise[..]);
                    hasher.update(&self.prefix);
                } else {
                    hasher.update(&self.prefix);
                }
                hasher.update(message);
                let mut hash_output = hasher.finalize();
                sc_reduce(&mut hash_output[0..64]);
                hash_output
            };
            let mut signature: [u8; 64] = [0; 64];
            let r = ge_scalarmult_base(&nonce[0..32]);
            signature[0..32].copy_from_slice(&r.to_bytes()[..]);
            signature[32..64].copy_from_slice(&self.blind_pk.0);
            let mut hasher = sha512::Hash::new();
            hasher.update(signature.as_ref());
            hasher.update(message);
            let mut hram = hasher.finalize();
            sc_reduce(&mut hram);
            sc_muladd(
                &mut signature[32..64],
                &hram[0..32],
                &self.blind_scalar,
                &nonce[0..32],
            );
            let signature = Signature(signature);

            #[cfg(feature = "self-verify")]
            {
                PublicKey::from_slice(&self.blind_pk.0)
                    .expect("Key length changed")
                    .verify(message, &signature)
                    .expect("Newly created signature cannot be verified");
            }
            signature
        }
    }

    impl Drop for BlindSecretKey {
        fn drop(&mut self) {
            Mem::wipe(&mut self.prefix);
            Mem::wipe(&mut self.blind_scalar);
        }
    }

    impl PublicKey {
        /// Returns a blind version of the public key.
        pub fn blind(&self, blind: &Blind, ctx: impl AsRef<[u8]>) -> Result<BlindPublicKey, Error> {
            let (blind_factor, _prefix2) = {
                let mut hx = sha512::Hash::new();
                hx.update(&blind[..]);
                hx.update([0u8]);
                hx.update(ctx.as_ref());
                let hash_output = hx.finalize();
                KeyPair::split(&hash_output, true, false)
            };
            let pk_p3 = GeP3::from_bytes_vartime(&self.0).ok_or(Error::InvalidPublicKey)?;
            Ok(BlindPublicKey(
                ge_scalarmult(&blind_factor, &pk_p3).to_bytes(),
            ))
        }
    }

    impl KeyPair {
        /// Returns a blind version of the key pair.
        pub fn blind(&self, blind: &Blind, ctx: impl AsRef<[u8]>) -> BlindKeyPair {
            let seed = self.sk.seed();
            let (scalar, prefix1) = {
                let hash_output = sha512::Hash::hash(&seed[..]);
                KeyPair::split(&hash_output, false, true)
            };

            let (blind_factor, prefix2) = {
                let mut hx = sha512::Hash::new();
                hx.update(&blind[..]);
                hx.update([0u8]);
                hx.update(ctx.as_ref());
                let hash_output = hx.finalize();
                KeyPair::split(&hash_output, true, false)
            };

            let blind_scalar = sc_mul(&scalar, &blind_factor);
            let blind_pk = ge_scalarmult_base(&blind_scalar).to_bytes();

            let mut prefix = [0u8; 2 * Seed::BYTES];
            prefix[0..32].copy_from_slice(&prefix1);
            prefix[32..64].copy_from_slice(&prefix2);
            let blind_pk = BlindPublicKey::new(blind_pk);

            BlindKeyPair {
                blind_pk,
                blind_sk: BlindSecretKey {
                    prefix,
                    blind_scalar,
                    blind_pk,
                },
            }
        }
    }
}

#[cfg(feature = "blind-keys")]
pub use blind_keys::*;

#[test]
#[cfg(all(feature = "blind-keys", feature = "random"))]
fn test_blind_ed25519() {
    use ct_codecs::{Decoder, Hex};

    let kp = KeyPair::generate();
    let blind = Blind::new([69u8; 32]);
    let blind_kp = kp.blind(&blind, "ctx");
    let message = b"Hello, World!";
    let signature = blind_kp.blind_sk.sign(message, None);
    assert!(blind_kp.blind_pk.verify(message, &signature).is_ok());
    let recovered_pk = blind_kp.blind_pk.unblind(&blind, "ctx").unwrap();
    assert!(recovered_pk == kp.pk);

    let kp = KeyPair::from_seed(
        Seed::from_slice(
            &Hex::decode_to_vec(
                "875532ab039b0a154161c284e19c74afa28d5bf5454e99284bbcffaa71eebf45",
                None,
            )
            .unwrap(),
        )
        .unwrap(),
    );
    assert_eq!(
        Hex::decode_to_vec(
            "3b5983605b277cd44918410eb246bb52d83adfc806ccaa91a60b5b2011bc5973",
            None
        )
        .unwrap(),
        kp.pk.as_ref()
    );

    let blind = Blind::from_slice(
        &Hex::decode_to_vec(
            "c461e8595f0ac41d374f878613206704978115a226f60470ffd566e9e6ae73bf",
            None,
        )
        .unwrap(),
    )
    .unwrap();
    let blind_kp = kp.blind(&blind, "ctx");
    assert_eq!(
        Hex::decode_to_vec(
            "246dcd43930b81d5e4d770db934a9fcd985b75fd014bc2a98b0aea02311c1836",
            None
        )
        .unwrap(),
        blind_kp.blind_pk.as_ref()
    );

    let message = Hex::decode_to_vec("68656c6c6f20776f726c64", None).unwrap();
    let signature = blind_kp.blind_sk.sign(message, None);
    assert_eq!(Hex::decode_to_vec("947bacfabc63448f8955dc20630e069e58f37b72bb433ae17f2fa904ea860b44deb761705a3cc2168a6673ee0b41ff7765c7a4896941eec6833c1689315acb0b",
        None).unwrap(), signature.as_ref());
}

#[cfg(feature = "random")]
#[test]
fn test_streaming() {
    let kp = KeyPair::generate();

    let msg1 = "mes";
    let msg2 = "sage";
    let mut st = kp.sk.sign_incremental(Noise::default());
    st.absorb(msg1);
    st.absorb(msg2);
    let signature = st.sign();
    assert_eq!(signature, st.sign());

    let msg1 = "mess";
    let msg2 = "age";
    let mut st = kp.pk.verify_incremental(&signature).unwrap();
    st.absorb(msg1);
    st.absorb(msg2);
    assert!(st.verify().is_ok());
}

#[test]
#[cfg(feature = "random")]
fn test_ed25519_invalid_keypair() {
    let kp1 = KeyPair::generate();
    let kp2 = KeyPair::generate();

    assert_eq!(
        kp1.sk.validate_public_key(&kp2.pk).unwrap_err(),
        Error::InvalidPublicKey
    );
    assert_eq!(
        kp2.sk.validate_public_key(&kp1.pk).unwrap_err(),
        Error::InvalidPublicKey
    );
    assert!(kp1.sk.validate_public_key(&kp1.pk).is_ok());
    assert!(kp2.sk.validate_public_key(&kp2.pk).is_ok());
    assert!(kp1.validate().is_ok());
}

#[test]
fn test_reject_noncanonical_identity_forgery() {
    let mut noncanonical_identity = [0xff; PublicKey::BYTES];
    noncanonical_identity[0] = 0xee;
    noncanonical_identity[31] = 0x7f;
    let pk = PublicKey::new(noncanonical_identity);

    let mut forged = [0u8; Signature::BYTES];
    forged[0] = 1;
    let signature = Signature::new(forged);

    assert!(pk.verify(b"any message", &signature).is_err());
}

#[cfg(all(test, feature = "std"))]
mod verification_tests {
    use ct_codecs::{Decoder, Hex};

    use super::*;
    use crate::edwards25519::small_order_points;

    fn hex<const N: usize>(s: &str) -> [u8; N] {
        <[u8; N]>::try_from(Hex::decode_to_vec(s, None).unwrap()).unwrap()
    }

    fn assert_verifies(pk: &PublicKey, message: &[u8], signature: &Signature) {
        pk.verify(message, signature).unwrap();
        let mut st = pk.verify_incremental(signature).unwrap();
        st.absorb(message);
        st.verify().unwrap();
    }

    fn assert_rejected(pk: &PublicKey, message: &[u8], signature: &Signature, error: Error) {
        assert_eq!(pk.verify(message, signature), Err(error));
        assert_eq!(pk.verify_incremental(signature).err(), Some(error));
    }

    #[test]
    fn rfc8032_vectors() {
        for (seed, pk, message, signature) in [
            (
                "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60",
                "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
                "",
                "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
            ),
            (
                "4ccd089b28ff96da9db6c346ec114e0f5b8a319f35aba624da8cf6ed4fb8a6fb",
                "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
                "72",
                "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
            ),
            (
                "c5aa8df43f9f837bedb7442f31dcb7b166d38535076f094b85ce3a2e0b4458f7",
                "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
                "af82",
                "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
            ),
        ] {
            let kp = KeyPair::try_from_seed(Seed::new(hex(seed))).unwrap();
            assert_eq!(kp.pk, PublicKey::new(hex(pk)));
            kp.validate().unwrap();
            kp.pk.validate().unwrap();
            let message = Hex::decode_to_vec(message, None).unwrap();
            let signature = Signature::new(hex(signature));
            assert_eq!(kp.sk.sign(&message, None), signature);
            assert_verifies(&kp.pk, &message, &signature);
        }
    }

    // Signed by Node.js v26.10.0 (OpenSSL).
    #[test]
    fn independent_signatures() {
        let seed = hex("7581a0f5c6370fc38d84129a410e62262513ac24cdf8cdb57f24876b8d75ad53");
        let pk = PublicKey::new(hex(
            "3d694c4d9fe31df8750b3e72a7a40262c1a3807cd0abb99c1f41db91b462f87f",
        ));
        assert_eq!(KeyPair::from_seed(Seed::new(seed)).pk, pk);
        let long_message = [b'x'; 1000];
        for (message, signature) in [
            (&b""[..], "7f5399e05c8ee45b7c5aae0a247fc0e67c1c2c06c77fbffe0471dcc6747229d37c6b30054f735679fca61ae64a49094bd55435a1358c24e676c856b1a14a320b"),
            (&b"jwt-simple"[..], "86748f8715b5735d08ad9fce53bd6fde97aef51a81e9ec374415b9287c9081e14f7b6c7519a392e553eb13432ce50e7dac3c4b24d490fdcaf63115db4fb6080b"),
            (&long_message[..], "589e5e3a321cabf8288b8bc6e0c9a18a45a356a02ee07c3b2edb502f71d9595b8f7d4dbb8df91f4762fad11ee1490cfd4df4ed20d02948f9c69a084dc209a50b"),
        ] {
            assert_verifies(&pk, message, &Signature::new(hex(signature)));
        }
    }

    #[test]
    fn small_order_public_keys_are_rejected() {
        // R = identity and S = 0 would verify any message with these keys.
        let mut forged = [0u8; 64];
        forged[0] = 1;
        let forged = Signature::new(forged);

        for a in small_order_points() {
            let pk = PublicKey::new(a.to_bytes());
            assert_eq!(pk.validate(), Err(Error::WeakPublicKey));
            assert_rejected(&pk, b"any message", &forged, Error::WeakPublicKey);
        }
    }

    #[test]
    fn noncanonical_public_keys_are_rejected() {
        let mut encodings = vec![];
        // y = p and y = p + 1, with and without the sign bit.
        for low in [0xed, 0xee] {
            let mut e = [0xffu8; 32];
            e[0] = low;
            e[31] = 0x7f;
            encodings.push(e);
            e[31] = 0xff;
            encodings.push(e);
        }
        // x = 0 with the sign bit set, for the identity and the order-2 point.
        let mut e = [0u8; 32];
        e[0] = 1;
        e[31] = 0x80;
        encodings.push(e);
        let mut e = [0xffu8; 32];
        e[0] = 0xec;
        encodings.push(e);
        // Not on the curve.
        let mut e = [0u8; 32];
        e[0] = 2;
        encodings.push(e);

        for e in encodings {
            assert_eq!(PublicKey::new(e).validate(), Err(Error::InvalidPublicKey));
        }
    }

    #[test]
    fn small_order_components_in_r() {
        let kp = KeyPair::from_seed(Seed::new([7u8; 32]));
        let message = b"message";
        let (a, _) = KeyPair::split(&sha512::Hash::hash(&kp.sk[0..32]), false, true);
        let sign_with_r = |r: [u8; 32], nonce: &[u8; 32]| {
            let mut hasher = sha512::Hash::new();
            hasher.update(r);
            hasher.update(&kp.pk[..]);
            hasher.update(message);
            let mut h = hasher.finalize();
            sc_reduce(&mut h);
            let mut signature = [0u8; 64];
            signature[0..32].copy_from_slice(&r);
            sc_muladd(&mut signature[32..64], &h[0..32], &a, nonce);
            Signature::new(signature)
        };
        let mut nonce = [0u8; 32];
        nonce[0] = 42;
        let honest_r = ge_scalarmult_base(&nonce);

        for t in small_order_points() {
            // An R that is off by a small-order point is still accepted.
            let r = (honest_r + t).to_bytes();
            assert_verifies(&kp.pk, message, &sign_with_r(r, &nonce));

            // A small-order R on its own satisfies the equation with a zero nonce.
            // It must be rejected anyway.
            let forged = sign_with_r(t.to_bytes(), &[0u8; 32]);
            assert_rejected(&kp.pk, message, &forged, Error::InvalidSignature);
        }
    }

    #[test]
    fn noncanonical_s_is_rejected() {
        let kp = KeyPair::from_seed(Seed::new([7u8; 32]));
        let mut signature = *kp.sk.sign(b"message", None);
        // S + L
        let l = hex::<32>("edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010");
        let mut carry = 0u16;
        for i in 0..32 {
            let sum = signature[32 + i] as u16 + l[i] as u16 + carry;
            signature[32 + i] = sum as u8;
            carry = sum >> 8;
        }
        assert_eq!(carry, 0);
        assert_rejected(
            &kp.pk,
            b"message",
            &Signature::new(signature),
            Error::NonCanonical,
        );
    }

    #[test]
    fn zero_seeds_and_mismatched_halves() {
        let zero = Seed::new([0u8; 32]);
        assert_eq!(KeyPair::try_from_seed(zero).err(), Some(Error::InvalidSeed));

        let mut raw = [0u8; 64];
        let kp = KeyPair::from_seed(Seed::new([7u8; 32]));
        raw[32..].copy_from_slice(&kp.pk[..]);
        let zero_kp = KeyPair::from_slice(&raw).unwrap();
        assert_eq!(zero_kp.validate(), Err(Error::InvalidSeed));
        assert_eq!(
            zero_kp.sk.validate_public_key(&kp.pk),
            Err(Error::InvalidSeed)
        );

        let other = KeyPair::from_seed(Seed::new([8u8; 32]));
        let mut raw = *kp.sk;
        raw[32..].copy_from_slice(&other.pk[..]);
        let mismatched = KeyPair::from_slice(&raw).unwrap();
        assert_eq!(mismatched.validate(), Err(Error::InvalidPublicKey));

        // A key pair whose public field is right but whose secret key embeds
        // another public key.
        let inconsistent = KeyPair {
            pk: kp.pk,
            sk: SecretKey::new(raw),
        };
        assert_eq!(inconsistent.validate(), Err(Error::InvalidPublicKey));

        assert_eq!(KeyPair::from_slice(&kp.sk[..]).unwrap().validate(), Ok(()));
    }
}
