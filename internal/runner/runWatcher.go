package runner

import (
	"io"
	"encoding/json"
	"fmt"
	"bufio"
  "bytes"
	"strings"
	"os"
	"flag"

	"github.com/urfave/cli/v2"
	"github.com/Noswad123/ideomancer/internal/common"
	"github.com/Noswad123/ideomancer/internal/helper"
)

func RunWatcherCommand(c *cli.Context) {
    fs := flag.NewFlagSet("watcher:run", flag.ExitOnError)
    typesCSV := fs.String("types", "", "comma-separated list of event types to match (optional)")
    fromFile := fs.String("from-file", "", "read NDJSON CloudEvents from file instead of stdin")
    printMode := fs.String("print", "summary", "summary|full|data (stderr output for humans)")
    _ = fs.Parse(c.args)

    var r io.Reader = os.Stdin
    if strings.TrimSpace(*fromFile) != "" {
        f, err := os.Open(*fromFile)
        if err != nil { helper.FailIO(err) }
        defer f.Close()
        r = f
    }

    typeSet := map[string]bool{}
    if t := strings.TrimSpace(*typesCSV); t != "" {
        for _, s := range strings.Split(t, ",") {
            s = strings.TrimSpace(s)
            if s != "" { typeSet[s] = true }
        }
    }

    sc := bufio.NewScanner(r)
    // allow long lines
    buf := make([]byte, 0, 1024*1024)
    sc.Buffer(buf, 10*1024*1024)

    for sc.Scan() {
        line := sc.Bytes()
        if len(bytes.TrimSpace(line)) == 0 { continue }

        var ce common.CloudEvent
        if err := json.Unmarshal(line, &ce); err != nil {
            fmt.Fprintf(os.Stderr, "watcher: skip invalid JSON: %v\n", err)
            continue
        }
        if ce.SpecVersion == "" || ce.Type == "" || ce.ID == "" {
            fmt.Fprintf(os.Stderr, "watcher: skip non-CloudEvent line\n")
            continue
        }
        if len(typeSet) > 0 && !typeSet[ce.Type] {
            continue // filtered out
        }

        // Human-facing stderr
        switch *printMode {
        case "summary":
            fmt.Fprintf(os.Stderr, "event %s id=%s source=%s\n", ce.Type, ce.ID, ce.Source)
        case "data":
            fmt.Fprintf(os.Stderr, "%s\n", string(ce.Data))
        case "full":
            b, _ := json.MarshalIndent(ce, "", "  ")
            fmt.Fprintf(os.Stderr, "%s\n", string(b))
        default:
            fmt.Fprintf(os.Stderr, "watcher: unknown --print mode %q (use summary|full|data)\n", *printMode)
        }

        // Machine-facing stdout: re-emit matched events (so you can pipe)
        os.Stdout.Write(line)
        os.Stdout.Write([]byte("\n"))
    }
    if err := sc.Err(); err != nil {
        helper.FailIO(err)
    }
}
