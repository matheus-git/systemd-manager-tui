#!/bin/bash
#
# Script to build the project and generate .deb and .rpm packages for multiple architectures
# Author: matheus-git <mathiew0@gmail.com>
# 
YELLOW_BOLD="\033[1;33m"
RESET="\033[0m"

echo -e "${YELLOW_BOLD}\ncargo build --release${RESET}"
cargo build --release

echo -e "${YELLOW_BOLD}\nstrip target/release/systemd-manager-tui${RESET}"
strip target/release/systemd-manager-tui

echo -e "${YELLOW_BOLD}\ncross build --release --target x86_64-unknown-linux-musl${RESET}"
cross build --release --target x86_64-unknown-linux-musl

echo -e "${YELLOW_BOLD}\nstrip target/x86_64-unknown-linux-musl/release/systemd-manager-tui${RESET}"
strip target/x86_64-unknown-linux-musl/release/systemd-manager-tui

echo -e "${YELLOW_BOLD}\ncross build --release --target aarch64-unknown-linux-musl${RESET}"
cross build --release --target aarch64-unknown-linux-musl 

echo -e "${YELLOW_BOLD}\nstrip target/aarch64-unknown-linux-musl/release/systemd-manager-tui${RESET}"
strip target/aarch64-unknown-linux-musl/release/systemd-manager-tui

echo -e "${YELLOW_BOLD}\ncargo deb${RESET}"
cargo deb --target x86_64-unknown-linux-musl --no-build

echo -e "${YELLOW_BOLD}\ncargo deb --target aarch64-unknown-linux-musl${RESET}"
cargo deb --target aarch64-unknown-linux-musl --no-build

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
