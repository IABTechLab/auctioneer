#!/usr/bin/env bash
set -euo pipefail
version="v$(awk '$1 == "viceroy" { print $2 }' .tool-versions)"
curl -fsSL -o /tmp/viceroy.tar.gz \
  "https://github.com/fastly/Viceroy/releases/download/${version}/viceroy_${version}_linux-amd64.tar.gz"
echo "03f66f27275e8adc0c1857c4f571bd6f5ae997e38cc43760b19c6840a92b298d  /tmp/viceroy.tar.gz" | sha256sum --check
tar -xzf /tmp/viceroy.tar.gz -C /tmp
sudo mv /tmp/viceroy /usr/local/bin/viceroy
viceroy --version
