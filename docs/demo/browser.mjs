import fs from "fs";
const dir = process.argv[2];
const list = await (await fetch("http://127.0.0.1:9333/json/list")).json();
const ws = new WebSocket(list.find(t => t.type === "page").webSocketDebuggerUrl);
let id = 0; const pending = new Map();
ws.onmessage = (m) => { const d = JSON.parse(m.data); if (d.id && pending.has(d.id)) { pending.get(d.id)(d); pending.delete(d.id); } };
await new Promise(r => ws.onopen = r);
const send = (method, params = {}) => new Promise(r => { const i = ++id; pending.set(i, r); ws.send(JSON.stringify({ id: i, method, params })); });
let n = 0;
const sleep = (ms) => new Promise(r => setTimeout(r, ms));
const shot = async () => {
  for (let t = 0; t < 20; t++) {
    const s = await send("Page.captureScreenshot", { format: "png" });
    if (s.result && s.result.data) { fs.writeFileSync(`${dir}/w${String(n++).padStart(4, "0")}.png`, Buffer.from(s.result.data, "base64")); return; }
    await sleep(150);
  }
};
const shots = async (k) => { for (let i = 0; i < k; i++) await shot(); };
await send("Emulation.setDeviceMetricsOverride", { width: 1280, height: 800, deviceScaleFactor: 1.5, mobile: false });
await send("Page.navigate", { url: "http://127.0.0.1:8765/index.html" });
await sleep(400);
await shots(25);
const key = async (key, code, text, modifiers = 0, vk = 0) => {
  await send("Input.dispatchKeyEvent", { type: "keyDown", key, code, text, unmodifiedText: text, modifiers, windowsVirtualKeyCode: vk });
  await send("Input.dispatchKeyEvent", { type: "keyUp", key, code, modifiers, windowsVirtualKeyCode: vk });
  await shot();
};
// Click into the page, jump to the end, start a new scene and type.
await send("Input.dispatchMouseEvent", { type: "mousePressed", x: 760, y: 420, button: "left", clickCount: 1 });
await send("Input.dispatchMouseEvent", { type: "mouseReleased", x: 760, y: 420, button: "left", clickCount: 1 });
await shots(3);
await key("ArrowDown", "ArrowDown", undefined, 4, 40);
await shots(4);
await key("Enter", "Enter", "\r", 0, 13);
for (const ch of "ext. browser tab - night") await key(ch, "", ch);
await key("Enter", "Enter", "\r", 0, 13);
for (const ch of "ScriptCraft runs here too, compiled to WebAssembly.") await key(ch, "", ch);
await shots(8);
await key("b", "KeyB", undefined, 4, 66);
await shots(10);
console.log("frames", n);
ws.close();
