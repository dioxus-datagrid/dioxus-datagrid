#!/usr/bin/env bash
# System libraries the desktop and mobile targets build against on Linux.
#
# Dioxus renders desktop and mobile through a WebView, so anything that enables
# those features — including `--all-features` across the workspace, which pulls
# in the playground's desktop feature — needs GTK and WebKitGTK present. Without
# them the build fails in glib-sys/gobject-sys with a pkg-config error.
set -euo pipefail

if [[ "$(uname -s)" != "Linux" ]]; then
  echo "Not Linux; nothing to install."
  exit 0
fi

sudo apt-get update
sudo apt-get install -y --no-install-recommends \
  libglib2.0-dev \
  libgtk-3-dev \
  libwebkit2gtk-4.1-dev \
  libxdo-dev \
  libayatana-appindicator3-dev \
  librsvg2-dev
