//go:build darwin

package rlnc

import (
	_ "embed"
	"os"
	"runtime"
)

//go:generate sh -c "cargo build --release && cargo build && cp ../target/release/librlnc_poc.dylib rust-lib/release/librlnc_poc.dylib && cp ../target/debug/librlnc_poc.dylib rust-lib/debug/librlnc_poc.dylib"

//go:embed rust-lib/release/librlnc_poc.dylib
var releaseLib []byte

//go:embed rust-lib/debug/librlnc_poc.dylib
var debugLib []byte

var tempLibPath string

func getLibPath() string {
	if tempLibPath != "" {
		return tempLibPath
	}

	// Create a temporary directory to extract the library
	tempDir := os.TempDir()

	DEBUG := os.Getenv("DEBUG") != ""
	libName := "librlnc_poc.dylib"
	tempPath := tempDir + "/" + libName

	// Choose which library to write based on DEBUG flag
	libData := releaseLib
	if DEBUG {
		libData = debugLib
	}

	// Write the library to a temporary file
	err := os.WriteFile(tempPath, libData, 0755)
	if err != nil {
		panic(err)
	}

	// Attempt to clean up the temporary file on exit
	runtime.SetFinalizer(new(struct{}), func(_ interface{}) {
		os.Remove(tempPath)
	})

	tempLibPath = tempPath
	return tempPath
}
