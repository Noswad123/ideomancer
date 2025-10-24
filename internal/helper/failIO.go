package helper

import (
	"os"
	"fmt"
)


func FailIO(err error) {
	fmt.Fprintf(os.Stderr, "error: %v\n", err)
	os.Exit(3)
}
