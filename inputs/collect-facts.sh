#!/bin/bash

RUSTC_RELEASE="stage1"
RUSTC_ARGS="-Znll-facts -Zborrowck=mir"
INPUT_FOLDERS=(drop-liveness drop-may-dangle drop-no-may-dangle enum-drop-access
               issue-47680 issue-52059-report-when-borrow-and-drop-conflict
               maybe-initialized-drop maybe-initialized-drop-implicit-fragment-drop
               maybe-initialized-drop-uninitialized maybe-initialized-drop-with-fragment
               maybe-initialized-drop-with-uninitialized-fragments smoke-test vec-push-ref)

for test_folder in "${INPUT_FOLDERS[@]}";
do
    pushd "$test_folder" || exit
    find . -name "*.facts" | xargs -- rm
    rustc +$RUSTC_RELEASE $RUSTC_ARGS -- *.rs
    popd || exit
done

