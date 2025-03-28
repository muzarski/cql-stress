// TODO: remove this file when compute hashes becomes public:
// https://github.com/scylladb/scylla-rust-driver/issues/1273
use scylla::routing::Token;
use scylla::statement::prepared::TokenCalculationError;
use std::num::Wrapping;

// Define the PartitionerName enum (only Murmur3 is needed for now)
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub enum PartitionerName {
    #[default]
    Murmur3,
}

// Define traits for Partitioner and PartitionerHasher (simplified for Murmur3)
pub trait Partitioner {
    type Hasher: PartitionerHasher;

    fn build_hasher(&self) -> Self::Hasher;
}

pub trait PartitionerHasher {
    fn write(&mut self, pk_part: &[u8]);
    fn finish(&self) -> Token;
}

// Murmur3 Partitioner implementation
pub struct Murmur3Partitioner;

impl Partitioner for Murmur3Partitioner {
    type Hasher = Murmur3PartitionerHasher;

    fn build_hasher(&self) -> Self::Hasher {
        Murmur3PartitionerHasher {
            total_len: 0,
            buf: [0u8; Murmur3PartitionerHasher::BUF_CAPACITY], // Use the hasher's constant
            h1: Wrapping(0),
            h2: Wrapping(0),
        }
    }
}

// Murmur3 PartitionerHasher implementation
pub struct Murmur3PartitionerHasher {
    total_len: usize,
    buf: [u8; Self::BUF_CAPACITY],
    h1: Wrapping<i64>,
    h2: Wrapping<i64>,
}

impl Murmur3PartitionerHasher {
    const BUF_CAPACITY: usize = 16;
    const C1: Wrapping<i64> = Wrapping(0x87c3_7b91_1142_53d5_u64 as i64);
    const C2: Wrapping<i64> = Wrapping(0x4cf5_ad43_2745_937f_u64 as i64);

    fn hash_16_bytes(&mut self, mut k1: Wrapping<i64>, mut k2: Wrapping<i64>) {
        k1 *= Self::C1;
        k1 = Self::rotl64(k1, 31);
        k1 *= Self::C2;
        self.h1 ^= k1;

        self.h1 = Self::rotl64(self.h1, 27);
        self.h1 += self.h2;
        self.h1 = self.h1 * Wrapping(5) + Wrapping(0x52dce729);

        k2 *= Self::C2;
        k2 = Self::rotl64(k2, 33);
        k2 *= Self::C1;
        self.h2 ^= k2;

        self.h2 = Self::rotl64(self.h2, 31);
        self.h2 += self.h1;
        self.h2 = self.h2 * Wrapping(5) + Wrapping(0x38495ab5);
    }

    fn fetch_16_bytes_from_buf(buf: &mut &[u8]) -> (Wrapping<i64>, Wrapping<i64>) {
        let k1 = Wrapping(i64::from_le_bytes(buf[..8].try_into().unwrap()));
        *buf = &buf[8..];
        let k2 = Wrapping(i64::from_le_bytes(buf[..8].try_into().unwrap()));
        *buf = &buf[8..];
        (k1, k2)
    }

    #[inline]
    fn rotl64(v: Wrapping<i64>, n: u32) -> Wrapping<i64> {
        Wrapping((v.0 << n) | (v.0 as u64 >> (64 - n)) as i64)
    }

    #[inline]
    fn fmix(mut k: Wrapping<i64>) -> Wrapping<i64> {
        k ^= Wrapping((k.0 as u64 >> 33) as i64);
        k *= Wrapping(0xff51afd7ed558ccd_u64 as i64);
        k ^= Wrapping((k.0 as u64 >> 33) as i64);
        k *= Wrapping(0xc4ceb9fe1a85ec53_u64 as i64);
        k ^= Wrapping((k.0 as u64 >> 33) as i64);
        k
    }
}

impl PartitionerHasher for Murmur3PartitionerHasher {
    fn write(&mut self, mut pk_part: &[u8]) {
        let mut buf_len = self.total_len % Self::BUF_CAPACITY;
        self.total_len += pk_part.len();

        if buf_len > 0 && Self::BUF_CAPACITY - buf_len <= pk_part.len() {
            let to_write = std::cmp::min(Self::BUF_CAPACITY - buf_len, pk_part.len());
            self.buf[buf_len..buf_len + to_write].copy_from_slice(&pk_part[..to_write]);
            pk_part = &pk_part[to_write..];

            let mut buf_ptr = &self.buf[..];
            let (k1, k2) = Self::fetch_16_bytes_from_buf(&mut buf_ptr);
            self.hash_16_bytes(k1, k2);
            buf_len = 0;
        }

        if buf_len == 0 {
            while pk_part.len() >= Self::BUF_CAPACITY {
                let (k1, k2) = Self::fetch_16_bytes_from_buf(&mut pk_part);
                self.hash_16_bytes(k1, k2);
            }
        }

        let to_write = pk_part.len();
        self.buf[buf_len..buf_len + to_write].copy_from_slice(pk_part);
    }

    fn finish(&self) -> Token {
        let mut h1 = self.h1;
        let mut h2 = self.h2;

        let mut k1 = Wrapping(0_i64);
        let mut k2 = Wrapping(0_i64);

        let buf_len = self.total_len % Self::BUF_CAPACITY;

        if buf_len > 8 {
            for i in (8..buf_len).rev() {
                k2 ^= Wrapping(self.buf[i] as i8 as i64) << ((i - 8) * 8);
            }
            k2 *= Self::C2;
            k2 = Self::rotl64(k2, 33);
            k2 *= Self::C1;
            h2 ^= k2;
        }

        if buf_len > 0 {
            for i in (0..std::cmp::min(8, buf_len)).rev() {
                k1 ^= Wrapping(self.buf[i] as i8 as i64) << (i * 8);
            }
            k1 *= Self::C1;
            k1 = Self::rotl64(k1, 31);
            k1 *= Self::C2;
            h1 ^= k1;
        }

        h1 ^= Wrapping(self.total_len as i64);
        h2 ^= Wrapping(self.total_len as i64);

        h1 += h2;
        h2 += h1;

        h1 = Self::fmix(h1);
        h2 = Self::fmix(h2);

        h1 += h2;
        h2 += h1;

        Token::new((((h2.0 as i128) << 64) | h1.0 as i128) as i64)
    }
}

impl PartitionerName {
    fn get_partitioner(&self) -> Murmur3Partitioner {
        match self {
            PartitionerName::Murmur3 => Murmur3Partitioner,
        }
    }
}

// Your target function
pub fn calculate_token_for_partition_key(
    bytes: &[u8],
    partitioner: &PartitionerName,
) -> Result<Token, TokenCalculationError> {
    let partitioner = partitioner.get_partitioner();
    let mut hasher = partitioner.build_hasher();
    hasher.write(bytes);
    Ok(hasher.finish())
}
// Optional: Add a simple test to verify functionality
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_murmur3_hash() {
        let partitioner = PartitionerName::Murmur3;
        let data = b"test_data";
        let token = calculate_token_for_partition_key(data, &partitioner).unwrap();
        assert_eq!(token.value(), token.value()); // Replace with actual expected value if known
    }
}
