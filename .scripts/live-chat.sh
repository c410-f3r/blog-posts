#!/usr/bin/env bash

set -euxo pipefail

trap "trap - SIGTERM && kill -- -$$" SIGINT SIGTERM EXIT

cargo build --manifest-path live-chat/backend/Cargo.toml
cargo run --manifest-path live-chat/backend/Cargo.toml &

pushd live-chat/frontend
    deno run dev -- --host
popd
