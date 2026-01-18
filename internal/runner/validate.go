package runner

import (
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"os"
	"strings"

	"github.com/urfave/cli/v2"

	"github.com/Noswad123/ideomancer/internal/common"
	"github.com/Noswad123/ideomancer/internal/helper"
)

type ValidateResult struct {
	Valid  bool          `json:"valid"`
	Errors []ValidateErr `json:"errors"`
}

type ValidateErr struct {
	Path    string `json:"path,omitempty"`
	Message string `json:"message"`
	Code    int    `json:"code,omitempty"`
}

func ValidateManifest(c *cli.Context) error {
	schemaOnly := c.Bool("schema-only")

	data, err := io.ReadAll(os.Stdin)
	if err != nil {
		helper.FailIO(err)
		return nil
	}
	if len(strings.TrimSpace(string(data))) == 0 {
		helper.FailIO(errors.New("no input on stdin"))
		return nil
	}

	var m common.Manifest
	if err := helper.DecodeManifest(data, &m); err != nil {
		helper.FailIO(fmt.Errorf("parse error: %w", err))
		return nil
	}

	errs := validateManifest(&m, schemaOnly)

	enc := json.NewEncoder(os.Stdout)
	enc.SetEscapeHTML(false)
	enc.SetIndent("", "  ")
	if err := enc.Encode(ValidateResult{Valid: len(errs) == 0, Errors: errs}); err != nil {
		helper.FailIO(err)
		return nil
	}
	if len(errs) == 0 {
		os.Exit(0)
	} else {
		os.Exit(2)
	}
	return nil
}

func contains(ss []string, s string) bool {
	for _, x := range ss {
		if x == s {
			return true
		}
	}
	return false
}

func verr(path, msg string, code int) ValidateErr {
	return ValidateErr{Path: path, Message: msg, Code: code}
}

func requireNonEmpty(errs *[]ValidateErr, v, path string, code int) {
	if strings.TrimSpace(v) == "" {
		*errs = append(*errs, verr(path, "required", code))
	}
}

func validateManifest(m *common.Manifest, schemaOnly bool) []ValidateErr {
	var errs []ValidateErr

	// required header fields
	requireNonEmpty(&errs, m.SchemaVersion, "schemaVersion", 1001)
	requireNonEmpty(&errs, m.ID, "id", 1002)
	requireNonEmpty(&errs, m.Name, "name", 1003)
	requireNonEmpty(&errs, m.Version, "version", 1004)

	if len(m.Resources) == 0 {
		errs = append(errs, verr("resources", "must define at least one resource", 1100))
	}
	if len(m.Ops) == 0 {
		errs = append(errs, verr("ops", "must define at least one op", 1101))
	}
	if m.Interfaces.CLI == nil || len(m.Interfaces.CLI.Commands) == 0 {
		errs = append(errs, verr("interfaces.cli.commands", "must define at least one CLI command", 1102))
	}

	if schemaOnly {
		return errs
	}

	// build lookup tables
	resNames := map[string]common.Resource{}
	for i, r := range m.Resources {
		path := fmt.Sprintf("resources[%d]", i)
		if r.Name == "" {
			errs = append(errs, verr(path+".name", "resource name required", 1200))
		} else {
			if _, exists := resNames[r.Name]; exists {
				errs = append(errs, verr(path+".name", "duplicate resource name", 1201))
			}
			resNames[r.Name] = r
		}
		if r.Kind == "" {
			errs = append(errs, verr(path+".kind", "kind required (system|subsystem|consumable|process|relation)", 1202))
		}
		if r.DefaultMime != "" && !contains(r.MimeTypes, r.DefaultMime) {
			errs = append(errs, verr(path+".defaultMime", "defaultMime must be one of mimeTypes", 1203))
		}
	}

	opNames := map[string]bool{}
	for i, op := range m.Ops {
		opath := fmt.Sprintf("ops[%d]", i)
		if op.Name == "" {
			errs = append(errs, verr(opath+".name", "op name required (use resource.verb)", 1300))
		} else {
			if opNames[op.Name] {
				errs = append(errs, verr(opath+".name", "duplicate op name", 1301))
			}
			opNames[op.Name] = true
			if !strings.Contains(op.Name, ".") {
				errs = append(errs, verr(opath+".name", "op name should be resource.verb", 1302))
			}
		}
		for j, c := range op.Consumes {
			if c.Resource == "" {
				errs = append(errs, verr(fmt.Sprintf("%s.consumes[%d].resource", opath, j), "resource required", 1310))
			} else if _, ok := resNames[c.Resource]; !ok {
				errs = append(errs, verr(fmt.Sprintf("%s.consumes[%d].resource", opath, j), "unknown resource", 1311))
			}
		}
		for j, p := range op.Produces {
			if p.Resource == "" {
				errs = append(errs, verr(fmt.Sprintf("%s.produces[%d].resource", opath, j), "resource required", 1320))
			} else if _, ok := resNames[p.Resource]; !ok {
				errs = append(errs, verr(fmt.Sprintf("%s.produces[%d].resource", opath, j), "unknown resource", 1321))
			}
		}
	}

	// CLI command → op binding
	seenCmd := map[string]bool{}
	for i, cmd := range m.Interfaces.CLI.Commands {
		cpath := fmt.Sprintf("interfaces.cli.commands[%d]", i)
		if cmd.Name == "" {
			errs = append(errs, verr(cpath+".name", "command name required", 1400))
		} else if seenCmd[cmd.Name] {
			errs = append(errs, verr(cpath+".name", "duplicate command name", 1401))
		} else {
			seenCmd[cmd.Name] = true
		}
		if cmd.Op == "" {
			errs = append(errs, verr(cpath+".op", "must reference an op", 1402))
		} else if !opNames[cmd.Op] {
			errs = append(errs, verr(cpath+".op", "unknown op '"+cmd.Op+"'", 1403))
		}
	}

	return errs
}
