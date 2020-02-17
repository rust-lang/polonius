#!/bin/bash

RUSTC_RELEASE="stage1"
RUSTC_ARGS="-Znll-facts -Zborrowck=mir"
INPUT_FOLDERS=(issue-47680 smoke-test vec-push-ref)

for test_folder in "${INPUT_FOLDERS[@]}";
do
    pushd "$test_folder" || exit
    find . -name "*.facts" | xargs -- rm
    rustc +$RUSTC_RELEASE $RUSTC_ARGS -- *.rs
    popd || exit
done

