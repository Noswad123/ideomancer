package graph

import (
	"image/color"
	"math"

	"github.com/hajimehoshi/ebiten/v2"
	"github.com/hajimehoshi/ebiten/v2/ebitenutil"
	"github.com/hajimehoshi/ebiten/v2/vector"
)

// RenderConfig controls preview sizing and node visuals.
type RenderConfig struct {
	Width  int
	Height int

	NodeW float64
	NodeH float64

	Title string
}

func RenderPreview(spec *Graph, model GraphJSON, cfg RenderConfig) error {
	// bigger defaults
	if cfg.Width <= 0 {
		cfg.Width = 1200
	}
	if cfg.Height <= 0 {
		cfg.Height = 800
	}
	if cfg.NodeW <= 0 {
		cfg.NodeW = 160
	}
	if cfg.NodeH <= 0 {
		cfg.NodeH = 70
	}
	if cfg.Title == "" {
		cfg.Title = "Ideomancer Graph Preview"
	}

	g := newEbitenGame(spec, model, cfg)

	ebiten.SetWindowSize(cfg.Width, cfg.Height)
	ebiten.SetWindowTitle(cfg.Title)
	ebiten.SetTPS(60)

	return ebiten.RunGame(g)
}

type ebitenGame struct {
	spec  *Graph
	model GraphJSON
	cfg   RenderConfig

	// viewport transform
	panX, panY float64
	zoom       float64

	dragging   bool
	lastMouseX int
	lastMouseY int

	// bounds for initial centering
	minX, minY, maxX, maxY float64
}

func newEbitenGame(spec *Graph, model GraphJSON, cfg RenderConfig) *ebitenGame {
	g := &ebitenGame{
		spec:  spec,
		model: model,
		cfg:   cfg,
		zoom:  1.0,
	}

	g.computeBounds()

	// center bbox in window (world coords)
	worldCX := (g.minX + g.maxX) / 2
	worldCY := (g.minY + g.maxY) / 2

	// pan is in screen pixels; set so world center appears at screen center
	g.panX = float64(cfg.Width)/2 - worldCX*g.zoom
	g.panY = float64(cfg.Height)/2 - worldCY*g.zoom

	g.sanityClampTransform()
	return g
}

func (g *ebitenGame) computeBounds() {
	first := true
	for _, n := range g.model.Nodes {
		x1 := n.X - g.cfg.NodeW/2
		y1 := n.Y - g.cfg.NodeH/2
		x2 := n.X + g.cfg.NodeW/2
		y2 := n.Y + g.cfg.NodeH/2

		if first {
			g.minX, g.minY, g.maxX, g.maxY = x1, y1, x2, y2
			first = false
			continue
		}
		g.minX = math.Min(g.minX, x1)
		g.minY = math.Min(g.minY, y1)
		g.maxX = math.Max(g.maxX, x2)
		g.maxY = math.Max(g.maxY, y2)
	}
	if first {
		g.minX, g.minY, g.maxX, g.maxY = 0, 0, 0, 0
	}
}

func (g *ebitenGame) Update() error {
	// Zoom with mouse wheel
	_, wy := ebiten.Wheel()
	if wy != 0 {
		mx, my := ebiten.CursorPosition()

		// World point under cursor BEFORE zoom
		beforeWX, beforeWY := g.screenToWorld(float64(mx), float64(my))

		f := math.Pow(1.1, wy)
		g.zoom *= f
		g.zoom = clamp(g.zoom, 0.2, 5.0)

		// After zoom, adjust pan so the SAME world point stays under the cursor
		afterSX, afterSY := g.worldToScreen(beforeWX, beforeWY)
		g.panX += float64(mx) - afterSX
		g.panY += float64(my) - afterSY
	}

	// Pan with left drag
	if ebiten.IsMouseButtonPressed(ebiten.MouseButtonLeft) {
		mx, my := ebiten.CursorPosition()
		if !g.dragging {
			g.dragging = true
			g.lastMouseX, g.lastMouseY = mx, my
		} else {
			dx := mx - g.lastMouseX
			dy := my - g.lastMouseY
			g.panX += float64(dx)
			g.panY += float64(dy)
			g.lastMouseX, g.lastMouseY = mx, my
		}
	} else {
		g.dragging = false
	}

	g.sanityClampTransform()
	return nil
}

