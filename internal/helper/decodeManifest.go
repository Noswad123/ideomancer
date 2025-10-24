package helper

import (
	"encoding/json"
	"fmt"

  "gopkg.in/yaml.v3"
	"github.com/Noswad123/ideomancer/internal/common"
)


func DecodeManifest(data []byte, out *common.Manifest) error {
	// Try JSON first
	if err := json.Unmarshal(data, out); err == nil && out.SchemaVersion != "" {
		return nil
	}
	// Try YAML → then re-marshal to JSON to leverage same struct tags
	var y any
	if err := yaml.Unmarshal(data, &y); err != nil {
		return fmt.Errorf("neither valid JSON nor YAML: %w", err)
	}
	// Normalize YAML numbers/keys by round-tripping through JSON encoder
	j, err := json.Marshal(y)
	if err != nil {
		return fmt.Errorf("yaml→json marshal: %w", err)
	}
	if err := json.Unmarshal(j, out); err != nil {
		return fmt.Errorf("yaml→struct unmarshal: %w", err)
	}
	return nil
}
