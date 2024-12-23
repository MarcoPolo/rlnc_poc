use crate::blocks::{block_to_chunks, chunk_to_scalars, Committer};
use crate::matrix::Eschelon;
use curve25519_dalek::ristretto::RistrettoPoint;
use curve25519_dalek::traits::MultiscalarMul;
use curve25519_dalek::Scalar;
use rand::Rng;

/*
A Message represents a single chunk that is received by the node.
In production it will also have the BLS signature, which we are removing
to meassure the performance of the RLNC encoding.
*/
#[derive(Clone)]
pub struct Message {
    chunk: Chunk,
    commitments: Vec<RistrettoPoint>,
}
// A Chunk contains the transmitted data. Coefficients are chosen to be u8,
// but since we perform operations in the Ristretto group we have to deal
// With larger integer type. Using u32 we are safe to up to 24 network hops
// Without overflowing.
#[derive(Clone)]
pub struct Chunk {
    data: Vec<Scalar>,
    coefficients: Vec<u32>,
}
/*
A Node keeps chunks and the full commitments from the source. It also keeps two matrices
that help check the linear independence of the chunks and compute the inverse system for
later decoding. The eschelon matrix is the result of the Gaussian elimination and the
transform matrix contains the coefficients to transform the original data into the eschelon
form. Coefficients for random linear combinations are chosen from the u8 type, but the operations
compound at each hop, thus the coefficients that are broadcast in the network are of u32 type.
There should not be any overflows with less than 24 hops. The eschelon matrix and the transform
ones are kept as u64 to avoid overflows during the Gaussian elimination. Notice that all these
operations are done in u64 without any overflows. The final inversion is done in the Ristretto scalar
field. These auxiliary matrices are kept to avoid performing operations on the actual chunks and
dealing with scalar modular operations except when decoding or sending new messages.
*/
pub struct Node<'a> {
    chunks: Vec<Vec<Scalar>>,
    commitments: Vec<RistrettoPoint>,
    eschelon: Eschelon,
    committer: &'a Committer,
}

impl Message {
    pub fn new(chunk: Chunk, commitments: Vec<RistrettoPoint>) -> Self {
        Message { chunk, commitments }
    }

    fn coefficients_to_scalars(&self) -> Vec<Scalar> {
        self.chunk
            .coefficients
            .iter()
            .map(|x| Scalar::from(*x))
            .collect()
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
}

impl<'a> Node<'a> {
    pub fn new(committer: &'a Committer, num_chunks: usize) -> Self {
        Node {
            chunks: Vec::new(),
            commitments: Vec::new(),
            eschelon: Eschelon::new(num_chunks),
            committer: committer,
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
            eschelon: Eschelon::new_identity(num_chunks),
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

    pub fn receive(&mut self, message: Message) -> Result<(), String> {
        // If we have already committments we check that they are the same
        self.check_existing_commitments(&message.commitments)?;
        self.check_existing_chunks(&message.chunk)?;
        message.verify(&self.committer)?;

        // TODO: verify linear independence here
        self.chunks.push(message.chunk.data);
        self.eschelon.add_row(message.chunk.coefficients);
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
        debug_assert_eq!(
            message.chunk.coefficients.len(),
            message.commitments.len()
        );
        debug_assert!(message.verify(&self.committer).is_ok());
        Ok(message)
    }

    fn linear_comb_chunk(&self, scalars: &[u8]) -> Chunk {
        let coefficients = self.eschelon.compound_scalars(scalars);
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

    pub fn chunks(&self) -> &Vec<Vec<Scalar>> {
        &self.chunks
    }

    pub fn commitments(&self) -> &Vec<RistrettoPoint> {
        &self.commitments
    }
}

fn generate_random_coeffs(length: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    (0..length).map(|_| rng.gen()).collect()
}

#[cfg(test)]
mod tests {
    use crate::blocks::{random_u8_slice, Committer};
    use crate::node::Node;

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
        destination_node.receive(message).unwrap();
        assert_eq!(destination_node.chunks().len(), 1);
        assert_eq!(destination_node.commitments().len(), num_chunks);
    }
}
