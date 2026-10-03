#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
cd -- "$repo_dir/firmware"

for tool in cargo rustup ldproxy; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        printf 'Missing required tool: %s. See README.md for setup instructions.\n' "$tool" >&2
        exit 1
    fi
done

if ! rustup run esp rustc --version >/dev/null 2>&1; then
    printf 'Missing or unusable esp Rust toolchain. Run espup install and source its export script; see README.md.\n' >&2
    exit 1
fi

# ESP-IDF builds its CMake project in Cargo's output directory. Resolve the
# tracked partition CSV here so the build also works outside the repository.
mkdir -p target
partition_sdkconfig="$repo_dir/firmware/target/esp32-partitions.sdkconfig"
if ! printf 'CONFIG_PARTITION_TABLE_CUSTOM_FILENAME="%s"\n' \
    "$repo_dir/firmware/partitions.csv" | cmp -s - "$partition_sdkconfig"; then
    printf 'CONFIG_PARTITION_TABLE_CUSTOM_FILENAME="%s"\n' \
        "$repo_dir/firmware/partitions.csv" > "$partition_sdkconfig"
fi
export ESP_IDF_SDKCONFIG="$partition_sdkconfig"
exec cargo build --release "$@"
