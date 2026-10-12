# Control protocol

The desktop app can be driven over a loopback JSON-lines socket — the same transport the MCP bridge
(`scriptcraft-cli mcp --connect`) uses.

```sh
scriptcraft --control 7990            # or SCRIPTCRAFT_CONTROL_PORT=7990
```

## Keys

On start the app writes a random 256-bit key to `<settings>/control-key.local-<port>` (mode 0600;
removed on a clean exit). Settings: macOS `~/Library/Application Support/ScriptCraft`, Windows
`%APPDATA%\ScriptCraft`, else `$XDG_CONFIG_HOME/scriptcraft`. Every request must carry it as
`key`; requests with a missing or wrong key are refused. The server binds 127.0.0.1 only.

## Requests

One JSON object per line; one reply per line, `{"id", "ok": true, "result"}` or `{"id", "ok": false, "error"}`.

```json
{"id":1,"key":"…","method":"engine.execute","params":{"command":"text.insert","params":{"text":"INT. DINER - NIGHT"}}}
{"id":2,"key":"…","method":"ui.screenshot","params":{"path":"/tmp/shot.png"}}
```

| Method | Params | Does |
|---|---|---|
| `engine.execute` | `{command, params}` | run any command (engine or `ui.*`) |
| `engine.commands` | | every command with id, label, location, shortcut, params, enabled |
| `document.inspect` | `{from?, count?}` | paragraphs, selection, title page, page count |
| `ui.inspect` | | view, theme, zoom, page rectangles, caret, SmartType state |
| `ui.key` | `{key, shift?, alt?, cmd?}` | a key press through egui (`Enter`, `Tab`, `A`, `ArrowUp`…) |
| `ui.text` | `{text}` | typed text through egui |
| `ui.click` | `{x, y, count?, shift?}` | a pointer click in window points |
| `ui.screenshot` | `{path?}` | PNG of the window (base64 `png` in the reply without `path`) |

## Headless screenshots

`cargo run -p scriptcraft-ui-egui --example ui_shot -- script.jsonl` renders the window offscreen
(wgpu) and accepts the same methods, plus `{"shot": "/abs/out.png"}` and `{"steps": n}`. The
README images are made this way.
