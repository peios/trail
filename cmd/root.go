package cmd

import (
	"fmt"
	"strings"
)

const usage = `trail — static site generator for documentation with pathway support

Usage:
  trail build [--dir <path>] [--output <path>]
  trail serve [--dir <path>] [--port <n>]

Commands:
  build    Build the static site
  serve    Start a local development server

Flags:
  --dir      Site root directory (default: current directory)
  --output   Output directory (default: _site)
  --port     Dev server port (default: 3000)
`

func Run(args []string) error {
	if len(args) == 0 {
		fmt.Print(usage)
		return nil
	}

	cmd := args[0]
	flags := parseFlags(args[1:])

	switch cmd {
	case "build":
		return runBuild(flags)
	case "serve":
		return runServe(flags)
	case "help", "--help", "-h":
		fmt.Print(usage)
		return nil
	default:
		return fmt.Errorf("unknown command: %s", cmd)
	}
}

type flags struct {
	dir    string
	output string
	port   string
}

func parseFlags(args []string) flags {
	f := flags{
		dir:    ".",
		output: "_site",
		port:   "3000",
	}
	for i := 0; i < len(args); i++ {
		switch args[i] {
		case "--dir":
			if i+1 < len(args) {
				f.dir = args[i+1]
				i++
			}
		case "--output":
			if i+1 < len(args) {
				f.output = args[i+1]
				i++
			}
		case "--port":
			if i+1 < len(args) {
				f.port = args[i+1]
				i++
			}
		}
	}
	// Trim trailing slash from dir
	f.dir = strings.TrimRight(f.dir, "/")
	return f
}
