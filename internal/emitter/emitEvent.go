
package emitter
import (
	"time"
	"flag"
	"strings"
	"os"
	"fmt"
	"encoding/json"
  "crypto/rand"

	"github.com/Noswad123/ideomancer/internal/common"
	"github.com/Noswad123/ideomancer/internal/helper"
)
func cmdEventsEmit(args []string) {
    fs := flag.NewFlagSet("events:emit", flag.ExitOnError)
    typ := fs.String("type", "", "CloudEvent type (required)")
    source := fs.String("source", "ideomancer://events.emit", "CloudEvent source")
    dataStr := fs.String("data", "", "inline JSON data (mutually exclusive with --data-file)")
    dataFile := fs.String("data-file", "", "path to JSON file for data")
    contentType := fs.String("datacontenttype", "application/json", "data content type")
    _ = fs.Parse(args)

    if strings.TrimSpace(*typ) == "" {
        fmt.Fprintln(os.Stderr, "error: --type is required")
        os.Exit(2)
    }
    var data json.RawMessage
    if *dataStr != "" && *dataFile != "" {
        fmt.Fprintln(os.Stderr, "error: use either --data or --data-file, not both")
        os.Exit(2)
    }
    if *dataStr != "" {
        data = json.RawMessage(*dataStr)
    } else if *dataFile != "" {
        b, err := os.ReadFile(*dataFile)
        if err != nil { helper.FailIO(err) }
        data = json.RawMessage(b)
    } else {
        data = json.RawMessage(`{}`)
    }

    ce := common.CloudEvent{
        SpecVersion: "1.0",
        ID:          makeEventID(),
        Source:      *source,
        Type:        *typ,
        Time:        time.Now().UTC().Format(time.RFC3339Nano),
        DataContentType: *contentType,
        Data:        data,
    }
    enc := json.NewEncoder(os.Stdout)
    enc.SetEscapeHTML(false)
    if err := enc.Encode(ce); err != nil { helper.FailIO(err) }
}

func makeEventID() string {
    // simple unique-ish ID: timestamp + pid + rand
    b := make([]byte, 8)
    if _, err := rand.Read(b); err != nil {
        // fallback to time-based
        return fmt.Sprintf("evt-%d", time.Now().UnixNano())
    }
    return fmt.Sprintf("evt-%d-%x", time.Now().UnixNano(), b)
}
