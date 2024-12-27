use criterion::{black_box, criterion_group, criterion_main, Criterion};
use rlnc_poc::blocks::{
    block_to_chunks, chunk_to_scalars, random_u8_slice, Committer,
};
use rlnc_poc::node::{Message, Node, ReceiveError};

fn benchmark_commit(c: &mut Criterion) {
    let chunk_size = 1;
    let num_chunks = 10;
    let block_size = chunk_size * num_chunks * 32;
    let block: Vec<u8> = random_u8_slice(block_size);
    let committer = Committer::new(chunk_size);
    let chunks = block_to_chunks(&block, num_chunks).unwrap();
    c.bench_function("commit small block", |b| {
        b.iter(|| {
            for chunk in &chunks {
                black_box(
                    committer
                        .commit(&chunk_to_scalars(chunk).unwrap())
                        .unwrap(),
                );
            }
        })
    });

    let large_chunk_size = 380;
    let large_num_chunks = 10;
    let large_block_size = large_chunk_size * large_num_chunks * 32;
    let large_block: Vec<u8> = random_u8_slice(large_block_size);
    let committer = Committer::new(large_chunk_size);
    let large_chunks = block_to_chunks(&large_block, large_num_chunks).unwrap();
    c.bench_function("commit large block", |b| {
        b.iter(|| {
            for chunk in &large_chunks {
                black_box(
                    committer
                        .commit(&chunk_to_scalars(chunk).unwrap())
                        .unwrap(),
                );
            }
        })
    });
}

fn benchmark_send_receive(c: &mut Criterion) {
    let chunk_size = 1;
    let num_chunks = 10;
    let block_size = chunk_size * num_chunks * 32;
    let block: Vec<u8> = random_u8_slice(block_size);
    let committer = Committer::new(chunk_size);
    let source_node =
        rlnc_poc::node::Node::new_source(&committer, &block, num_chunks)
            .unwrap();
    c.bench_function("send small block", |b| {
        b.iter(|| {
            black_box(source_node.send().unwrap());
        })
    });

    let large_chunk_size = 380;
    let large_num_chunks = 10;
    let large_block_size = large_chunk_size * large_num_chunks * 32;
    let large_block: Vec<u8> = random_u8_slice(large_block_size);
    let committer = Committer::new(large_chunk_size);
    let source_node = rlnc_poc::node::Node::new_source(
        &committer,
        &large_block,
        large_num_chunks,
    )
    .unwrap();
    c.bench_function("send large block", |b| {
        b.iter(|| {
            black_box(source_node.send().unwrap());
        })
    });

    let source_node =
        rlnc_poc::node::Node::new_source(&committer, &block, num_chunks)
            .unwrap();
    let message = source_node.send().unwrap();
    let mut destination_node = Node::new(&committer, num_chunks);
    c.bench_function("receive small block", |b| {
        b.iter(|| {
            let cloned_message = message.clone();
            black_box(
                destination_node
                    .receive(cloned_message)
                    .or_else(|e| match e {
                        ReceiveError::LinearlyDependentChunk => Ok(()),
                        _ => Err(e),
                    })
                    .unwrap(),
            );
        })
    });

    let source_node = rlnc_poc::node::Node::new_source(
        &committer,
        &large_block,
        large_num_chunks,
    )
    .unwrap();
    let message = source_node.send().unwrap();
    let mut destination_node = Node::new(&committer, large_num_chunks);
    c.bench_function("receive large block", |b| {
        b.iter(|| {
            let cloned_message = message.clone();
            black_box(
                destination_node
                    .receive(cloned_message)
                    .or_else(|e| match e {
                        ReceiveError::LinearlyDependentChunk => Ok(()),
                        _ => Err(e),
                    })
                    .unwrap(),
            );
        })
    });
}

