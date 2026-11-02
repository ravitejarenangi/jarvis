#!/usr/bin/env bash
# Downloads speech models JARVIS needs into the app's models folder.
# Usage: scripts/fetch-models.sh [--all]
set -euo pipefail

if [[ -n "${JARVIS_CONFIG_DIR:-}" ]]; then
  MODELS="$JARVIS_CONFIG_DIR/models"
elif [[ "$(uname)" == "Darwin" ]]; then
  MODELS="$HOME/Library/Application Support/jarvis/models"
else
  MODELS="${XDG_DATA_HOME:-$HOME/.local/share}/jarvis/models"
fi
mkdir -p "$MODELS"

BASE="https://huggingface.co/ggerganov/whisper.cpp/resolve/main"
fetch() {
  local name="$1"
  if [[ -s "$MODELS/$name" ]]; then
    echo "have $name"
    return
  fi
  echo "downloading $name"
  curl -fL --retry 3 -o "$MODELS/$name.part" "$BASE/$name"
  mv "$MODELS/$name.part" "$MODELS/$name"
}

fetch ggml-base.bin
if [[ "${1:-}" == "--all" ]]; then
  fetch ggml-small.bin
fi
echo "models in: $MODELS"
