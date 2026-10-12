#!/bin/sh
# Records docs/demo/pr33-tour.mp4: the in-app tour (ui_demo), a real terminal run for #11 and
# the browser build in Chrome for #12. Needs: ffmpeg, node 22+, Google Chrome, wasm-bindgen-cli.
# Usage: docs/demo/record.sh [work-dir]
set -eu
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
WORK=${1:-$(mktemp -d)}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$ROOT/.target}
cd "$ROOT"
cargo build -q --release -p scriptcraft -p scriptcraft-cli -p scriptcraft-ui-egui --example ui_demo
cargo xtask web
# #12: drive the browser build in headless Chrome over the DevTools protocol.
mkdir -p "$WORK/web_frames"
python3 -m http.server 8765 --bind 127.0.0.1 -d apps/scriptcraft-web/dist >/dev/null 2>&1 & HTTP=$!
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new --remote-debugging-port=9333 \
  --use-angle=swiftshader --enable-unsafe-swiftshader --window-size=1280,800 --user-data-dir="$WORK/chrome" about:blank >/dev/null 2>&1 & CHROME=$!
sleep 3
node docs/demo/browser.mjs "$WORK/web_frames"
kill $CHROME $HTTP
# #11: the transcript in docs/demo/pr33-terminal.txt is a real run (see the PR description).
sed -e "s|@TRANSCRIPT@|$ROOT/docs/demo/pr33-terminal.txt|" -e "s|@WEB_FRAMES@|$WORK/web_frames|" docs/demo/pr33-tour.jsonl > "$WORK/tour.jsonl"
"$CARGO_TARGET_DIR/release/examples/ui_demo" "$WORK/tour.jsonl" "$WORK/frames"
ffmpeg -loglevel error -y -framerate 15 -i "$WORK/frames/f%05d.png" -c:v libx264 -preset slow -crf 24 -pix_fmt yuv420p -vf "scale=1440:-2" -movflags +faststart docs/demo/pr33-tour.mp4
echo "wrote docs/demo/pr33-tour.mp4"
