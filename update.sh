#!/bin/bash

VERSION="11.2.0"

pushd harfbuzz
git fetch
git checkout $VERSION
popd

bindgen --no-prepend-enum-name --allowlist-function hb_.\* --allowlist-type hb_.\* wrapper.h > src/bindings.rs
