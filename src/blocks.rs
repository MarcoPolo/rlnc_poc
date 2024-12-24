use curve25519_dalek::constants::RISTRETTO_BASEPOINT_POINT;
use curve25519_dalek::ristretto::RistrettoPoint;
use curve25519_dalek::scalar::Scalar;
use curve25519_dalek::traits::MultiscalarMul;
use rand::Rng;

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
        .chunks_exact(32)
        .map(|x| {
            let mut array = [0u8; 32];
            array.copy_from_slice(x);
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
