#!/bin/bash

# `cd` into the directory where this script lives
cd "$(dirname "${BASH_SOURCE[0]}")"
# `cd` into the project's root dir
cd ..

for f in $(find rules -name '*.dl'); do
    echo $f
    name=$(basename -- $f)
    cargo run -q --package polonius-docgen -- "$f" > "book/src/rules/${name%.*}.md"
done
