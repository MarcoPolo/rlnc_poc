use crate::blocks::{
    block_to_chunks, chunk_to_scalars, scalars_to_chunk, Committer,
};
use crate::matrix::Echelon;
use curve25519_dalek::ristretto::RistrettoPoint;
use curve25519_dalek::traits::MultiscalarMul;
use curve25519_dalek::Scalar;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/*
A Message represents a single chunk that is received by the node.
In production it will also have the BLS signature, which we are removing
to meassure the performance of the RLNC encoding.
*/
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    chunk: Chunk,
    commitments: Vec<RistrettoPoint>,
}
// A Chunk contains the transmitted data. Coefficients are also in the Ristretto group
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chunk {
    data: Vec<Scalar>,
    coefficients: Vec<Scalar>,
}
/*
A Node keeps chunks and the full commitments from the source. The Echelon object is used to keep
track of the linear independence of the chunks.
*/
pub struct Node<'a> {
    chunks: Vec<Vec<Scalar>>,
    commitments: Vec<RistrettoPoint>,
    echelon: Echelon,
    committer: &'a Committer,
}

#[derive(Debug)]
pub enum ReceiveError {
    ExistingCommitmentsMismatch(String),
    ExistingChunksMismatch(String),
    InvalidMessage(String),
    LinearlyDependentChunk,
}

impl Message {
    pub fn new(chunk: Chunk, commitments: Vec<RistrettoPoint>) -> Self {
        Message { chunk, commitments }
    }

    fn coefficients_to_scalars(&self) -> Vec<Scalar> {
        self.chunk.coefficients.to_vec()
    }

    pub fn verify(&self, committer: &Committer) -> Result<(), String> {
        let msm = RistrettoPoint::multiscalar_mul(
            self.coefficients_to_scalars(),
            &self.commitments,
        );
        let commitment = committer.commit(&self.chunk.data)?;
        if msm != commitment {
            return Err("The commitment does not match".to_string());
        }
        Ok(())
    }

    pub fn coefficients(&self) -> &Vec<Scalar> {
        &self.chunk.coefficients
    }

    pub fn commitments_hash(&self) -> [u8; 32] {
        let mut hasher = Sha256::new();
        let serialized = bincode::serialize(&self.commitments).unwrap();
        hasher.update(&serialized);
        hasher.finalize().into()
    }
}

impl<'a> Node<'a> {
    pub fn new(committer: &'a Committer, num_chunks: usize) -> Self {
        Node {
            chunks: Vec::new(),
            commitments: Vec::new(),
            echelon: Echelon::new(num_chunks),
            committer,
        }
    }
    pub fn new_source(
        committer: &'a Committer,
        block: &[u8],
        num_chunks: usize,
    ) -> Result<Self, String> {
        let chunks: Vec<_> = block_to_chunks(block, num_chunks)?
            .into_iter()
            .map(|data| chunk_to_scalars(data).unwrap())
            .collect();
        let commitments = chunks
            .iter()
            .map(|chunk| committer.commit(&chunk).unwrap())
            .collect();
        Ok(Node {
            chunks,
            commitments,
            echelon: Echelon::new_identity(num_chunks),
            committer,
        })
    }

    fn check_existing_commitments(
        &self,
        commitments: &[RistrettoPoint],
    ) -> Result<(), String> {
        if !self.commitments.is_empty() {
            if self.commitments.len() != commitments.len() {
                return Err(
                    "The number of commitments is different".to_string()
                );
            }
            if self.commitments != commitments {
                return Err("The commitments do not match".to_string());
            }
        }
        Ok(())
    }

    fn check_existing_chunks(&self, chunk: &Chunk) -> Result<(), String> {
        if !self.chunks.is_empty() {
            if self.chunks[0].len() != chunk.data.len() {
                return Err("The chunk size is different".to_string());
            }
        }
        Ok(())
    }

