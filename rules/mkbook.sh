#!/bin/bash

# `cd` into the directory where this script lives
cd "$(dirname "${BASH_SOURCE[0]}")"

for f in $(find . -name '*.dl'); do
    echo $f
    name=$(basename -- $f)
    ./literate.py "$f" > "../../book/src/rules/${name%.*}.md"
done
