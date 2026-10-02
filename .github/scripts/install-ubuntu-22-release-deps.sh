#!/usr/bin/env bash
# Ubuntu 22.04 release-image dependencies.
#
# pipewire-rs 0.10 (via xcap) needs PipeWire 1.0 headers. Jammy's 0.3.48
# headers fail to compile libspa. The pipewire-debian upstream PPA builds
# 1.0.x against jammy glibc, so the AppImage still runs on Ubuntu 22.04.
#
# Do not call add-apt-repository. It imports the PPA key over HKP
# (keyserver.ubuntu.com:11371) with no timeout, and that socket can stay open
# until GitHub's 6-hour job limit. The v0.9.22 Ubuntu release job sat in this
# step for more than 5 hours; the same step finished in about 50 seconds for
# v0.9.21. Fetch the pinned key over HTTPS and bound every apt transfer.

set -euo pipefail

export DEBIAN_FRONTEND=noninteractive

# shellcheck disable=SC1091
. /etc/os-release
if [ "${VERSION_CODENAME:-}" != "jammy" ]; then
  echo "install-ubuntu-22-release-deps.sh requires Ubuntu 22.04 (jammy), found ${VERSION_CODENAME:-unknown}." >&2
  exit 1
fi

if [ -z "${GITHUB_ENV:-}" ]; then
  echo "GITHUB_ENV is not set. This script is meant to run in GitHub Actions." >&2
  exit 1
fi

apt_get=(
  apt-get
  -o Acquire::Retries=3
  -o Acquire::http::Timeout=30
  -o Acquire::https::Timeout=30
)

sudo "${apt_get[@]}" update
sudo "${apt_get[@]}" install -y ca-certificates curl gnupg

# Launchpad signing key for ppa:pipewire-debian/pipewire-upstream.
# If Launchpad rotates it, refresh signing_key_fingerprint from:
# https://api.launchpad.net/devel/~pipewire-debian/+archive/ubuntu/pipewire-upstream
pipewire_ppa_fingerprint=FC43B7352BCC0EC8AF2EEB8B25088A0359807596
keyring=/etc/apt/keyrings/pipewire-upstream.gpg
list=/etc/apt/sources.list.d/pipewire-upstream.list

tmp_key=$(mktemp)
trap 'rm -f "$tmp_key"' EXIT

curl -fsSL --retry 5 --retry-delay 2 --retry-all-errors --max-time 30 \
  "https://keyserver.ubuntu.com/pks/lookup?op=get&search=0x${pipewire_ppa_fingerprint}" \
  -o "$tmp_key"

sudo mkdir -p /etc/apt/keyrings
sudo gpg --batch --yes --dearmor --output "$keyring" "$tmp_key"
sudo chmod 644 "$keyring"

installed=$(gpg --show-keys --with-colons "$keyring" | awk -F: '$1 == "fpr" { print $10; exit }')
if [ "$installed" != "$pipewire_ppa_fingerprint" ]; then
  echo "PipeWire PPA key fingerprint mismatch: expected ${pipewire_ppa_fingerprint}, got ${installed:-none}." >&2
  exit 1
fi

echo "deb [signed-by=${keyring}] https://ppa.launchpadcontent.net/pipewire-debian/pipewire-upstream/ubuntu jammy main" \
  | sudo tee "$list" >/dev/null

sudo "${apt_get[@]}" update
sudo "${apt_get[@]}" install -y \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libappindicator3-dev \
  librsvg2-dev \
  patchelf \
  libpipewire-0.3-dev \
  libclang-dev \
  libegl-dev \
  libgbm-dev \
  libxkbcommon-dev

# prepare-tauri-build.cjs --appimage-ort stages this directory before cargo.
# pyke's static ONNX Runtime needs glibc 2.38; this shared build does not.
# Pass --config via tauri-action args (see Build the app step).
{
  echo "ORT_LIB_PATH=${GITHUB_WORKSPACE}/src-tauri/onnxruntime-linux/lib"
  echo "ORT_PREFER_DYNAMIC_LINK=1"
  echo "LD_LIBRARY_PATH=${GITHUB_WORKSPACE}/src-tauri/onnxruntime-linux/lib"
  echo "LIBRAGENT_APPIMAGE_ORT=1"
} >> "$GITHUB_ENV"
