use rand::{Rng, distributions::Alphanumeric};
use sha2::{Digest, Sha256};

pub fn solve_pow(prefix: &[u8], difficulty: u8) -> Vec<u8> {
    loop {
        let nonce: String = rand::thread_rng()
            .sample_iter(&Alphanumeric)
            .take(16)
            .map(char::from)
            .collect();
        let mut hasher = Sha256::new();
        hasher.update(prefix);
        hasher.update(nonce.as_bytes());
        let mut hash = hasher.finalize().to_vec();
        hash.reverse();

        let mut num = 0u128;
        for byte in &hash {
            num = (num << 8) | *byte as u128;
        }

        if num & ((1 << difficulty) - 1) == 0 {
            return nonce.as_bytes().to_vec();
        }
    }
}

use rayon::iter::IntoParallelIterator;
use rayon::iter::ParallelIterator;
pub fn solve_pow_parallel(prefix: &[u8], difficulty: u8) -> Vec<u8> {
    let nonce: u128 = (0..=u128::MAX)
        .into_par_iter()
        .filter(|nonce| {
            let mut hasher = Sha256::new();
            hasher.update(prefix);
            hasher.update(nonce.to_le_bytes());
            let mut hash = hasher.finalize().to_vec();
            hash.reverse();

            let mut num = 0u128;
            for byte in &hash {
                num = (num << 8) | *byte as u128;
            }

            num & ((1 << difficulty) - 1) == 0
        })
        .take_any(1)
        .sum();

    nonce.to_le_bytes().into()
}
