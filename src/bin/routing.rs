fn main() {
    run_simulation();
}

struct Node {
    neighbors: Vec<usize>,
    full: bool,
}

struct Network {
    nodes: Vec<Node>,
    timestamp: u32,
    wasted_bandwdidth: u32,
    full_nodes: usize,
}

impl Network {
    fn create_nodes(num_nodes: usize, mesh_size: usize) -> Vec<Node> {
        let mut ret: Vec<Node> = Vec::with_capacity(num_nodes);
        for _ in 0..num_nodes {
            let mut neighbors: Vec<usize> = Vec::with_capacity(mesh_size);
            for _ in 0..mesh_size {
                neighbors.push(rand::random::<usize>() % num_nodes);
            }
            ret.push(Node {
                neighbors,
                full: false,
            });
        }
        ret[0].full = true;
        return ret;
    }

    pub fn new(num_nodes: usize, mesh_size: usize) -> Self {
        let nodes = Network::create_nodes(num_nodes, mesh_size);
        Network {
            nodes,
            timestamp: 0,
            wasted_bandwdidth: 0,
            full_nodes: 1,
        }
    }

    pub fn round(&mut self) {
        self.timestamp += 10;
        let mut round_destinations: Vec<usize> = Vec::new();
        println!(
            "Timestamp: {}, Full nodes: {}, Wasted Bandwidth: {}",
            self.timestamp, self.full_nodes, self.wasted_bandwdidth
        );

        for i in 0..self.nodes.len() {
            let source = &self.nodes[i];
            if !source.full {
                continue;
            }
            for j in &source.neighbors {
                if *j == i {
                    continue;
                }
                if self.nodes[*j].full {
                    self.wasted_bandwdidth += 10;
                } else {
                    round_destinations.push(*j);
                }
            }
        }
        for j in round_destinations {
            if self.nodes[j].full {
                continue;
            }
            self.nodes[j].full = true;
            self.full_nodes += 1;
        }
    }

    pub fn full_nodes(&self) -> usize {
        self.full_nodes
    }
}

fn run_simulation() {
    let num_nodes = 10000; // Similar to Ethereum mainnet
    let mesh_size = 6;
    let mut network = Network::new(num_nodes, mesh_size);
    while network.full_nodes() < num_nodes * 99 / 100 {
        network.round();
    }
}
