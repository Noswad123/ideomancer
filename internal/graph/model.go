package graph

type Node struct {
	ID    string
	Label string
}

type Edge struct {
	Start string
	End   string
	Label string
}

type PositionedNode struct {
	X float64 `json:"x"`
	Y float64 `json:"y"`
}

type GraphJSON struct {
	Nodes   map[string]PositionedNode `json:"nodes"`
	Vectors []EdgeJSON               `json:"vectors"`
}

type EdgeJSON struct {
	Start string `json:"start"`
	End   string `json:"end"`
	Label string `json:"label,omitempty"`
}
