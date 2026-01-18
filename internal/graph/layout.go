package graph

import "math"

type LayoutOptions struct {
	Width  float64
	Height float64
}

func (g *Graph) ToJSON(opts LayoutOptions) GraphJSON {
	if opts.Width <= 0 {
		opts.Width = 700
	}
	if opts.Height <= 0 {
		opts.Height = 400
	}

	n := len(g.Order)
	nodes := make(map[string]PositionedNode, n)

	usableW := math.Max(0, opts.Width)
	y := opts.Height / 2

	// If 1 node, place it center-ish.
	if n == 1 {
		id := g.Order[0]
		nodes[id] = PositionedNode{X: opts.Width / 2, Y: y}
	} else if n > 1 {
		step := usableW / float64(n-1)
		for i, id := range g.Order {
			x := float64(i)*step
			nodes[id] = PositionedNode{X: x, Y: y}
		}
	}

	vectors := make([]EdgeJSON, 0, len(g.Edges))
	for _, e := range g.Edges {
		vectors = append(vectors, EdgeJSON{
			Start: e.Start,
			End:   e.End,
			Label: e.Label,
		})
	}

	return GraphJSON{Nodes: nodes, Vectors: vectors}
}
