package main

import (
	"log"
	"os"

	"github.com/Noswad123/ideomancer/internal/runner"
	"github.com/urfave/cli/v2"
)

func main() {

	app := &cli.App{
		Name: "ideomancer",
		Usage: `
		A system planning tool
		Usage:
			ideomancer manifest:validate [--schema-only]
  		ideomancer manifest:init --name NAME [--id ID] [--out PATH]

		Description:
			manifest:validate
				Reads a manifest (JSON or YAML) from stdin and validates structure + basic semantics.
				Prints {"valid":true|false,"errors":[...]} to stdout.
				Exit codes: 0 ok, 2 validation error, 3 I/O/parse error.

			manifest:init
				Emits a minimal, valid Ideomancer manifest. If --out ends with .idman.json or .idman.yaml,
				writes that file; otherwise prints JSON to stdout. Won't overwrite existing files.

		Examples:
			ideomancer manifest:init --name "Ideomancer" --id ideomancer --out ideomancer.idman.yaml
			cat ideomancer.idman.yaml | ideomancer manifest:validate
		`,

		Commands: []*cli.Command{
			{
				Name:  "init",
				Usage: "create a manifest file",
				Flags: []cli.Flag{
					&cli.StringFlag{Name: "name", Required: true},
					&cli.StringFlag{Name: "id"},
					&cli.StringFlag{Name: "out"},
				},
				Action: runner.CreateManifest,
			},
			{
				Name:  "validate",
				Usage: "Validate manifest",
				Flags: []cli.Flag{
					&cli.BoolFlag{Name: "schema-only"},
				},
				Action: runner.ValidateManifest,
			},
			{
				Name:  "graph",
				Usage: "Generate a graph JSON file based off a graph spec",
				Flags: []cli.Flag{
					&cli.StringFlag{Name: "in", Usage: "path to graph spec file", Required: true},
					&cli.StringFlag{Name: "out", Usage: "path to output JSON (default: stdout)"},
					&cli.Float64Flag{Name: "width", Value: 700, Usage: "layout width in pixels"},
					&cli.Float64Flag{Name: "height", Value: 400, Usage: "layout height in pixels"},
					&cli.BoolFlag{Name: "no-render", Usage: "do not open the GUI preview window"},
				},
				Action: runner.GenerateGraph,
			},
		},
	}

	if err := app.Run(os.Args); err != nil {
		log.Fatal(err)
	}
}
