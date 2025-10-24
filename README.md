
# Ideomancer

![Ideomancer](img/ideomancer.png)

API Manifest (IETF draft / Microsoft)
# build
go mod tidy
go build -o ideomancer .

# 1) init as YAML with your naming convention
./ideomancer manifest:init --name "Ideomancer" --id ideomancer --out examples/ideomancer.idman.yaml
# -> writes file; refuses to overwrite if already exists

# 2) validate YAML directly (stdin)
cat examples/ideomancer.idman.yaml | ./ideomancer manifest:validate
# -> {"valid":true,"errors":[]}

# 3) init to JSON on stdout (no --out)
./ideomancer manifest:init --name "Test App" --id test-app > test.idman.json

# 4) validate JSON file
cat test.idman.json | ./ideomancer manifest:validate
