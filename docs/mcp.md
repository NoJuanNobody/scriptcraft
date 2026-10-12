# MCP server

```sh
scriptcraft-cli mcp                              # headless editing session
scriptcraft-cli mcp --connect 127.0.0.1:7990     # drive the running app (start it with --control 7990)
```

JSON-RPC 2.0 over stdio, protocol `2025-06-18` (also accepts `2025-03-26`, `2024-11-05`).

Example client config:

```json
{"mcpServers": {"scriptcraft": {"command": "scriptcraft-cli", "args": ["mcp"]}}}
```

## Tools

| Tool | Arguments | Does |
|---|---|---|
| `list_commands` | | every command (`engine.commands`) |
| `execute` | `{command, params}` | run one command |
| `batch` | `{steps: [{command, params}]}` | run commands in order; stops at the first error |
| `inspect_script` | `{from?, count?}` | paragraphs with element, text, page |
| `get_fountain` | | the script as Fountain |
| `set_fountain` | `{text}` | replace the script from Fountain (undoable) |
| `reports` | | character, scene and location reports |
| `screenshot` | `{path}` | PNG of the app window (needs `--connect`) |

Resources: `scriptcraft://script` (inspect) and `scriptcraft://commands`.
