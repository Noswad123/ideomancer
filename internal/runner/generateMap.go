package runner

import (
	"encoding/json"
	"fmt"
	"os"

	"github.com/Noswad123/ideomancer/internal/graph"
	"github.com/urfave/cli/v2"
)

func GenerateGraph(c *cli.Context) error {
	inPath := c.String("in")
	outPath := c.String("out")
	noRender := c.Bool("no-render")

	width := c.Int("width")
	height := c.Int("height")

	if inPath == "" {
		return fmt.Errorf("--in is required")
	}
	if width <= 0 {
		width = 900
	}
	if height <= 0 {
		height = 600
	}

	f, err := os.Open(inPath)
	if err != nil {
		return err
	}
	defer f.Close()

	spec, err := graph.Parse(f)
	if err != nil {
		return err
	}

	model := spec.ToJSON(graph.LayoutOptions{
		Width:  float64(width),
		Height: float64(height),
	})

	if outPath != "" {
		if err := writeJSON(outPath, model); err != nil {
			return err
		}
	}

	if noRender {
		return nil
	}

	return graph.RenderPreview(spec, model, graph.RenderConfig{
		Width:  width,
		Height: height,
		Title:  "Ideomancer Graph Preview",
	})
}

func writeJSON(path string, v any) error {
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()

	enc := json.NewEncoder(f)
	enc.SetIndent("", "  ")
	return enc.Encode(v)
}
