package main

import (
	"fmt"
	"os"

	"github.com/peios/trail/cmd"
)

func main() {
	if err := cmd.Run(os.Args[1:]); err != nil {
		fmt.Fprintf(os.Stderr, "trail: %v\n", err)
		os.Exit(1)
	}
}
