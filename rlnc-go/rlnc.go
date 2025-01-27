package rlnc

import (
	"fmt"
	"slices"
	"unsafe"

	"github.com/ebitengine/purego"
)

type RLNC struct {
	lib uintptr

	genCommitter         func(chunkSizeInScalars uint32) unsafe.Pointer
	serializeCommitter   func(commiter unsafe.Pointer, outPtr *unsafe.Pointer, outLen *uint64)
	deserializeCommitter func(serializedPtr unsafe.Pointer, serializedLen uint64) unsafe.Pointer
	freeCommitter        func(commiter unsafe.Pointer)
	newNode              func(commiter unsafe.Pointer, numChunks uint32) unsafe.Pointer
	newSourceNode        func(commiter unsafe.Pointer, block []byte, blockLen uint64, numChunks uint32) unsafe.Pointer
	freeNode             func(node unsafe.Pointer)
	sendChunk            func(node unsafe.Pointer, outData *unsafe.Pointer, outDataLen *uint64) int32
	receiveChunk         func(node unsafe.Pointer, chunk []byte, chunkLen uint64) int32
	decode               func(node unsafe.Pointer, outData *unsafe.Pointer, outDataLen *uint64) int32
	freeBuffer           func(buffer unsafe.Pointer, len uint64)
	isFull               func(node unsafe.Pointer) bool

	commitmentsHash func(messageData unsafe.Pointer, messageLen uint64, outPtr *unsafe.Pointer, outLen *uint64) int32
}

func NewRLNC() (*RLNC, error) {
	libPath := getLibPath()
	lib, err := purego.Dlopen(libPath, purego.RTLD_NOW|purego.RTLD_GLOBAL)
	if err != nil {
		return nil, err
	}

	r := &RLNC{lib: lib}

	purego.RegisterLibFunc(&r.genCommitter, lib, "gen_committer")
	purego.RegisterLibFunc(&r.serializeCommitter, lib, "serialize_committer")
	purego.RegisterLibFunc(&r.deserializeCommitter, lib, "deserialize_committer")
	purego.RegisterLibFunc(&r.freeCommitter, lib, "free_committer")
	purego.RegisterLibFunc(&r.newNode, lib, "new_node")
	purego.RegisterLibFunc(&r.newSourceNode, lib, "new_source_node")
	purego.RegisterLibFunc(&r.freeNode, lib, "free_node")
	purego.RegisterLibFunc(&r.sendChunk, lib, "send_chunk")
	purego.RegisterLibFunc(&r.receiveChunk, lib, "receive_chunk")
	purego.RegisterLibFunc(&r.decode, lib, "decode")
	purego.RegisterLibFunc(&r.freeBuffer, lib, "free_buffer")
	purego.RegisterLibFunc(&r.isFull, lib, "is_full")
	purego.RegisterLibFunc(&r.commitmentsHash, lib, "commitments_hash")
	return r, nil
}

func (r *RLNC) Close() {
	purego.Dlclose(r.lib)
}

func (r *RLNC) GenCommitter(messageSize int, numChunks int) (*Committer, error) {
	if messageSize%numChunks != 0 {
		return nil, fmt.Errorf("message size must be a multiple of num chunks")
	}
	chunkSize := messageSize / numChunks
	chunkSizeInScalars := (chunkSize*8 + 251) / 252
	commiter := r.genCommitter(uint32(chunkSizeInScalars))
	return &Committer{r: r, p: commiter}, nil
}

func (r *RLNC) CommitmentsHash(message []byte) ([]byte, error) {
	var outPtr unsafe.Pointer
	var outLen uint64
	res := r.commitmentsHash(unsafe.Pointer(&message[0]), uint64(len(message)), &outPtr, &outLen)
	if res != 0 {
		return nil, fmt.Errorf("failed to get commitments hash")
	}
	defer r.freeBuffer(outPtr, outLen)
	s := unsafe.Slice((*byte)(outPtr), int(outLen))
	copied := slices.Clone(s)
	return copied, nil
}

type Committer struct {
	r *RLNC
	p unsafe.Pointer
}

func (c *Committer) Serialize() ([]byte, error) {
	var outPtr unsafe.Pointer
	var outLen uint64
	c.r.serializeCommitter(c.p, &outPtr, &outLen)
	copied := slices.Clone(unsafe.Slice((*byte)(outPtr), int(outLen)))
	c.r.freeBuffer(outPtr, outLen)
	return copied, nil
}

func (c *Committer) Deserialize(r *RLNC, serialized []byte) error {
	c.r = r
	c.p = c.r.deserializeCommitter(unsafe.Pointer(&serialized[0]), uint64(len(serialized)))
	return nil
}

func (c *Committer) Close() {
	c.r.freeCommitter(c.p)
}

type Node struct {
	r *RLNC
	p unsafe.Pointer
}

func (c *Committer) NewNode(numChunks int) *Node {
	return &Node{r: c.r, p: c.r.newNode(c.p, uint32(numChunks))}
}

func (c *Committer) NewSourceNode(block []byte, numChunks int) (*Node, error) {
	if len(block)%numChunks != 0 {
		return nil, fmt.Errorf("block size must be a multiple of chunk size")
	}

	return &Node{r: c.r, p: c.r.newSourceNode(c.p, block, uint64(len(block)), uint32(numChunks))}, nil
}

func (n *Node) Close() {
	n.r.freeNode(n.p)
}

func (n *Node) ChunkToSend() ([]byte, error) {
	var outData unsafe.Pointer
	var outDataLen uint64
	res := n.r.sendChunk(n.p, &outData, &outDataLen)
	if res != 0 {
		return nil, fmt.Errorf("failed to get chunk")
	}
	defer n.r.freeBuffer(outData, outDataLen)
	s := unsafe.Slice((*byte)(outData), int(outDataLen))
	copied := slices.Clone(s)
	return copied, nil
}

func (n *Node) ReceiveChunk(chunk []byte) error {
	res := n.r.receiveChunk(n.p, chunk, uint64(len(chunk)))
	switch res {
	case 0:
		return nil
	case -1:
		return fmt.Errorf("failed to receive chunk")
	case -2:
		return fmt.Errorf("existing commitments mismatch")
	case -3:
		return fmt.Errorf("existing chunks mismatch")
	case -4:
		return fmt.Errorf("invalid message")
	case -5:
		return fmt.Errorf("linearly dependent chunk")
	default:
		return fmt.Errorf("unknown error")
	}
}

func (n *Node) Data() ([]byte, error) {
	var outData unsafe.Pointer
	var outDataLen uint64
	res := n.r.decode(n.p, &outData, &outDataLen)
	if res != 0 {
		return nil, fmt.Errorf("failed to get data")
	}
	defer n.r.freeBuffer(outData, outDataLen)
	s := unsafe.Slice((*byte)(outData), int(outDataLen))
	copied := slices.Clone(s)
	return copied, nil
}

func (n *Node) IsFull() bool {
	return n.r.isFull(n.p)
}
