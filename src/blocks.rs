use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT;
use curve25519_dalek::ristretto::RistrettoPoint;
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::traits::MultiscalarMul;
use rand::Rng;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Committer {
    generators: Vec<RistrettoPoint>,
}

impl Committer {
    pub fn new(n: usize) -> Self {
        Committer {
            generators: generators(n),
        }
    }

    pub fn len(&self) -> usize {
        self.generators.len()
    }

    pub fn commit(&self, scalars: &[Scalar]) -> Result<RistrettoPoint, String> {
        if scalars.len() > self.generators.len() {
            println!(
                "Chunk size is too large. Expected {}, got {}",
                self.generators.len(),
                scalars.len()
            );
            return Err("Chunk size is too large".to_string());
        }
        Ok(RistrettoPoint::multiscalar_mul(
            scalars,
            &self.generators[..scalars.len()],
        ))
    }
}

// TODO: read the points from file instead of computing them at runtime
fn generators(n: usize) -> Vec<RistrettoPoint> {
    let mut rng = rand::thread_rng();
    (0..n)
        .map(|_| RISTRETTO_BASEPOINT_POINT * Scalar::from(rng.gen::<u128>()))
        .collect()
}

// chunk_to_scalars returns a vector of scalars in the Ristretto curve from the
// given array, it works modulo the characteristic of the Ristretto Scalar field.
// In real life blocks need to be encoded by bitpacking so that each 256 bits have
// the last couple of them zeroed.
pub fn chunk_to_scalars(chunk: &[u8]) -> Result<Vec<Scalar>, String> {
    if chunk.len() % 32 != 0 {
        return Err("Chunk size is not divisible by 32".to_string());
    }
    Ok(chunk
        .chunks(63 * 32)
        .map(|x| {
            let mut tail_bits = [0u8; 32];
            let mut scalars: Vec<Scalar> = x
                .chunks_exact(32)
                .enumerate()
                .map(|(i, x)| {
                    let mut array = [0u8; 32];
                    array.copy_from_slice(x);
                    // Store high 4 bits in tail_bits
                    // Each byte in tail_bits can store 2 high-4-bit values
                    let high_bits = array[31] >> 4;
                    tail_bits[i >> 1] |= high_bits << (4 * (i & 1));
                    array[31] &= 0x0F;
                    Scalar::from_bytes_mod_order(array)
                })
                .collect();

            scalars.push(Scalar::from_bytes_mod_order(tail_bits));
            scalars
        })
        .flatten()
        .collect())
}

pub fn chunk_to_scalars_31(chunk: &[u8]) -> Result<Vec<Scalar>, String> {
    if chunk.len() % 31 != 0 {
        return Err(format!(
            "Chunk size is not divisible by 31. It is {}",
            chunk.len()
        ));
    }
    Ok(chunk
        .chunks_exact(31)
        .map(|x| {
            let mut array = [0u8; 32];
            array[0..31].copy_from_slice(x);
            Scalar::from_bytes_mod_order(array)
        })
        .collect())
}

// random_u8_slice returns a vector of random u32 numbers of the given length.
pub fn random_u8_slice(length: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut ret: Vec<u8> = (0..length).map(|_| rng.gen()).collect();
    for i in (31..length).step_by(32) {
        ret[i] = 0;
    }
    ret
}

pub fn block_to_chunks(
    block: &[u8],
    num_chunks: usize,
) -> Result<Vec<&[u8]>, String> {
    if block.len() % num_chunks != 0 {
        return Err("Block size is not divisible by num_chunks".to_string());
    }
    let chunk_size = block.len() / num_chunks;
    Ok(block.chunks(chunk_size).collect())
}

// scalars_to_chunk converts a vector of Scalars back into bytes, reversing the
// transformation done by chunk_to_scalars. It extracts the tail bits from the last
// scalar of each 255-scalar chunk and combines them with the main bytes.
pub fn scalars_to_chunk(scalars: &[Scalar]) -> Result<Vec<u8>, String> {
    if scalars.is_empty() {
        return Ok(Vec::new());
    }

    // Each chunk of 64 scalars represents 63*32 bytes (the last scalar contains tail bits)
    let chunk_size = 64;

    // Calculate total bytes needed: each chunk of 64 scalars produces 63*32 bytes
    let full_chunks = scalars.len() / chunk_size;
    let remaining_scalars = scalars.len() % chunk_size;
    let capacity = (full_chunks * 63 * 32)
        + (if remaining_scalars > 1 {
            (remaining_scalars - 1) * 32
        } else {
            0
        });

    let mut result = Vec::with_capacity(capacity);

    for chunk in scalars.chunks(chunk_size) {
        if chunk.len() <= 1 {
            return Err("Invalid scalar chunk size: each chunk must have enough scalars to contain data and tail bits".to_string());
        }

        // The last scalar in the chunk contains the tail bits
        let tail_bits = chunk.last().unwrap().to_bytes();

        // Process all scalars except the last one (which contains tail bits)
        for (i, scalar) in chunk[..chunk.len() - 1].iter().enumerate() {
            let mut bytes = scalar.to_bytes();
            // Restore the high 4 bits from tail_bits
            let high_bits = (tail_bits[i >> 1] >> (4 * (i & 1))) & 0x0F;
            bytes[31] |= high_bits << 4;
            result.extend_from_slice(&bytes);
        }
    }

    Ok(result)
}

pub fn scalars_to_chunk_31(scalars: &[Scalar]) -> Vec<u8> {
    scalars
        .iter()
        .flat_map(|scalar| {
            // Convert scalar to 32 bytes
            let bytes = scalar.to_bytes();
            // Take only first 31 bytes since we know the last byte is always 0
            bytes[..31].to_vec()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use rand::thread_rng;

    use super::*;

    #[test]
    fn test_roundtrip_chunk_conversion() {
        // Test with one chunk (63*32 bytes) and multiple chunks
        let test_sizes = vec![32, 63 * 32, 63 * 32 * 2, 63 * 32 * 8];

        for size in test_sizes {
            let mut original = vec![0u8; size];
            thread_rng().fill(&mut original[..]);
            original[31] &= 0x0F;

            // Convert to scalars and back
            let scalars = chunk_to_scalars(&original).unwrap();
            let result = scalars_to_chunk(&scalars).unwrap();

            assert_eq!(original.len(), result.len());
            assert_eq!(
                original, result,
                "Failed roundtrip test for size {}",
                size
            );
        }
    }

    #[test]
    fn test_roundtrip_chunk_31_conversion() {
        let test_sizes = vec![31, 31 * 2, 31 * 8];

        for size in test_sizes {
            let mut original = vec![0u8; size];
            thread_rng().fill(&mut original[..]);

            // Convert to scalars and back
            let scalars = chunk_to_scalars_31(&original).unwrap();
            let result = scalars_to_chunk_31(&scalars);

            assert_eq!(original.len(), result.len());
            assert_eq!(
                original, result,
                "Failed roundtrip test for size {}",
                size
            );
        }
    }
}
