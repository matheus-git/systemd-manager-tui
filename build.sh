#!/usr/bin/env bash

set -Eeuo pipefail

YELLOW_BOLD="\033[1;33m"
RESET="\033[0m"
PACKAGE_NAME="systemd-manager-tui"
PROJECT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"

TARGETS=(
    "x86_64-unknown-linux-musl"
    "aarch64-unknown-linux-musl"
)

declare -A RPM_ARCH=(
    [x86_64-unknown-linux-musl]="x86_64"
    [aarch64-unknown-linux-musl]="aarch64"
)

build_native=true
build_cross=true
build_deb=true
build_rpm=true
install_tools=true

usage() {
    cat <<'EOF'
Usage: ./build.sh [OPTION]

Build binaries and Linux packages without editing Cargo.toml per architecture.

Options:
  --rpm-only       Generate RPMs from existing cross-compiled binaries
  --deb-only       Generate DEBs from existing cross-compiled binaries
  --packages-only  Generate DEBs and RPMs without compiling
  --skip-native    Skip the native release build
  --target TRIPLE  Process only one supported target triple
  --no-install     Do not automatically install missing packaging tools
  -h, --help       Show this help
EOF
}

log() {
    printf "%b\n" "${YELLOW_BOLD}$1${RESET}"
}

ensure_tool() {
    local executable="$1"
    local crate="$2"

    if command -v "$executable" >/dev/null 2>&1; then
        return
    fi
    if [[ "$install_tools" == false ]]; then
        printf "Missing required tool: %s (install with: cargo install %s)\n" "$executable" "$crate" >&2
        exit 1
    fi
    log "Installing ${crate}"
    cargo install "$crate" --locked
}

while (($# > 0)); do
    case "$1" in
        --rpm-only)
            build_native=false
            build_cross=false
            build_deb=false
            ;;
        --deb-only)
            build_native=false
            build_cross=false
            build_rpm=false
            ;;
        --packages-only)
            build_native=false
            build_cross=false
            ;;
        --skip-native)
            build_native=false
            ;;
        --target)
            if (($# < 2)); then
                printf "--target requires a target triple\n" >&2
                exit 2
            fi
            if [[ -z "${RPM_ARCH[$2]+supported}" ]]; then
                printf "Unsupported target: %s\n" "$2" >&2
                exit 2
            fi
            TARGETS=("$2")
            shift
            ;;
        --no-install)
            install_tools=false
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            printf "Unknown option: %s\n\n" "$1" >&2
            usage >&2
            exit 2
            ;;
    esac
    shift
done

cd "$PROJECT_DIR"

if [[ "$build_native" == true ]]; then
    log "Building native release binary"
    cargo build --release --locked
fi

if [[ "$build_cross" == true ]]; then
    ensure_tool cargo-zigbuild cargo-zigbuild
    for target in "${TARGETS[@]}"; do
        log "Building release binary for ${target}"
        cargo zigbuild --release --locked --target "$target"
    done
fi

if [[ "$build_deb" == true ]]; then
    ensure_tool cargo-deb cargo-deb
    for target in "${TARGETS[@]}"; do
        binary="target/${target}/release/${PACKAGE_NAME}"
        if [[ ! -x "$binary" ]]; then
            printf "Missing binary: %s. Run ./build.sh first.\n" "$binary" >&2
            exit 1
        fi
        deb_output="target/${target}/debian"
        mkdir -p "$deb_output"
        log "Generating DEB for ${target}"
        cargo deb --locked --target "$target" --no-build --output "${deb_output}/"
    done
fi

if [[ "$build_rpm" == true ]]; then
    ensure_tool cargo-generate-rpm cargo-generate-rpm
    for target in "${TARGETS[@]}"; do
        binary="target/${target}/release/${PACKAGE_NAME}"
        if [[ ! -x "$binary" ]]; then
            printf "Missing binary: %s. Run ./build.sh first.\n" "$binary" >&2
            exit 1
        fi

        rpm_metadata="assets = [{ source = \"${binary}\", dest = \"/usr/bin/${PACKAGE_NAME}\", mode = \"755\" }]"
        log "Generating RPM for ${target} (${RPM_ARCH[$target]})"
        cargo generate-rpm \
            --target "$target" \
            --arch "${RPM_ARCH[$target]}" \
            --set-metadata "$rpm_metadata"
    done
fi

log "Build completed"
if [[ "$build_native" == true ]]; then
    printf "  Native binary: target/release/\n"
fi
for target in "${TARGETS[@]}"; do
    printf "  %-28s target/%s/release/\n" "${target}:" "$target"
    if [[ "$build_deb" == true ]]; then
        printf "  %-28s target/%s/debian/\n" "${target} DEB:" "$target"
    fi
    if [[ "$build_rpm" == true ]]; then
        printf "  %-28s target/%s/generate-rpm/\n" "${target} RPM:" "$target"
    fi
done
