//! MCP tools: thin wrappers over control-channel methods.

use serde_json::{Value, json};

use crate::Backend;

/// A tool result.
pub struct ToolResult {
    pub value: Result<Value, String>,
}

impl ToolResult {
    pub fn to_value(&self) -> Value {
        match &self.value {
            Ok(v) => {
                let text = if let Some(s) = v.as_str() { s.to_string() } else { serde_json::to_string_pretty(v).unwrap_or_default() };
                json!({"content": [{"type": "text", "text": text}], "structuredContent": {"result": v}, "isError": false})
            }
            Err(e) => json!({"content": [{"type": "text", "text": e}], "isError": true}),
        }
    }
}

fn obj(props: Value, required: &[&str]) -> Value {
    json!({"type": "object", "properties": props, "required": required})
}

/// The tools we offer.
pub fn tool_definitions() -> Value {
    json!([
        {"name": "list_commands", "title": "List commands", "description": "Every ScriptCraft command: id, label, menu location, shortcut, params, whether it changes the script, enabled.", "inputSchema": obj(json!({}), &[])},
        {"name": "execute", "title": "Run a command", "description": "Run one command by id (see list_commands), e.g. {command:\"text.insert\", params:{text:\"INT. DINER - NIGHT\"}}, {command:\"text.enter\"}, {command:\"element.set\", params:{element:\"character\"}}.", "inputSchema": obj(json!({"command": {"type": "string"}, "params": {"type": "object"}}), &["command"])},
        {"name": "batch", "title": "Run several commands", "description": "Run commands in order; stops at the first error. {steps:[{command, params}]}", "inputSchema": obj(json!({"steps": {"type": "array", "items": {"type": "object"}}}), &["steps"])},
        {"name": "inspect_script", "title": "Inspect the script", "description": "Paragraphs with element, text and page; selection; title page; page count.", "inputSchema": obj(json!({"from": {"type": "integer"}, "count": {"type": "integer"}}), &[])},
        {"name": "get_fountain", "title": "Script as Fountain", "description": "The whole script as Fountain plain text.", "inputSchema": obj(json!({}), &[])},
        {"name": "set_fountain", "title": "Replace script from Fountain", "description": "Replace the whole script with Fountain text (undoable).", "inputSchema": obj(json!({"text": {"type": "string"}}), &["text"])},
        {"name": "reports", "title": "Reports", "description": "Character, scene and location reports.", "inputSchema": obj(json!({}), &[])},
        {"name": "screenshot", "title": "Screenshot the app", "description": "PNG of the ScriptCraft window saved to `path` (needs the app: scriptcraft-cli mcp --connect).", "inputSchema": obj(json!({"path": {"type": "string"}}), &["path"])},
    ])
}

fn exec(b: &mut dyn Backend, id: &str, params: Value) -> Result<Value, String> {
    b.call("engine.execute", json!({"command": id, "params": params}))
}

/// Call a tool.
pub fn call_tool(b: &mut dyn Backend, name: &str, args: &Value) -> ToolResult {
    let value = match name {
        "list_commands" => b.call("engine.commands", json!({})),
        "execute" => match args.get("command").and_then(Value::as_str) {
            Some(id) => exec(b, id, args.get("params").cloned().unwrap_or(json!({}))),
            None => Err("missing `command`".into()),
        },
        "batch" => match args.get("steps").and_then(Value::as_array) {
            Some(steps) => {
                let mut out = Vec::new();
                let mut err = None;
                for (i, st) in steps.iter().enumerate().take(10_000) {
                    let Some(id) = st.get("command").and_then(Value::as_str) else {
                        err = Some(format!("step {i}: missing `command`"));
                        break;
                    };
                    match exec(b, id, st.get("params").cloned().unwrap_or(json!({}))) {
                        Ok(v) => out.push(v),
                        Err(e) => {
                            err = Some(format!("step {i} ({id}): {e}"));
                            break;
                        }
                    }
                }
                match err {
                    Some(e) => Err(format!("{e} (after {} successful steps)", out.len())),
                    None => Ok(Value::Array(out)),
                }
            }
            None => Err("missing `steps`".into()),
        },
        "inspect_script" => exec(b, "document.inspect", args.clone()),
        "get_fountain" => exec(b, "document.fountain", json!({})).map(|v| v.get("text").cloned().unwrap_or(Value::Null)),
        "set_fountain" => exec(b, "document.setFountain", args.clone()),
        "reports" => (|| {
            Ok(json!({
                "characters": exec(b, "report.characters", json!({}))?,
                "scenes": exec(b, "report.scenes", json!({}))?,
                "locations": exec(b, "report.locations", json!({}))?,
            }))
        })(),
        "screenshot" => b.call("ui.screenshot", args.clone()),
        other => Err(format!("unknown tool `{other}`")),
    };
    ToolResult { value }
}
