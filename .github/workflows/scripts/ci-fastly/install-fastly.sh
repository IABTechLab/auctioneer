#!/usr/bin/env bash
set -euo pipefail
version="v$(awk '$1 == "fastly" { print $2 }' .tool-versions)"
curl -fsSL -o /tmp/fastly.tar.gz \
  "https://github.com/fastly/cli/releases/download/${version}/fastly_${version}_linux-amd64.tar.gz"
echo "b1b89112f1f1291ff275393779ffdacba8724ac64091bda0fa6000ae06308459  /tmp/fastly.tar.gz" | sha256sum --check
tar -xzf /tmp/fastly.tar.gz -C /tmp
sudo mv /tmp/fastly /usr/local/bin/fastly
fastly version
