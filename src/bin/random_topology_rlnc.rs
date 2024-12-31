use rlnc_poc::blocks::{random_u8_slice, Committer};
use rlnc_poc::node::{Message, Node, ReceiveError};

fn main() {
    run_simulation();
}

struct SimulationNode<'a> {
    node: Node<'a>,
    neighbors: Vec<usize>,
    sent_message: bool,
}
struct Network<'a> {
    nodes: Vec<SimulationNode<'a>>,
    timestamp: u32,
    wasted_bandwdidth: u32,
    full_nodes: usize,
    round_messages: Vec<Message>,
    round_destinations: Vec<usize>,
}

impl<'a> SimulationNode<'a> {
    fn new(committer: &'a Committer, num_chunks: usize) -> Self {
        SimulationNode {
            node: Node::<'a>::new(&committer, num_chunks),
            neighbors: Vec::new(),
            sent_message: false,
        }
    }

    fn new_source(
        committer: &'a Committer,
        block: &[u8],
        num_chunks: usize,
    ) -> Result<Self, String> {
        let node = Node::new_source(&committer, block, num_chunks)?;
        Ok(SimulationNode {
            node,
            neighbors: Vec::new(),
            sent_message: false,
        })
    }
}
impl<'a> Network<'a> {
    fn create_nodes(
        committer: &'a Committer,
        num: usize,
        num_chunks: usize,
        mesh_size: usize,
        block: &[u8],
    ) -> Vec<SimulationNode<'a>> {
        let mut ret: Vec<SimulationNode> = Vec::with_capacity(num);
        let source_node =
            SimulationNode::new_source(&committer, block, num_chunks).unwrap();
        ret.push(source_node);
        for _ in 1..num {
            ret.push(SimulationNode::new(&committer, num_chunks));
        }
        for i in 0..num {
            let mut neighbors: Vec<usize> = Vec::with_capacity(mesh_size);
            for _ in 0..mesh_size {
                neighbors.push(rand::random::<usize>() % num);
            }
            ret[i].neighbors = neighbors;
        }
        return ret;
    }
    pub fn new(
        committer: &'a Committer,
        num_nodes: usize,
        mesh_size: usize,
    ) -> Self {
        let num_chunks = 10;
        let nodes = Network::create_nodes(
            &committer,
            num_nodes,
            num_chunks,
            mesh_size,
            &random_u8_slice(committer.len() * num_chunks * 32),
        );
        Network {
            nodes,
            timestamp: 0,
            wasted_bandwdidth: 0,
            full_nodes: 1,
            round_destinations: Vec::new(),
            round_messages: Vec::new(),
        }
    }

    pub fn round(&mut self) {
        self.timestamp += 1;
        self.round_messages.clear();
        self.round_destinations.clear();
        for i in 0..self.nodes.len() {
            let source = &mut self.nodes[i];
            if source.sent_message {
                continue;
            }
            for &j in source.neighbors.iter() {
                if j == i {
                    continue;
                }
                if let Ok(message) = source.node.send() {
                    source.sent_message = true;
                    self.round_messages.push(message);
                    self.round_destinations.push(j);
                }
            }
        }
        self.round_messages
            .iter()
            .zip(self.round_destinations.iter())
            .for_each(|(message, &j)| {
                let destination = &mut self.nodes[j];
                match destination.node.receive(message.clone()) {
                    Ok(_) => {
                        if destination.node.is_full() {
                            self.full_nodes += 1;
                        }
                    }
                    Err(ReceiveError::LinearlyDependentChunk) => {
                        self.wasted_bandwdidth += 1;
                    }
                    Err(e) => {
                        panic!("Unhandled error: {:?}", e);
                    }
                }
            });
    }

    pub fn all_nodes_full(&self) -> bool {
        self.full_nodes == self.nodes.len()
    }
}

fn run_simulation() {
    let num_nodes = 10000; // Similar to Ethereum mainnet
    let chunk_size = 1;
    let committer = Committer::new(chunk_size);
    let mesh_size = 60;
    let mut network = Network::new(&committer, num_nodes, mesh_size);
    while !network.all_nodes_full() && network.timestamp < 100 {
        network.round();
        println!(
            "Timestamp: {}, Full nodes: {}, Wasted Bandwidth: {}",
            network.timestamp, network.full_nodes, network.wasted_bandwdidth
        );
    }
}
