#!/usr/bin/env bash
set -euo pipefail
version="v$(awk '$1 == "viceroy" { print $2 }' .tool-versions)"
curl -fsSL -o /tmp/viceroy.tar.gz \
  "https://github.com/fastly/Viceroy/releases/download/${version}/viceroy_${version}_linux-amd64.tar.gz"
echo "52ed3c3ed59d926c81015d637509942c3195443c02c2eebbded9125484336a9e  /tmp/viceroy.tar.gz" | sha256sum --check
tar -xzf /tmp/viceroy.tar.gz -C /tmp
sudo mv /tmp/viceroy /usr/local/bin/viceroy
viceroy --version
