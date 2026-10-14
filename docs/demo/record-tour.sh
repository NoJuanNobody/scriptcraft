#!/bin/sh
# Records a narrated tour video of the real app from a ui_demo script (see the top of
# crates/ui-egui/examples/ui_demo.rs for the actions).
# Usage: docs/demo/record-tour.sh docs/demo/v03-tour.jsonl docs/demo/v03-tour.mp4 [work-dir]
# In the script, @ROOT@ is the repo and @WORK@ a fresh scratch folder for this run (<work-dir>/run). Needs ffmpeg; swift (macOS) for
# `run` lines that render a PDF with docs/demo/pdf-pages.swift.
set -eu
[ $# -ge 2 ] || { echo "usage: $0 tour.jsonl out.mp4 [work-dir]" >&2; exit 2; }
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
TOUR=$(cd "$(dirname "$1")" && pwd)/$(basename "$1")
OUT=$2
WORK=${3:-$(mktemp -d)}
mkdir -p "$WORK"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/.target}
# Never touch a real settings profile.
export SCRIPTCRAFT_CONFIG_DIR=
cd "$ROOT"
cargo build -q --release -p scriptcraft-ui-egui --example ui_demo
RUN="$WORK/run"
rm -rf "$RUN"
mkdir -p "$RUN"
sed -e "s|@ROOT@|$ROOT|g" -e "s|@WORK@|$RUN|g" "$TOUR" > "$RUN/tour.jsonl"
"$CARGO_TARGET_DIR/release/examples/ui_demo" "$RUN/tour.jsonl" "$RUN/frames"
ffmpeg -loglevel error -y -framerate 15 -i "$RUN/frames/f%05d.png" -c:v libx264 -preset slow -crf 24 \
  -pix_fmt yuv420p -vf "scale=1440:-2" -movflags +faststart "$OUT"
echo "frames: $RUN/frames ($(ls "$RUN/frames" | wc -l | tr -d ' '))"
echo "wrote $OUT"