func (g *ebitenGame) Draw(screen *ebiten.Image) {
	// Background
	screen.Fill(color.RGBA{18, 18, 18, 255})

	// If transform is broken, bail early (prevents GPU backend from seeing NaNs/Infs)
	if !isFinite(g.panX) || !isFinite(g.panY) || !isFinite(g.zoom) || g.zoom <= 0 {
		return
	}

	nodeW := g.cfg.NodeW * g.zoom
	nodeH := g.cfg.NodeH * g.zoom
	halfW := nodeW / 2
	halfH := nodeH / 2

	// ---- Edges (behind nodes) with arrowheads ----
	for _, e := range g.model.Vectors {
		a, ok1 := g.model.Nodes[e.Start]
		b, ok2 := g.model.Nodes[e.End]
		if !ok1 || !ok2 {
			continue
		}

		// screen centers
		x1, y1 := g.worldToScreen(a.X, a.Y)
		x2, y2 := g.worldToScreen(b.X, b.Y)

		if !finitePoint(x1, y1) || !finitePoint(x2, y2) {
			continue
		}

		dx := x2 - x1
		dy := y2 - y1
		dist := math.Hypot(dx, dy)
		if dist < 1e-6 || !isFinite(dist) {
			continue
		}
		ux := dx / dist
		uy := dy / dist
		if !finitePoint(ux, uy) {
			continue
		}

		// node rects in screen space
		axMin, ayMin, axMax, ayMax := rectFromCenter(x1, y1, nodeW, nodeH)
		bxMin, byMin, bxMax, byMax := rectFromCenter(x2, y2, nodeW, nodeH)

		// intersection points: exiting A and entering B
		startX, startY, okS := rayRectIntersection(x1, y1, ux, uy, axMin, ayMin, axMax, ayMax)
		endX, endY, okE := rayRectIntersection(x2, y2, -ux, -uy, bxMin, byMin, bxMax, byMax)

		if !okS || !okE || !finitePoint(startX, startY) || !finitePoint(endX, endY) {
			// fallback to center-to-center, but still safe
			startX, startY = x1, y1
			endX, endY = x2, y2
		}

		// move start/end slightly OUTSIDE rectangles
		borderPad := math.Max(2.0, 4.0*g.zoom)
		startX += ux * borderPad
		startY += uy * borderPad
		endX -= ux * borderPad
		endY -= uy * borderPad

		if !finitePoint(startX, startY) || !finitePoint(endX, endY) {
			continue
		}
		if !reasonablePoint(startX, startY, g.cfg.Width, g.cfg.Height) ||
			!reasonablePoint(endX, endY, g.cfg.Width, g.cfg.Height) {
			// prevents absurd coordinates from getting drawn
			continue
		}

		drawLine(screen, startX, startY, endX, endY, color.White)

		// Arrowhead (visible at low zoom too)
		arrowLen := math.Max(12.0, 18.0*g.zoom)
		arrowHalfWidth := math.Max(6.0, 9.0*g.zoom)

		// tip is at endX,endY; base is behind it
		baseX := endX - ux*arrowLen
		baseY := endY - uy*arrowLen

		// perpendicular vector
		px := -uy
		py := ux

		leftX := baseX + px*arrowHalfWidth
		leftY := baseY + py*arrowHalfWidth
		rightX := baseX - px*arrowHalfWidth
		rightY := baseY - py*arrowHalfWidth

		if !finitePoint(leftX, leftY) || !finitePoint(rightX, rightY) {
			continue
		}

		// draw arrowhead outline
		drawLine(screen, endX, endY, leftX, leftY, color.White)
		drawLine(screen, endX, endY, rightX, rightY, color.White)
		drawLine(screen, leftX, leftY, rightX, rightY, color.White)

		// optional: edge label at midpoint
		if e.Label != "" {
			mx := (startX+endX)/2 - float64(len(e.Label))*3
			my := (startY+endY)/2 - 6
			if finitePoint(mx, my) && reasonablePoint(mx, my, g.cfg.Width, g.cfg.Height) {
				ebitenutil.DebugPrintAt(screen, e.Label, int(mx), int(my))
			}
		}
	}

	// ---- Nodes (on top) ----
	for id, pos := range g.model.Nodes {
		label := id
		if n, ok := g.spec.Nodes[id]; ok && n.Label != "" {
			label = n.Label
		}

		cx, cy := g.worldToScreen(pos.X, pos.Y)
		if !finitePoint(cx, cy) {
			continue
		}

		x := cx - halfW
		y := cy - halfH

		if !reasonablePoint(x, y, g.cfg.Width, g.cfg.Height) {
			continue
		}

		// Fill
		drawRect(screen, x, y, nodeW, nodeH, color.RGBA{30, 30, 30, 255})
		// Border
		drawLine(screen, x, y, x+nodeW, y, color.White)
		drawLine(screen, x+nodeW, y, x+nodeW, y+nodeH, color.White)
		drawLine(screen, x+nodeW, y+nodeH, x, y+nodeH, color.White)
		drawLine(screen, x, y+nodeH, x, y, color.White)

		// Text
		tx := int(x + nodeW/2 - float64(len(label))*3)
		ty := int(y + nodeH/2 - 6)
		ebitenutil.DebugPrintAt(screen, label, tx, ty)
	}
}

