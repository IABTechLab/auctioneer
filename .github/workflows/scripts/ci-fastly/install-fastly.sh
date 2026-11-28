#!/usr/bin/env bash
set -euo pipefail
version="v$(awk '$1 == "fastly" { print $2 }' .tool-versions)"
curl -fsSL -o /tmp/fastly.tar.gz \
  "https://github.com/fastly/cli/releases/download/${version}/fastly_${version}_linux-amd64.tar.gz"
echo "0879ac89d8bfa73ab537e6f9d0bfbf58382ba0c2c45cb51a9a3c898dc136c7c4  /tmp/fastly.tar.gz" | sha256sum --check
tar -xzf /tmp/fastly.tar.gz -C /tmp
sudo mv /tmp/fastly /usr/local/bin/fastly
fastly version
