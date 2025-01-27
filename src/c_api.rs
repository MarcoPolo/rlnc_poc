use std::ptr;

use crate::blocks::Committer;
use crate::node::{Message, Node, ReceiveError};

#[no_mangle]
pub extern "C" fn gen_committer(
    chunk_size_in_scalars: u32,
) -> *const std::ffi::c_void {
    let committer = Committer::new(chunk_size_in_scalars as usize);
    return Box::into_raw(Box::new(committer)) as *const std::ffi::c_void;
}

#[no_mangle]
pub extern "C" fn serialize_committer(
    committer_ptr: *const std::ffi::c_void,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) {
    let committer = unsafe { &*(committer_ptr as *const Committer) };
    let serialized = bincode::serialize(&committer).unwrap();
    unsafe {
        *out_len = serialized.len();
        *out_ptr = Box::into_raw(serialized.into_boxed_slice()) as *mut u8;
    }
}

#[no_mangle]
pub extern "C" fn deserialize_committer(
    serialized_ptr: *const u8,
    serialized_len: usize,
) -> *const std::ffi::c_void {
    let serialized =
        unsafe { std::slice::from_raw_parts(serialized_ptr, serialized_len) };

    bincode::deserialize::<Committer>(&serialized)
        .and_then(|c| Ok(Box::into_raw(Box::new(c)) as *const std::ffi::c_void))
        .unwrap_or(ptr::null())
}

#[no_mangle]
pub extern "C" fn free_committer(committer_ptr: *const std::ffi::c_void) {
    unsafe { drop(Box::from_raw(committer_ptr as *mut Committer)) }
}

#[no_mangle]
pub extern "C" fn new_node(
    commiter: *const std::ffi::c_void,
    num_chunks: u32,
) -> *const std::ffi::c_void {
    let commiter = unsafe { &*(commiter as *const Committer) };
    let node = Node::new(commiter, num_chunks as usize);
    return Box::into_raw(Box::new(node)) as *const std::ffi::c_void;
}

#[no_mangle]
pub extern "C" fn new_source_node(
    commiter: *const std::ffi::c_void,
    block: *const u8,
    block_len: usize,
    num_chunks: u32,
) -> *const std::ffi::c_void {
    let commiter = unsafe { &*(commiter as *const Committer) };
    let block = unsafe { std::slice::from_raw_parts(block, block_len) };
    if let Ok(node) = Node::new_source(commiter, block, num_chunks as usize) {
        return Box::into_raw(Box::new(node)) as *const std::ffi::c_void;
    }
    ptr::null()
}

#[no_mangle]
pub extern "C" fn free_node(node_ptr: *const std::ffi::c_void) {
    unsafe { drop(Box::from_raw(node_ptr as *mut Node)) }
}

#[no_mangle]
pub extern "C" fn send_chunk(
    node_ptr: *const std::ffi::c_void,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    let node = unsafe { &*(node_ptr as *const Node) };
    if let Ok(serialized) = node.send().and_then(|message| {
        bincode::serialize(&message).map_err(|e| e.to_string())
    }) {
        unsafe {
            *out_len = serialized.len();
            let boxed = serialized.into_boxed_slice();
            *out_data = Box::into_raw(boxed) as *mut u8;
        }
        return 0;
    }
    -1
}

#[no_mangle]
pub extern "C" fn receive_chunk(
    node_ptr: *const std::ffi::c_void,
    chunk_start: *const u8,
    chunk_len: usize,
) -> i32 {
    let node = unsafe { &mut *(node_ptr as *mut Node) };
    let chunk = unsafe { std::slice::from_raw_parts(chunk_start, chunk_len) };

    match bincode::deserialize(chunk).or(Err(-1)).and_then(|message| {
        node.receive(message).map_err(|e| match e {
            ReceiveError::ExistingCommitmentsMismatch(_e) => -2,
            ReceiveError::ExistingChunksMismatch(_e) => -3,
            ReceiveError::InvalidMessage(_e) => -4,
            ReceiveError::LinearlyDependentChunk => -5,
        })
    }) {
        Ok(_) => 0,
        Err(e) => e,
    }
}

#[no_mangle]
pub extern "C" fn is_full(node_ptr: *const std::ffi::c_void) -> i32 {
    let node = unsafe { &*(node_ptr as *const Node) };
    if node.is_full() {
        return 1;
    }
    return 0;
}

#[no_mangle]
pub extern "C" fn decode(
    node_ptr: *const std::ffi::c_void,
    out_data: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    let node = unsafe { &*(node_ptr as *const Node) };
    if !node.is_full() {
        return -1;
    }

    if let Ok(data) = node.decode() {
        unsafe {
            *out_len = data.len();
            *out_data = Box::into_raw(data.into_boxed_slice()) as *mut u8;
        }
        return 0;
    }
    return 0;
}

#[no_mangle]
pub extern "C" fn free_buffer(ptr: *mut u8, len: usize) {
    unsafe {
        let slice = std::slice::from_raw_parts_mut(ptr, len);
        drop(Box::from_raw(slice));
    }
}

#[no_mangle]
pub extern "C" fn commitments_hash(
    message_data: *const u8,
    message_len: usize,
    out_ptr: *mut *mut u8,
    out_len: *mut usize,
) -> i32 {
    let message_bytes =
        unsafe { std::slice::from_raw_parts(message_data, message_len) };
    match bincode::deserialize::<Message>(&message_bytes) {
        Ok(message) => {
            let hash = message.commitments_hash();
            unsafe {
                *out_len = hash.len();
                *out_ptr =
                    Box::into_raw(hash.to_vec().into_boxed_slice()) as *mut u8;
            }
            return 0;
        }
        Err(_) => return -1,
    }
}