fn benchmark_decode(c: &mut Criterion) {
    let chunk_size = 1;
    let num_chunks = 10;
    let block_size = chunk_size * num_chunks * 32;
    let block: Vec<u8> = random_u8_slice(block_size);
    let committer = Committer::new(chunk_size);
    let source_node =
        rlnc_poc::node::Node::new_source(&committer, &block, num_chunks)
            .unwrap();
    let mut destination_node = Node::new(&committer, num_chunks);
    for _ in 0..num_chunks {
        destination_node
            .receive(source_node.send().unwrap())
            .or_else(|e| match e {
                ReceiveError::LinearlyDependentChunk => Ok(()),
                _ => Err(e),
            })
            .unwrap();
    }
    c.bench_function("decode small block", |b| {
        b.iter(|| {
            black_box(destination_node.decode().unwrap());
        })
    });

    let large_chunk_size = 380;
    let large_num_chunks = 10;
    let large_block_size = large_chunk_size * large_num_chunks * 32;
    let large_block: Vec<u8> = random_u8_slice(large_block_size);
    let committer = Committer::new(large_chunk_size);
    let source_node = rlnc_poc::node::Node::new_source(
        &committer,
        &large_block,
        large_num_chunks,
    )
    .unwrap();
    destination_node = Node::new(&committer, large_num_chunks);
    for _ in 0..large_num_chunks {
        destination_node
            .receive(source_node.send().unwrap())
            .or_else(|e| match e {
                ReceiveError::LinearlyDependentChunk => Ok(()),
                _ => Err(e),
            })
            .unwrap();
    }
    c.bench_function("decode large block", |b| {
        b.iter(|| {
            black_box(destination_node.decode().unwrap());
        })
    });
}

fn benchmark_receive_and_decode(c: &mut Criterion) {
    let chunk_size = 1;
    let num_chunks = 10;
    let block_size = chunk_size * num_chunks * 32;
    let block: Vec<u8> = random_u8_slice(block_size);
    let committer = Committer::new(chunk_size);
    let source_node =
        rlnc_poc::node::Node::new_source(&committer, &block, num_chunks)
            .unwrap();
    let mut messages: Vec<Message> = Vec::with_capacity(num_chunks);
    let mut destination_node = Node::new(&committer, num_chunks);
    for _ in 0..num_chunks {
        messages.push(source_node.send().unwrap());
    }
    c.bench_function("decode and receive small block", |b| {
        b.iter(|| {
            for i in &messages {
                destination_node
                    .receive(i.clone())
                    .or_else(|e| match e {
                        ReceiveError::LinearlyDependentChunk => Ok(()),
                        _ => Err(e),
                    })
                    .unwrap();
            }
            black_box(destination_node.decode().unwrap());
        })
    });

    let large_chunk_size = 380;
    let large_num_chunks = 10;
    let large_block_size = large_chunk_size * large_num_chunks * 32;
    let large_block: Vec<u8> = random_u8_slice(large_block_size);
    let committer = Committer::new(large_chunk_size);
    let source_node = rlnc_poc::node::Node::new_source(
        &committer,
        &large_block,
        large_num_chunks,
    )
    .unwrap();
    destination_node = Node::new(&committer, large_num_chunks);
    let mut messages: Vec<Message> = Vec::with_capacity(large_num_chunks);
    for _ in 0..large_num_chunks {
        messages.push(source_node.send().unwrap());
    }
    c.bench_function("decode and receive large block", |b| {
        b.iter(|| {
            for i in &messages {
                destination_node
                    .receive(i.clone())
                    .or_else(|e| match e {
                        ReceiveError::LinearlyDependentChunk => Ok(()),
                        _ => Err(e),
                    })
                    .unwrap();
            }
            black_box(destination_node.decode().unwrap());
        })
    });
}

criterion_group!(
    benches,
    benchmark_commit,
    benchmark_send_receive,
    benchmark_decode,
    benchmark_receive_and_decode,
);
criterion_main!(benches);
