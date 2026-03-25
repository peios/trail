package cmd

import (
	"github.com/peios/trail/internal/server"
)

func runServe(f flags) error {
	return server.Serve(f.dir, f.output, f.port)
}
