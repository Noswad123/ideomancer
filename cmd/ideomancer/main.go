package main

import (
    "os"
		"log"

		"github.com/Noswad123/ideomancer/internal/runner"
		"github.com/urfave/cli/v2"
)


func main() {

	app := &cli.App{
		Name:  "ideomancer",
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
				Action: func(c *cli.Context) error {
					return runner.RunCreateManifestCommand(c)
				},
			},
			{
				Name:  "validate",
				Usage: "Validate manifest",
				Flags: []cli.Flag{},
				Action: func(c *cli.Context) error {
					return runner.RunValidateManifestCommand(c)
				},
			},
			{
				Name:  "generate",
				Usage: "Generate a system map file based off manifest",
				Flags: []cli.Flag{},
				Action: func(c *cli.Context) error {
					return runner.RunGenerateMap(c)
				},
				// flag.NewFlagSet("generate:map", flag.ExitOnError)
			},
			{
				Name:  "watcher",
				Usage: "Watch manifest file for changes",
				Action: func(c *cli.Context) error {
					return runner.RunWatchManifest(c)
				},
			},
		},
	}

	if err := app.Run(os.Args); err != nil {
		log.Fatal(err)
	}
}


