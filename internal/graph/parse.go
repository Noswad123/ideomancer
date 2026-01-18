package graph

import (
	"bufio"
	"fmt"
	"io"
	"regexp"
	"strings"
)

var (
	// reNode matches a node declaration line of the form:
//
//   A["Service A"]
//
// Meaning:
// - A node ID starting with a letter, followed by letters, digits, or underscores
// - Optional whitespace
// - A label enclosed in square brackets and double quotes
// - Optional whitespace at the beginning and end of the line
//
// Capture groups:
//   1) The node ID (e.g. "A")
//   2) The node label text (e.g. "Service A")
	reNode = regexp.MustCompile("^\\s*([A-Za-z][A-Za-z0-9_]*)\\s*\\[\\s*\"(.*)\"\\s*\\]\\s*$")
	// Edge supports: A --> B  OR  A --> B : "label"
	reEdge = regexp.MustCompile(`^\s*([A-Za-z][A-Za-z0-9_]*)\s*-->\s*([A-Za-z][A-Za-z0-9_]*)\s*(?::\s*"(.*)")?\s*$`)
)

type Graph struct {
	Nodes map[string]Node
	Edges []Edge
	Order []string // stable order of first-seen nodes (useful for deterministic layout)
}

func Parse(r io.Reader) (*Graph, error) {
	g := &Graph{
		Nodes: map[string]Node{},
		Edges: []Edge{},
		Order: []string{},
	}

	sc := bufio.NewScanner(r)
	lineNo := 0

	seenHeader := false
	seenNode := func(id string) {
		if _, ok := g.Nodes[id]; ok {
			return
		}
		g.Nodes[id] = Node{ID: id, Label: id}
		g.Order = append(g.Order, id)
	}

	for sc.Scan() {
		lineNo++
		raw := sc.Text()
		line := strings.TrimSpace(raw)
		if line == "" || strings.HasPrefix(line, "#") {
			continue
		}

		if !seenHeader {
			if line != "graph" {
				return nil, fmt.Errorf("line %d: expected 'graph' header", lineNo)
			}
			seenHeader = true
			continue
		}

		// Node line: A["Label"]
		if m := reNode.FindStringSubmatch(line); m != nil {
			id := m[1]
			label := m[2]
			if _, exists := g.Nodes[id]; !exists {
				g.Order = append(g.Order, id)
			}
			g.Nodes[id] = Node{ID: id, Label: label}
			continue
		}

		// Edge line: A --> B : "label"
		if m := reEdge.FindStringSubmatch(line); m != nil {
			start := m[1]
			end := m[2]
			label := m[3]

			seenNode(start)
			seenNode(end)

			g.Edges = append(g.Edges, Edge{
				Start: start,
				End:   end,
				Label: label,
			})
			continue
		}

		return nil, fmt.Errorf("line %d: unrecognized syntax: %q", lineNo, raw)
	}

	if err := sc.Err(); err != nil {
		return nil, err
	}
	if !seenHeader {
		return nil, fmt.Errorf("missing 'graph' header")
	}

	return g, nil
}
