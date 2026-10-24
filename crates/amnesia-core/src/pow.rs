//! Issuance-flooding guard: SHA-256(salt || id || nonce) needs `bits` leading zero bits.

use sha2::{Digest, Sha256};

pub fn leading_zero_bits(h: &[u8]) -> u32 {
    let mut n = 0;
    for &b in h {
        if b == 0 {
            n += 8;
        } else {
            return n + b.leading_zeros();
        }
    }
    n
}

pub fn verify(salt: &[u8; 16], id: &[u8; 16], nonce: &[u8], bits: u32) -> bool {
    let h = Sha256::new()
        .chain_update(salt)
        .chain_update(id)
        .chain_update(nonce)
        .finalize();
    leading_zero_bits(&h) >= bits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_bits() {
        assert_eq!(leading_zero_bits(&[0, 0, 0x10]), 19);
        assert_eq!(leading_zero_bits(&[0x80]), 0);
        assert_eq!(leading_zero_bits(&[0, 0]), 16);
    }

    #[test]
    fn finds_and_verifies_low_difficulty() {
        let (salt, id) = ([1u8; 16], [2u8; 16]);
        let nonce = (0u64..)
            .map(|n| n.to_string())
            .find(|n| verify(&salt, &id, n.as_bytes(), 8))
            .unwrap();
        assert!(verify(&salt, &id, nonce.as_bytes(), 8));
        assert!(verify(&salt, &id, b"anything", 0));
    }
}
