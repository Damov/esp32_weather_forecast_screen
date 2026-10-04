# Firmware release licenses

The project's own code is MIT-licensed. Embedded third-party software, fonts,
icons and certificate data retain their own licenses. Distribute firmware with
its accompanying license materials; a bare `.bin` is not the documented release
format. This workflow checks evidence coverage and integrity, not legal
compliance, copyright ownership or permission to use online services.

## Collected evidence

[Third-party notices](licensing/THIRD-PARTY-NOTICES.txt) describe the components and
include their original notices and source origins. [The manifest](licensing/manifest.json) records
versions, selected license alternatives, source origins and SHA-256 checksums.
The format-3 manifest also records the application ELF hash and application
source/configuration hashes; packaging rejects a different ELF or changed inputs.
The collection covers the ESP32 target's normal dependencies and relevant
code-generation providers, currently 67 package versions. Build/test-only
packages and unrelated platforms are excluded. Macro providers are retained
conservatively when their generated code could be included; the timestamp-only
`build-time` macro is explicitly excluded. Target dependencies can still be
optimized out; this is target graph coverage rather than a claim that every
package contributes bytes to the binary.

SDK coverage uses the application and bootloader memory maps, excluding the
map's discarded sections and cross-reference table. Original notices and source
headers are retained for the relevant components and compilation inputs. Only
compiler runtime notices are included, not licenses for the compiler's build
tools. The Rust standard-library report and notices within a relevant SDK
component can conservatively cover additional platform variants.

License texts and attributions are combined into one readable text document.
Identical bodies are stored once while preserving every associated evidence
label and source origin. The manifest records hashes and byte ranges for the
original notice bodies. File-level upstream notices take precedence over
summaries.

MIT is selected where offered by Rust dependencies, with supplemental Unicode
notices retained. Other selections are recorded per package. Mbed TLS uses its
Apache-2.0 option. GCC runtime material retains its original terms and the GCC
Runtime Library Exception; the standard free Rust/GCC build process must remain
eligible for that exception.

Most notices come from checksum-verified crate archives. Missing notices are
retrieved from immutable upstream revisions identified by the package's VCS
metadata. `void` 1.0.2 declares MIT in its README, with the author's name, but
supplies no standalone copyright notice. Its original declarations and standard
MIT terms are preserved with an explicit evidence note. No copyright holder or
year has been invented. This documents available evidence, not a certification
of rights.

Certificate material remains MPL-2.0. The exact SDK PEM source input, converter,
build configuration, MPL text and provenance are retained under
[certificate sources](licensing/sources/certificates/README.txt). Recipients can
modify that input and rebuild the firmware. Preserve these materials and their
notices when redistributing. The project's MIT license does not replace MPL.

Open-Meteo and GeoNames data attribution remains CC BY 4.0. The free Open-Meteo
service is for non-commercial use; MIT does not grant commercial API access.
Freenove's other resources retain their own terms and are not covered by the
project's MIT license.

## Check without changing files

From the repository root:

```sh
python3 tools/license_release.py check
python3 -m unittest discover -s tools/tests -p 'test_license_release.py'
```

`check` needs no network, SDK, compiler or additional Python packages. It checks
the reviewed inputs and retained evidence; it does not rebuild the firmware.
Missing or altered evidence and changed dependency/configuration inputs fail.

## Refresh the evidence after a reviewed change

First review dependency/license changes and build with `./build.sh --locked`.
Load your normal Rust/ESP environment as described in the main README. The SDK
build directory is the `out/build` directory below the current `esp-idf-sys`
build output. Its `project_description.json` gives the compiler path. Supply the
compiler distribution root containing both `bin/` and `share/licenses/`.
For the current build layout:

```sh
python3 tools/license_release.py update \
  --idf-build-dir firmware/target/xtensa-esp32-espidf/release/build/esp-idf-sys-039f6d788fc2a729/out/build \
  --toolchain-dir firmware/.embuild/espressif/tools/xtensa-esp-elf/esp-14.2.0_20260121/xtensa-esp-elf \
  --firmware-dir firmware/target/xtensa-esp32-espidf/release
python3 tools/license_release.py check
```

