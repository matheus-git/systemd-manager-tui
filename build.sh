#!/bin/bash
#
# Script to build the project and generate .deb and .rpm packages for multiple architectures
# Author: matheus-git <mathiew0@gmail.com>
# 
YELLOW_BOLD="\033[1;33m"
RESET="\033[0m"

PGO_DIR="/tmp/pgo-data"
MERGED_PROFILE="$(pwd)/merged.profdata"

#echo -e "${YELLOW_BOLD}\nCleaning old builds${RESET}"
#cargo clean
#rm -rf "$PGO_DIR" "$MERGED_PROFILE"

echo -e "${YELLOW_BOLD}\ncargo build --release${RESET}"
RUSTFLAGS="-Cprofile-generate=/tmp/pgo-data" cargo build --release

echo -e "${YELLOW_BOLD}\nRunning binary to collect profiles${RESET}"
./target/release/systemd-manager-tui

echo -e "${YELLOW_BOLD}\nMerging profiles${RESET}"
llvm-profdata merge -o "$MERGED_PROFILE" "$PGO_DIR"/*.profraw

echo -e "${YELLOW_BOLD}\nFinal release build using PGO${RESET}"
RUSTFLAGS="-Cprofile-use=$MERGED_PROFILE -Cllvm-args=-pgo-warn-missing-function" cargo build --release

echo -e "${YELLOW_BOLD}\nstrip target/release/systemd-manager-tui${RESET}"
strip target/release/systemd-manager-tui

if ! command -v cross &> /dev/null; then
    echo -e "${YELLOW_BOLD}\nInstalling cross${RESET}"
    cargo install cross
fi

echo -e "${YELLOW_BOLD}\ncross build --release --target x86_64-unknown-linux-musl${RESET}"
cross build --release --target x86_64-unknown-linux-musl

echo -e "${YELLOW_BOLD}\nstrip target/x86_64-unknown-linux-musl/release/systemd-manager-tui${RESET}"
strip target/x86_64-unknown-linux-musl/release/systemd-manager-tui

echo -e "${YELLOW_BOLD}\ncross build --release --target aarch64-unknown-linux-musl${RESET}"
cross build --release --target aarch64-unknown-linux-musl 

echo -e "${YELLOW_BOLD}\nstrip target/aarch64-unknown-linux-musl/release/systemd-manager-tui${RESET}"
strip target/aarch64-unknown-linux-musl/release/systemd-manager-tui

if ! command -v cargo-deb &> /dev/null; then
    echo -e "${YELLOW_BOLD}\nInstalling cargo-deb${RESET}"
    cargo install cargo-deb
fi 

echo -e "${YELLOW_BOLD}\ncargo deb${RESET}"
cargo deb --target x86_64-unknown-linux-musl --no-build

echo -e "${YELLOW_BOLD}\ncargo deb --target aarch64-unknown-linux-musl${RESET}"
cargo deb --target aarch64-unknown-linux-musl --no-build

if ! command -v cargo-generate-rpm &> /dev/null && ! command -v cargo generate-rpm &> /dev/null; then
    echo -e "${YELLOW_BOLD}\nInstalling cargo-rpm${RESET}"
    cargo install cargo-rpm
fi 

echo -e "${YELLOW_BOLD}\ncargo generate-rpm --target x86_64-unknown-linux-musl${RESET}"
cargo generate-rpm --target x86_64-unknown-linux-musl

echo -e "${YELLOW_BOLD}\ncargo generate-rpm --target aarch64-unknown-linux-musl${RESET}"
cargo generate-rpm --target aarch64-unknown-linux-musl

echo -e "${YELLOW_BOLD}\nBuilds and packages generated in the following directories:${RESET}"
echo -e "${YELLOW_BOLD}  - target/release/${RESET}"
echo -e "${YELLOW_BOLD}  - target/x86_64-unknown-linux-musl/release/${RESET}"
echo -e "${YELLOW_BOLD}  - target/aarch64-unknown-linux-musl/release/${RESET}"
echo -e "${YELLOW_BOLD}  - target/x86_64-unknown-linux-musl/debian/${RESET}"
echo -e "${YELLOW_BOLD}  - target/aarch64-unknown-linux-musl/debian/${RESET}"
echo -e "${YELLOW_BOLD}  - target/x86_64-unknown-linux-musl/generate-rpm/${RESET}"
echo -e "${YELLOW_BOLD}  - target/aarch64-unknown-linux-musl/generate-rpm/${RESET}"
