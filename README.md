# Ideomancer

![Ideomancer](img/ideomancer.png)

## What is Ideomancer?
It is a program to help plan software projects.
### Ideal feature set:
    - generate boilerplat manifest
    - From the manifest, generate Mermaid and or UML diagrams

## Development
### build
go mod tidy
go build -o ideomancer .

### init as YAML with your naming convention
./ideomancer manifest:init --name "Ideomancer" --id ideomancer --out examples/ideomancer.idman.yaml
### writes file; refuses to overwrite if already exists

### validate YAML directly (stdin)
cat examples/ideomancer.idman.yaml | ./ideomancer manifest:validate
### {"valid":true,"errors":[]}

### init to JSON on stdout (no --out)
./ideomancer manifest:init --name "Test App" --id test-app > test.idman.json

### validate JSON file
cat test.idman.json | ./ideomancer manifest:validate
## Todo
- Finalize manifest structure
- Generate mermaid files from manifest
- generate UML from manifest
