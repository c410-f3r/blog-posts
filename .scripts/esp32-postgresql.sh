#!/usr/bin/env bash

export ESP_LOG="INFO"
export URI="postgres://esp32:esp32@127.0.0.1:5432/esp32?channel_binding=disable"
export WIFI_PW=""
export WIFI_SSID=""

podman rm -f esp32
podman run \
    -d \
    --name esp32 \
    -e POSTGRES_DB=esp32 \
    -e POSTGRES_PASSWORD=esp32 \
    -p 5432:5432 \
    -v .scripts/postgres.sh:/docker-entrypoint-initdb.d/setup.sh \
    docker.io/library/postgres:17
# podman exec -e PGPASSWORD=esp32 -it esp32 psql -U esp32 -d esp32 -c "SELECT * FROM sensor;"

pushd esp32-postgresql
    cargo run -Z build-std-features="panic_immediate_abort" --release
popd
