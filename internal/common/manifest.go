package common

import (
	"encoding/json"
)

type Lore struct {
	Aliases     []string `json:"aliases,omitempty" yaml:"aliases,omitempty"`
	Epithet     string   `json:"epithet,omitempty" yaml:"epithet,omitempty"`
	Description string   `json:"description,omitempty" yaml:"description,omitempty"`
}

type Resource struct {
	Name        string   `json:"name" yaml:"name"`
	Kind        string   `json:"kind" yaml:"kind"`
	DefaultMime string   `json:"defaultMime,omitempty" yaml:"defaultMime,omitempty"`
	MimeTypes   []string `json:"mimeTypes,omitempty" yaml:"mimeTypes,omitempty"`
	SchemaRef   string   `json:"schemaRef,omitempty" yaml:"schemaRef,omitempty"`
	Description string   `json:"description,omitempty" yaml:"description,omitempty"`
}

type Op struct {
	Name     string  `json:"name" yaml:"name"` // convention: resource.verb
	Consumes []OpRef `json:"consumes,omitempty" yaml:"consumes,omitempty"`
	Produces []OpRef `json:"produces,omitempty" yaml:"produces,omitempty"`
	// errors/events omitted for brevity (add later)
}

type OpRef struct {
	Resource string `json:"resource,omitempty" yaml:"resource,omitempty"`
}

type Event struct {
	Name       string `json:"name" yaml:"name"`
	PayloadRef string `json:"payloadRef,omitempty" yaml:"payloadRef,omitempty"`
}

type Manifest struct {
	SchemaVersion string      `json:"schemaVersion" yaml:"schemaVersion"`
	ID            string      `json:"id" yaml:"id"`
	Name          string      `json:"name" yaml:"name"`
	Version       string      `json:"version" yaml:"version"`
	Summary       string      `json:"summary,omitempty" yaml:"summary,omitempty"`
	Resources     []Resource  `json:"resources" yaml:"resources"`
	Ops           []Op        `json:"ops" yaml:"ops"`
	Events        []Event     `json:"events,omitempty" yaml:"events,omitempty"`
	Interfaces    Interfaces  `json:"interfaces" yaml:"interfaces"`
	Lore          *Lore       `json:"lore,omitempty" yaml:"lore,omitempty"`
}
type Interfaces struct {
	CLI *CLI `json:"cli,omitempty" yaml:"cli,omitempty"`
}

type CLI struct {
	Commands []CLICommand `json:"commands" yaml:"commands"`
}

type CLICommand struct {
	Name       string        `json:"name" yaml:"name"`
	Op         string        `json:"op" yaml:"op"`
	Args       []CLIArg      `json:"args,omitempty" yaml:"args,omitempty"`
	Subscribes *Subscription `json:"subscribes,omitempty" yaml:"subscribes,omitempty"`
}

type CLIArg struct {
	Name     string `json:"name" yaml:"name"`
	Type     string `json:"type" yaml:"type"`
	Required bool   `json:"required" yaml:"required"`
}

type CloudEvent struct {
    SpecVersion     string          `json:"specversion"`
    ID              string          `json:"id"`
    Source          string          `json:"source"`
    Type            string          `json:"type"`
    Time            string          `json:"time"`
    DataContentType string          `json:"datacontenttype,omitempty"`
    Data            json.RawMessage `json:"data,omitempty"`
}

type Subscription struct {
	Bus         string `json:"bus" yaml:"bus"`
	Topic       string `json:"topic" yaml:"topic"`
	ContentType string `json:"contentType,omitempty" yaml:"contentType,omitempty"`
}
