#!/usr/bin/env node
const { spawn } = require("child_process");
const fs = require("fs");
const os = require("os");
const path = require("path");

const [, , url, out, w = "1024", h = "768", waitMs = "6000"] = process.argv;
if (!url || !out) {
  console.error("usage: shot.js URL OUT.png [W H WAIT_MS]");
  process.exit(2);
}

const chrome = process.env.CHROME || "google-chrome";
const profile = fs.mkdtempSync(path.join(os.tmpdir(), "kgshot-"));
const proc = spawn(chrome, [
  "--headless=new",
  "--no-sandbox",
  "--use-angle=swiftshader",
  "--enable-unsafe-swiftshader",
  "--hide-scrollbars",
  "--remote-debugging-port=0",
  `--user-data-dir=${profile}`,
  `--window-size=${w},${h}`,
  "about:blank",
]);

let done = false;
function finish(code) {
  if (done) return;
  done = true;
  proc.kill("SIGKILL");
  fs.rmSync(profile, { recursive: true, force: true });
  process.exit(code);
}
setTimeout(() => {
  console.error("timed out");
  finish(1);
}, Number(waitMs) + 60000);

let buf = "";
proc.stderr.on("data", async (d) => {
  buf += d.toString();
  const m = buf.match(/DevTools listening on (ws:\/\/[^\s]+)/);
  if (!m || proc.started) return;
  proc.started = true;
  const port = new URL(m[1]).port;
  const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
  const page = list.find((t) => t.type === "page");
  run(page.webSocketDebuggerUrl).catch((e) => {
    console.error(e);
    finish(1);
  });
});

async function run(wsUrl) {
  const ws = new WebSocket(wsUrl);
  await new Promise((r) => ws.addEventListener("open", r, { once: true }));
  let id = 0;
  const waiting = new Map();
  ws.addEventListener("message", (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.id && waiting.has(msg.id)) {
      waiting.get(msg.id)(msg);
      waiting.delete(msg.id);
    } else if (msg.method === "Runtime.consoleAPICalled") {
      const text = msg.params.args.map((a) => a.value ?? a.description ?? "").join(" ");
      if (process.env.SHOW_CONSOLE || /panic|error/i.test(text)) console.log(`[console] ${text}`);
    } else if (msg.method === "Runtime.exceptionThrown") {
      console.log(`[exception] ${msg.params.exceptionDetails.exception?.description || msg.params.exceptionDetails.text}`);
    }
  });
  const send = (method, params = {}) =>
    new Promise((r) => {
      id += 1;
      waiting.set(id, r);
      ws.send(JSON.stringify({ id, method, params }));
    });
  await send("Runtime.enable");
  await send("Page.enable");
  await send("Emulation.setDeviceMetricsOverride", {
    width: Number(w),
    height: Number(h),
    deviceScaleFactor: Number(process.env.DPR || 1),
    mobile: false,
  });
  await send("Page.navigate", { url });
  await new Promise((r) => setTimeout(r, Number(waitMs)));
  const shot = await send("Page.captureScreenshot", { format: "png" });
  fs.writeFileSync(out, Buffer.from(shot.result.data, "base64"));
  finish(0);
}