    pub fn receive(&mut self, message: Message) -> Result<(), ReceiveError> {
        // If we have already committments we check that they are the same
        self.check_existing_commitments(&message.commitments)
            .map_err(ReceiveError::ExistingCommitmentsMismatch)?;

        self.check_existing_chunks(&message.chunk)
            .map_err(ReceiveError::ExistingChunksMismatch)?;

        message
            .verify(&self.committer)
            .map_err(ReceiveError::InvalidMessage)?;

        // Verify linear independence
        if !self.echelon.add_row(message.chunk.coefficients) {
            return Err(ReceiveError::LinearlyDependentChunk);
        }

        self.chunks.push(message.chunk.data);
        if self.commitments.is_empty() {
            self.commitments = message.commitments;
        }
        Ok(())
    }

    pub fn send(&self) -> Result<Message, String> {
        if self.chunks.is_empty() {
            return Err("There are no chunks to send".to_string());
        }
        let scalars = generate_random_coeffs(self.chunks.len());
        let chunk = self.linear_comb_chunk(&scalars);

        let message = Message::new(chunk, self.commitments.clone());
        debug_assert!(message.verify(&self.committer).is_ok());
        Ok(message)
    }

    fn linear_comb_chunk(&self, scalars: &[u8]) -> Chunk {
        let coefficients = self.echelon.compound_scalars(scalars);
        let data = self.linear_comb_data(scalars);
        Chunk { data, coefficients }
    }

    fn linear_comb_data(&self, scalars: &[u8]) -> Vec<Scalar> {
        (0..self.chunks[0].len())
            .map(|i| {
                scalars
                    .iter()
                    .zip(&self.chunks)
                    .map(|(&x, chunk)| Scalar::from(x) * chunk[i])
                    .sum()
            })
            .collect()
    }

    pub fn decode(&self) -> Result<Vec<u8>, String> {
        let inverse = self.echelon.inverse()?;
        let mut ret: Vec<u8> = Vec::with_capacity(
            self.commitments.len() * self.chunks[0].len() * 32,
        );

        for i in 0..inverse.len() {
            let mut ret_scalars = Vec::with_capacity(
                self.commitments.len() * self.chunks[0].len(),
            );
            for k in 0..self.chunks[0].len() {
                ret_scalars.push(
                    (0..inverse.len())
                        .map(|j| inverse[i][j] * self.chunks[j][k])
                        .sum::<Scalar>(),
                );
            }

            ret.extend_from_slice(&scalars_to_chunk(&ret_scalars)?);
        }

        Ok(ret)
    }

    pub fn chunks(&self) -> &Vec<Vec<Scalar>> {
        &self.chunks
    }

    pub fn commitments(&self) -> &Vec<RistrettoPoint> {
        &self.commitments
    }

    pub fn is_full(&self) -> bool {
        self.echelon.is_full()
    }
}

fn generate_random_coeffs(length: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    (0..length).map(|_| rng.gen()).collect()
}

#[cfg(test)]
mod tests {
    use rand::RngCore;

    use crate::blocks::{random_u8_slice, Committer};
    use crate::node::{Node, ReceiveError};

    #[test]
    fn test_source_node() {
        let num_chunks = 3;
        let chunk_size = 4;
        let committer = Committer::new(chunk_size);
        let block = random_u8_slice(num_chunks * chunk_size * 32);
        let source_node =
            Node::new_source(&committer, &block, num_chunks).unwrap();
        assert_eq!(source_node.chunks().len(), num_chunks);
        assert_eq!(source_node.commitments().len(), num_chunks);
    }

    #[macro_export]
    macro_rules! measure_time {
        ($prefix:expr, $expr:expr) => {{
            use std::time::Instant;
            let start = Instant::now();
            let result = $expr;
            let duration = start.elapsed();
            println!("{}: {:?}", $prefix, duration);
            result
        }};
    }