Build hashes and toolchain directory names can change; use the paths belonging
to your build. `update` explicitly uses the network and atomically prepares a
complete replacement collection before installing it. It verifies the reviewed
ESP-IDF/submodule revisions and crate checksums. It does not silently accept a
new lockfile: update and review the dependency inventory first. The checked-in `licensing-policy.json` pins the reviewed SDK/compiler/Rust
versions, target-package selection and explicit notice/code-generation
exceptions. A new SDK or runtime version requires review of its actual licensing terms and an explicit policy
update before refreshing the evidence. Review the resulting diff and original
notices, not just the generated success message.

## Package the existing firmware images

Generate `weather-forecast-firmware.bin` from the Rust ELF using the existing
[user-guide instructions](../USER_GUIDE.md#installing-the-firmware-under-linux).
The firmware directory must contain that ELF plus `bootloader.bin`,
`partition-table.bin` and `weather-forecast-firmware.bin`. Use the ESP-IDF Python
environment containing esptool:

```sh
python3 tools/license_release.py package \
  --idf-build-dir firmware/target/xtensa-esp32-espidf/release/build/esp-idf-sys-039f6d788fc2a729/out/build \
  --toolchain-dir firmware/.embuild/espressif/tools/xtensa-esp-elf/esp-14.2.0_20260121/xtensa-esp-elf \
  --firmware-dir firmware/target/xtensa-esp32-espidf/release \
  --esptool-python firmware/.embuild/espressif/python_env/idf5.5_py3.10_env/bin/python \
  --version 0.1.0 \
  --output-dir firmware/target/license-release
```

This creates `weather-forecast-0.1.0.zip` and `.tar.gz`, each containing the same
three flash images, project license, third-party evidence, certificate sources,
standalone installation instructions, build provenance and `SHA256SUMS`.
Existing output archives are not overwritten. Nothing is flashed or published.

Packaging verifies SDK/runtime evidence, compares the bootloader and partition
table with their build outputs, and regenerates the application image from the
Rust ELF in a temporary directory to verify its contents. ESP-IDF's intermediate
`libespidf.bin` is never substituted for the Rust firmware. The classic ESP32,
4 MiB flash and recorded partition configuration are the supported layout.

Publish complete archives as the firmware downloads. If distributing images
separately, provide the same accompanying license and source materials and make
them clearly available to recipients. Release wording should say:

> Project code is MIT-licensed. Included third-party components retain their
> respective licenses; see the accompanying license files.

## Automatic GitHub builds and releases

The `Firmware release` GitHub Actions workflow builds on pushed version tags
such as `v0.1.0` or `v0.1.0-rc.1`. Other `v*` tags fail validation. A successful
tag build publishes complete firmware ZIP/tar.gz packages and an external
`SHA256SUMS` file. GitHub supplies the tag's source ZIP and tar.gz automatically.
The firmware archives contain the three images, installation instructions,
notices, certificate sources and build provenance. Separate bare binary assets
are not published.

Use Actions → Firmware release → Run workflow for the first validation run.
Select a branch containing the workflow. Manual runs upload test artifacts for
30 days and never publish a release. The workflow must exist on GitHub's default
branch for the manual trigger to be available. Ordinary branch pushes do not
start firmware builds. No personal access token is required; only the tag-only
publication job receives repository contents write permission.

CI installs pinned ESP Rust 1.97.0.0 and builds with `--locked`, without caching
application outputs. `tools/ci_release.py` discovers the active build paths from
Cargo output. It collects evidence in a temporary directory after compilation
and packages only the recorded ELF. All licensing policy checks still apply;
new dependency or SDK/runtime versions require review. Repository files are not
rewritten. All license-tool commands accept `--collection-dir` to select a
build-specific collection; the default remains `docs/licensing`.

Source and ELF hashes record the build supplied at collection time. They are
integrity evidence, not proof that an arbitrary manually supplied ELF was built
from those sources. CI establishes that association by running the locked build
and collecting immediately from its reported application artifact.

The release stays a draft until both archives and checksums have uploaded.
Existing releases are never overwritten. If an upload fails after draft creation,
inspect the draft and delete it manually before retrying; the workflow will not
replace assets automatically. A first hosted manual run is still needed to
validate GitHub runner setup before creating a release tag.
