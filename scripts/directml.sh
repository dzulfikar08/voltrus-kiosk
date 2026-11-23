#!/usr/bin/env bash
# directml.sh DEST_DIR: put DirectML.dll (which the statically linked ONNX Runtime loads on
# Windows) into DEST_DIR. Takes it from ort-sys's download cache, or fetches the same archive
# when a restored build cache skipped the download.
set -euo pipefail
dest="$1"
dll="$(find "${LOCALAPPDATA:-/nonexistent}/ort.pyke.io" -name DirectML.dll 2>/dev/null | head -1 || true)"
if [ -z "$dll" ]; then
  url="https://cdn.pyke.io/0/pyke:ort-rs/ms@1.22.0/x86_64-pc-windows-msvc.tgz"
  sum="540D19B3379FDA6FB8F7280D8C15EFDE20ED225A67A357A6DAE38C4300FE190D"
  tmp="${RUNNER_TEMP:-/tmp}/ort-win"
  mkdir -p "$tmp"
  curl -fsSL "$url" -o "$tmp/ort.tgz"
  echo "${sum,,}  $tmp/ort.tgz" | sha256sum -c -
  tar xzf "$tmp/ort.tgz" -C "$tmp"
  dll="$tmp/onnxruntime/lib/DirectML.dll"
fi
mkdir -p "$dest"
cp "$dll" "$dest/"
echo "DirectML.dll -> $dest"
