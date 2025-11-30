package runner

import (
	"os"
	"strings"
	"errors"
	"fmt"
	"encoding/json"
	"io"


	"github.com/urfave/cli/v2"

	"github.com/Noswad123/ideomancer/internal/helper"
	"github.com/Noswad123/ideomancer/internal/common"
)

func RunGenerateMapCommand(c *cli.Context) error {
	format := c.String("format")

	data, err := io.ReadAll(os.Stdin)
	if err != nil { helper.FailIO(err) }
	if len(strings.TrimSpace(string(data))) == 0 { helper.FailIO(errors.New("no input")) }

	var m common.Manifest
	if err := helper.DecodeManifest(data, &m); err != nil { helper.FailIO(err) }

	// Build a simple graph
	g := map[string]any{
			"nodes": []map[string]any{{"id": m.ID, "kind": "system", "label": m.Name}},
			"edges": []map[string]any{},
	}

	switch format {
	case "json":
			enc := json.NewEncoder(os.Stdout)
			enc.SetIndent("", "  ")
			_ = enc.Encode(g)
	case "mermaid":
			fmt.Println("graph TD")
			fmt.Printf("  %s[%s]\n", m.ID, m.Name)
	default:
			helper.FailIO(fmt.Errorf("unsupported format: %s", format))
	}
	return nil
}
