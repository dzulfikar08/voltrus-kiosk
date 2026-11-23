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
  (cd "$tmp" && echo "${sum,,}  ort.tgz" | sha256sum -c -)
  # Relative paths: GNU tar reads "D:/..." as host "D".
  (cd "$tmp" && tar xzf ort.tgz)
  dll="$(find "$tmp" -name DirectML.dll | head -1)"
  [ -n "$dll" ] || { echo "DirectML.dll not in $url" >&2; exit 1; }
fi
mkdir -p "$dest"
cp "$dll" "$dest/"
echo "DirectML.dll -> $dest"
