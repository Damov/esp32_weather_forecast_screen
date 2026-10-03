#!/usr/bin/env bash
set -euo pipefail

repo_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
port="${1:-/dev/ttyUSB0}"
image_dir="$repo_dir/firmware/target/xtensa-esp32-espidf/release"
python_bin="$repo_dir/firmware/.embuild/espressif/python_env/idf5.5_py3.10_env/bin/python"

if [[ ! -c "$port" ]]; then
    printf 'Serial device is unavailable: %s\nRun this script in the host terminal with the board connected.\n' "$port" >&2
    exit 1
fi
if [[ ! -r "$port" || ! -w "$port" ]]; then
    printf 'No read/write access to %s. Grant your terminal user serial access first.\n' "$port" >&2
    exit 1
fi
if [[ ! -x "$python_bin" ]]; then
    python_bin=python3
fi
for file in bootloader.bin partition-table.bin weather-forecast-firmware.bin; do
    if [[ ! -f "$image_dir/$file" ]]; then
        printf 'Missing ESP32 image: %s/%s. Build and generate it first.\n' "$image_dir" "$file" >&2
        exit 1
    fi
done

# Reject an application that would extend into the following data partition.
"$python_bin" - "$repo_dir/firmware/partitions.csv" \
    "$image_dir/weather-forecast-firmware.bin" <<'PYTHON'
import csv
from pathlib import Path
import sys

with open(sys.argv[1]) as partitions:
    rows = csv.reader(line for line in partitions if not line.lstrip().startswith("#"))
    app = next(row for row in rows if row and row[0].strip() == "app0")
capacity = int(app[4].strip(), 0)
size = Path(sys.argv[2]).stat().st_size
if size > capacity:
    sys.exit(f"Application image is {size} bytes, exceeding the {capacity}-byte app0 partition. Nothing was flashed.")
PYTHON

# Fail before writing if the target chip is not a classic ESP32.
"$python_bin" -m esptool --chip esp32 --port "$port" flash_id
# Write only bootloader, matching partition table, and application sectors.
# esptool verifies each write and resets into the new firmware afterwards.
exec "$python_bin" -m esptool --chip esp32 --port "$port" --baud 460800 \
    write_flash --flash_mode dio --flash_freq 40m --flash_size 4MB \
    0x1000 "$image_dir/bootloader.bin" \
    0x8000 "$image_dir/partition-table.bin" \
    0x10000 "$image_dir/weather-forecast-firmware.bin"