    #[test]
    fn test_roundtrip() {
        let num_chunks = 8;
        // let chunk_size = 16 * 1024;
        let chunk_size = 2048;
        let mut block = vec![0; num_chunks * chunk_size];
        rand::thread_rng().fill_bytes(&mut block);
        for i in (31..block.len()).step_by(32) {
            block[i] = 0;
        }
        let committer = measure_time!(
            "gen commiter",
            // Each scalar represents 252 bits. We add 251 to round up the result, since if we need
            // 1.1 scalars we need 2.
            Committer::new((chunk_size * 8 + 251) / 252)
        );
        let source_node =
            Node::new_source(&committer, &block, num_chunks).unwrap();
        assert_eq!(source_node.chunks().len(), num_chunks);
        assert_eq!(source_node.commitments().len(), num_chunks);

        let mut destination_node = measure_time!(
            "build destination node",
            Node::new(&committer, num_chunks)
        );

        for _ in 0..num_chunks {
            let message =
                measure_time!("gen send chunk", source_node.send().unwrap());
            measure_time!(
                "receive chunk",
                destination_node
                    .receive(message)
                    .or_else(|e| match e {
                        ReceiveError::LinearlyDependentChunk => Ok(()),
                        _ => Err(e),
                    })
                    .unwrap()
            );
        }

        assert!(destination_node.is_full());

        let decoded = destination_node.decode().unwrap();

        assert_eq!(decoded.len(), block.len());
        for i in 0..decoded.len() {
            assert_eq!(
                decoded[i], block[i],
                "Failed to match at idx {} left {} right {}",
                i, decoded[i], block[i]
            );
        }
        // assert_eq!(decoded, block);
    }

    #[test]
    fn test_send_receive() {
        let num_chunks = 3;
        let chunk_size = 4;
        let committer = Committer::new(chunk_size);
        let block = random_u8_slice(num_chunks * chunk_size * 32);
        let source_node =
            Node::new_source(&committer, &block, num_chunks).unwrap();
        let message = source_node.send().unwrap();
        let mut destination_node = Node::new(&committer, num_chunks);
        destination_node
            .receive(message)
            .or_else(|e| match e {
                ReceiveError::LinearlyDependentChunk => Ok(()),
                _ => Err(e),
            })
            .unwrap();
        assert_eq!(destination_node.chunks().len(), 1);
        assert_eq!(destination_node.commitments().len(), num_chunks);

        destination_node.send().unwrap();
    }

    #[test]
    fn test_decode() {
        let num_chunks = 3;
        let chunk_size = 4;
        let committer = Committer::new(chunk_size);
        let block = random_u8_slice(num_chunks * chunk_size * 32);
        let source_node =
            Node::new_source(&committer, &block, num_chunks).unwrap();
        let message1 = source_node.send().unwrap();
        let message2 = source_node.send().unwrap();
        let message3 = source_node.send().unwrap();
        let mut destination_node = Node::new(&committer, num_chunks);
        destination_node
            .receive(message1)
            .or_else(|e| match e {
                ReceiveError::LinearlyDependentChunk => Ok(()),
                _ => Err(e),
            })
            .unwrap();
        destination_node
            .receive(message2)
            .or_else(|e| match e {
                ReceiveError::LinearlyDependentChunk => Ok(()),
                _ => Err(e),
            })
            .unwrap();
        destination_node
            .receive(message3)
            .or_else(|e| match e {
                ReceiveError::LinearlyDependentChunk => Ok(()),
                _ => Err(e),
            })
            .unwrap();
        let decoded = destination_node.decode().unwrap();
        assert_eq!(decoded.len(), block.len());
        assert_eq!(decoded, block);
    }

    #[test]
    fn test_message_serialization() {
        use super::Message;
        // Setup
        let num_chunks = 3;
        let chunk_size = 4;
        let committer = Committer::new(chunk_size);
        let block = random_u8_slice(num_chunks * chunk_size * 32);

        // Create a source node and get a message
        let source_node =
            Node::new_source(&committer, &block, num_chunks).unwrap();
        let original_message = source_node.send().unwrap();

        // Serialize to bytes
        let serialized = bincode::serialize(&original_message).unwrap();

        // Deserialize back
        let deserialized_message: Message =
            bincode::deserialize(&serialized).unwrap();

        // Verify the deserialized message
        assert_eq!(
            original_message.chunk.data, deserialized_message.chunk.data,
            "Data vectors don't match"
        );
        assert_eq!(
            original_message.chunk.coefficients,
            deserialized_message.chunk.coefficients,
            "Coefficients don't match"
        );
        assert_eq!(
            original_message.commitments, deserialized_message.commitments,
            "Commitments don't match"
        );

        // Verify the deserialized message can still be verified
        assert!(deserialized_message.verify(&committer).is_ok());
    }
}
