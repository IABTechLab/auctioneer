#!/usr/bin/env bash
set -euo pipefail
version="v$(awk '$1 == "spin" { print $2 }' .tool-versions)"
curl -fsSL https://developer.fermyon.com/downloads/install.sh | bash -s -- -v "$version"
sudo mv ./spin /usr/local/bin/spin
spin --version
