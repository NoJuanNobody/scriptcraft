use serde_json::{Value, json};

use crate::{Headless, Server};

fn call(s: &mut Server, msg: Value) -> Value {
    serde_json::from_str(&s.handle_line(&msg.to_string()).unwrap()).unwrap()
}

#[test]
fn lifecycle_and_tools() {
    let mut s = Server::new(Box::new(Headless::default()));
    let r = call(&mut s, json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {"protocolVersion": "2025-06-18"}}));
    assert_eq!(r["result"]["serverInfo"]["name"], "scriptcraft");
    assert!(s.handle_line(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#).is_none());
    assert!(s.is_initialized());
    let tools = call(&mut s, json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}));
    assert!(tools["result"]["tools"].as_array().unwrap().len() >= 6);
    let steps = json!([
        {"command": "text.insert", "params": {"text": "int. diner - night"}},
        {"command": "text.enter"},
        {"command": "text.insert", "params": {"text": "Rain."}},
        {"command": "text.tab"},
        {"command": "text.insert", "params": {"text": "margo"}},
        {"command": "text.enter"},
        {"command": "text.insert", "params": {"text": "Coffee?"}},
    ]);
    let r = call(&mut s, json!({"jsonrpc": "2.0", "id": 3, "method": "tools/call", "params": {"name": "batch", "arguments": {"steps": steps}}}));
    assert_eq!(r["result"]["isError"], false, "{r}");
    let r = call(&mut s, json!({"jsonrpc": "2.0", "id": 4, "method": "tools/call", "params": {"name": "get_fountain", "arguments": {}}}));
    let text = r["result"]["content"][0]["text"].as_str().unwrap();
    assert!(text.contains("INT. DINER - NIGHT\n\nRain.\n\nMARGO\nCoffee?"), "{text}");
    let r = call(
        &mut s,
        json!({"jsonrpc": "2.0", "id": 5, "method": "tools/call", "params": {"name": "screenshot", "arguments": {"path": "/tmp/x.png"}}}),
    );
    assert_eq!(r["result"]["isError"], true);
    let r = call(&mut s, json!({"jsonrpc": "2.0", "id": 6, "method": "resources/read", "params": {"uri": "scriptcraft://script"}}));
    assert!(r["result"]["contents"][0]["text"].as_str().unwrap().contains("MARGO"));
}

#[test]
fn bad_messages() {
    let mut s = Server::new(Box::new(Headless::default()));
    for bad in [
        "{",
        "[]",
        "5",
        r#"{"id":1}"#,
        r#"{"id":[1],"method":"ping"}"#,
        r#"{"id":1,"method":"nope"}"#,
        r#"{"id":1,"method":"tools/call","params":{"name":"execute","arguments":5}}"#,
    ] {
        let r = s.handle_line(bad).unwrap();
        assert!(r.contains("error") || r.contains("isError"), "{bad}: {r}");
    }
}
