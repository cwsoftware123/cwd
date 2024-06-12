use {
    crate::{to_sized, validate_length, CryptoResult, Identity256},
    p256::ecdsa::{signature::DigestVerifier, Signature, VerifyingKey},
};

const SECP256R1_PUBKEY_COMPRESSED_LEN: usize = 33;
const SECP256R1_PUBKEY_UNCOMPRESSED_LEN: usize = 65;
const SECP256R1_SIGNATURE_LEN: usize = 64;

/// NOTE: This function takes the hash of the message, not the prehash.
pub fn secp256r1_verify(msg_hash: &[u8], sig: &[u8], pk: &[u8]) -> CryptoResult<()> {
    let msg = Identity256::from_slice(msg_hash)?;

    let sig = to_sized::<SECP256R1_SIGNATURE_LEN>(sig)?;
    let mut sig = Signature::from_bytes(&sig.into())?;

    // High-S signatures require normalization since our verification implementation
    // rejects them by default. If we had a verifier that does not restrict to
    // low-S only, this step was not needed.
    if let Some(normalized) = sig.normalize_s() {
        sig = normalized;
    }

    validate_length!(
        pk,
        SECP256R1_PUBKEY_COMPRESSED_LEN,
        SECP256R1_PUBKEY_UNCOMPRESSED_LEN
    )?;
    let vk = VerifyingKey::from_sec1_bytes(pk)?;
    vk.verify_digest(msg, &sig).map_err(Into::into)
}

// ----------------------------------- tests -----------------------------------

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::sha256,
        p256::ecdsa::{signature::DigestSigner, Signature, SigningKey},
        rand::rngs::OsRng,
    };

    #[test]
    fn verifying_secp256r1() {
        // generate a valid signature
        let sk = SigningKey::random(&mut OsRng);
        let vk = VerifyingKey::from(&sk);
        let prehash_msg = b"Jake";
        let msg = Identity256::from(sha256(prehash_msg));
        let sig: Signature = sk.sign_digest(msg.clone());

        // valid signature
        assert!(secp256r1_verify(msg.as_bytes(), &sig.to_bytes(), &vk.to_sec1_bytes()).is_ok());

        // incorrect private key
        let false_sk = SigningKey::random(&mut OsRng);
        let false_sig: Signature = false_sk.sign_digest(msg.clone());
        assert!(
            secp256r1_verify(msg.as_bytes(), &false_sig.to_bytes(), &vk.to_sec1_bytes()).is_err()
        );

        // incorrect public key
        let false_sk = SigningKey::random(&mut OsRng);
        let false_vk = VerifyingKey::from(&false_sk);
        assert!(
            secp256r1_verify(msg.as_bytes(), &sig.to_bytes(), &false_vk.to_sec1_bytes()).is_err()
        );

        // incorrect message
        let false_prehash_msg = b"Larry";
        let false_msg = sha256(false_prehash_msg);
        assert!(secp256r1_verify(
            &false_msg,
            &sig.to_bytes(),
            vk.to_encoded_point(true).as_bytes()
        )
        .is_err());
    }
}
