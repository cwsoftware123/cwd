use {
    anyhow::ensure,
    digest::{
        consts::U32, generic_array::GenericArray, FixedOutput, HashMarker, OutputSizeUser, Update,
    },
};

/// Hash the given message using BLAKE3 hash function, return the result hash as
/// an Identity256.
#[cfg(test)]
pub(crate) fn hash(data: &[u8]) -> Identity256 {
    let mut digest = Identity256::default();
    digest.update(blake3::hash(data).as_bytes());
    digest
}

/// To utilize the `signature::DigestVerifier::verify_digest` method, the digest
/// must implement the `digest::Digest` trait, which in turn requires the
/// following traits:
///
/// - Default
/// - OutputSizeUser
/// - Update
/// - FixedOutput
/// - HashMarker
///
/// Here we define a container struct that implements the required traits.
///
/// Adapted from:
/// https://github.com/CosmWasm/cosmwasm/blob/main/packages/crypto/src/identity_digest.rs
#[derive(Default, Clone)]
pub struct Identity256 {
    bytes: GenericArray<u8, U32>,
}

impl Identity256 {
    pub fn from_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        ensure!(bytes.len() == 32, "[Identity256]: message is not exactly 32 bytes");
        Ok(Self {
            bytes: *GenericArray::from_slice(bytes),
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl OutputSizeUser for Identity256 {
    type OutputSize = U32;
}

impl Update for Identity256 {
    fn update(&mut self, data: &[u8]) {
        assert_eq!(data.len(), 32);
        self.bytes = *GenericArray::from_slice(data);
    }
}

impl FixedOutput for Identity256 {
    fn finalize_into(self, out: &mut digest::Output<Self>) {
        *out = self.bytes
    }
}

impl HashMarker for Identity256 {}
