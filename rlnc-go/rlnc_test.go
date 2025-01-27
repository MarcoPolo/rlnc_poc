package rlnc

import (
	"bytes"
	"crypto/rand"
	"testing"
)

func TestRoundTrip(t *testing.T) {
	rlnc, err := NewRLNC()
	if err != nil {
		t.Fatalf("Error creating RLNC: %v", err)
	}
	defer rlnc.Close()

	numChunks := 8
	chunkSize := 31 * 512

	committer, err := rlnc.GenCommitter(chunkSize / 31)
	if err != nil {
		t.Fatalf("Error creating committer: %v", err)
	}
	defer committer.Close()

	serialized, err := committer.Serialize()
	if err != nil {
		t.Fatalf("Error serializing committer: %v", err)
	}

	{
		var roundTripped Committer
		roundTripped.Deserialize(rlnc, serialized)
		roundTripped.Close()
	}

	data := make([]byte, chunkSize*numChunks)
	rand.Read(data)

	sourceNode, err := committer.NewSourceNode(data, numChunks)
	if err != nil {
		t.Fatalf("Error creating source node: %v", err)
	}
	defer sourceNode.Close()

	destinationNode := committer.NewNode(numChunks)
	defer destinationNode.Close()

	chunkToSend, err := sourceNode.ChunkToSend()
	if err != nil {
		t.Fatalf("Error getting chunk to send: %v", err)
	}
	commitmentsHash, err := rlnc.CommitmentsHash(chunkToSend)
	if err != nil {
		t.Fatalf("Error getting commitments hash: %v", err)
	}
	t.Logf("Commitments hash: %x", commitmentsHash)

	for i := 0; i < int(numChunks); i++ {
		chunkToSend, err := sourceNode.ChunkToSend()
		if err != nil {
			t.Fatalf("Error getting chunk to send: %v", err)
		}

		err = destinationNode.ReceiveChunk(chunkToSend)
		if err != nil {
			t.Fatalf("Error receiving chunk: %v", err)
		}
	}

	if !destinationNode.IsFull() {
		t.Fatalf("Destination node is not full")
	}

	destData, err := destinationNode.Data()
	if err != nil {
		t.Fatalf("Error getting data: %v", err)
	}
	if !bytes.Equal(data, destData) {
		t.Fatalf("Source and destination nodes do not have the same data")
	}
}