func (g *ebitenGame) Layout(outsideW, outsideH int) (int, int) {
	if outsideW > 0 {
		g.cfg.Width = outsideW
	}
	if outsideH > 0 {
		g.cfg.Height = outsideH
	}
	return outsideW, outsideH
}

func (g *ebitenGame) worldToScreen(wx, wy float64) (float64, float64) {
	return wx*g.zoom + g.panX, wy*g.zoom + g.panY
}

func (g *ebitenGame) screenToWorld(sx, sy float64) (float64, float64) {
	return (sx - g.panX) / g.zoom, (sy - g.panY) / g.zoom
}

func (g *ebitenGame) sanityClampTransform() {
	// Keep zoom sane
	if !isFinite(g.zoom) || g.zoom <= 0 {
		g.zoom = 1.0
	}
	g.zoom = clamp(g.zoom, 0.2, 5.0)

	// Keep pan from exploding into absurd coordinates (helps avoid GPU backend weirdness)
	limit := 1e6
	if !isFinite(g.panX) || math.Abs(g.panX) > float64(limit) {
		g.panX = 0
	}
	if !isFinite(g.panY) || math.Abs(g.panY) > float64(limit) {
		g.panY = 0
	}
}

func clamp(v, lo, hi float64) float64 {
	if v < lo {
		return lo
	}
	if v > hi {
		return hi
	}
	return v
}

func isFinite(v float64) bool {
	return !math.IsNaN(v) && !math.IsInf(v, 0)
}

func finitePoint(x, y float64) bool {
	return isFinite(x) && isFinite(y)
}

// reasonablePoint rejects extremely large values even if finite.
// This prevents accidental gigantic coordinates from hitting the renderer.
// The multiplier gives a generous area beyond the window.
func reasonablePoint(x, y float64, w, h int) bool {
	m := 50.0 // allow panning far, but not into crazy space
	maxX := float64(w) * m
	maxY := float64(h) * m
	return math.Abs(x) <= maxX && math.Abs(y) <= maxY
}

func rectFromCenter(cx, cy, w, h float64) (minX, minY, maxX, maxY float64) {
	halfW := w / 2
	halfH := h / 2
	return cx - halfW, cy - halfH, cx + halfW, cy + halfH
}

// rayRectIntersection returns the first intersection point of a ray starting at (sx,sy)
// going in direction (dx,dy) with the axis-aligned rectangle [minX,minY]..[maxX,maxY].
// Works best when (sx,sy) is inside the rectangle.
func rayRectIntersection(sx, sy, dx, dy, minX, minY, maxX, maxY float64) (ix, iy float64, ok bool) {
	const eps = 1e-9
	bestT := math.Inf(1)

	// vertical sides
	if math.Abs(dx) > eps {
		t := (minX - sx) / dx
		if t > 0 && isFinite(t) {
			y := sy + t*dy
			if isFinite(y) && y >= minY-eps && y <= maxY+eps && t < bestT {
				bestT = t
			}
		}
		t = (maxX - sx) / dx
		if t > 0 && isFinite(t) {
			y := sy + t*dy
			if isFinite(y) && y >= minY-eps && y <= maxY+eps && t < bestT {
				bestT = t
			}
		}
	}

	// horizontal sides
	if math.Abs(dy) > eps {
		t := (minY - sy) / dy
		if t > 0 && isFinite(t) {
			x := sx + t*dx
			if isFinite(x) && x >= minX-eps && x <= maxX+eps && t < bestT {
				bestT = t
			}
		}
		t = (maxY - sy) / dy
		if t > 0 && isFinite(t) {
			x := sx + t*dx
			if isFinite(x) && x >= minX-eps && x <= maxX+eps && t < bestT {
				bestT = t
			}
		}
	}

	if !math.IsInf(bestT, 1) && isFinite(bestT) {
		ix = sx + bestT*dx
		iy = sy + bestT*dy
		return ix, iy, finitePoint(ix, iy)
	}
	return 0, 0, false
}

func drawLine(dst *ebiten.Image, x1, y1, x2, y2 float64, clr color.Color) {
	vector.StrokeLine(
		dst,
		float32(x1), float32(y1),
		float32(x2), float32(y2),
		1.5, // stroke width
		clr,
		false,
	)
}

func drawRect(dst *ebiten.Image, x, y, w, h float64, clr color.Color) {
	vector.FillRect(
		dst,
		float32(x),
		float32(y),
		float32(w),
		float32(h),
		clr,
		false,
	)
}
