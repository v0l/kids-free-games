const { chromium } = require("playwright-core");
const probe = `
class Probe extends AudioWorkletProcessor {
  constructor() { super(); this.quiet = true; this.last = -1; this.skips = 0; this.loud = 0; }
  process(inputs) {
    if (this.last >= 0 && currentFrame - this.last !== 128) this.skips++;
    this.last = currentFrame;
    let peak = 0;
    for (const ch of inputs[0] || []) for (let i = 0; i < ch.length; i++) { const a = Math.abs(ch[i]); if (a > peak) peak = a; }
    if (peak > 0.004) { if (this.quiet) this.port.postMessage({ onset: currentTime, skips: this.skips }); this.quiet = false; this.loud = 0; }
    else if (++this.loud > 40) this.quiet = true;
    return true;
  }
}
registerProcessor("kg-probe", Probe);`;
(async () => {
  const b = await chromium.launch({ executablePath: process.env.HOME + "/.cache/ms-playwright/chromium-1243/chrome-linux64/chrome", args: ["--use-gl=angle","--use-angle=swiftshader","--enable-unsafe-swiftshader","--autoplay-policy=no-user-gesture-required"] });
  const p = await b.newPage({ viewport: { width: 1024, height: 768 } });
  await p.addInitScript((src) => {
    window.__onsets = []; window.__keys = []; window.__ctxs = [];
    const url = URL.createObjectURL(new Blob([src], { type: "text/javascript" }));
    const probes = new WeakMap();
    const getProbe = (ctx) => {
      if (!probes.has(ctx)) {
        window.__ctxs.push(ctx);
        probes.set(ctx, ctx.audioWorklet.addModule(url).then(() => {
          const n = new AudioWorkletNode(ctx, "kg-probe", { numberOfOutputs: 1 });
          n.port.onmessage = (e) => window.__onsets.push(e.data);
          n.connect(ctx.destination);
          return n;
        }));
      }
      return probes.get(ctx);
    };
    const conn = AudioNode.prototype.connect;
    AudioNode.prototype.connect = function (dest, ...rest) {
      const r = conn.call(this, dest, ...rest);
      if (dest instanceof AudioDestinationNode && !(this instanceof AudioWorkletNode && this.__probe)) {
        const src = this;
        getProbe(this.context).then((n) => { n.__probe = true; if (src !== n) conn.call(src, n); });
      }
      return r;
    };
    window.addEventListener("keydown", () => {
      const c = window.__ctxs[0];
      if (!c) return;
      const ts = c.getOutputTimestamp();
      const ctxNow = ts.contextTime + (performance.now() - ts.performanceTime) / 1000;
      window.__keys.push(ctxNow);
    }, true);
  }, probe);
  const cdp = await p.context().newCDPSession(p);
  await cdp.send("Emulation.setCPUThrottlingRate", { rate: Number(process.env.THROTTLE || 1) });
  await p.goto(process.argv[2]);
  await p.waitForTimeout(8000);
  await p.mouse.click(512, 384);
  await p.keyboard.type("Mia");
  await p.keyboard.press("Enter");
  await p.waitForTimeout(4000);
  await p.mouse.click(45, 38);
  await p.waitForTimeout(3000);
  await p.evaluate(() => { window.__keys = []; window.__onsets = []; });
  for (let i = 0; i < 16; i++) {
    await p.keyboard.press(i % 8 < 4 ? "ArrowRight" : "ArrowLeft");
    await p.waitForTimeout(1200);
  }
  const r = await p.evaluate(() => {
    const c = window.__ctxs[0];
    const lat = [];
    for (const k of window.__keys) {
      const o = window.__onsets.find((o) => o.onset >= k && o.onset < k + 1.1);
      if (o) lat.push(Math.round((o.onset - k + c.baseLatency + (c.outputLatency || 0)) * 1000));
    }
    lat.sort((a, b) => a - b);
    const last = window.__onsets[window.__onsets.length - 1];
    return { keys: window.__keys.length, heard: lat.length, latencyMs: lat, median: lat[lat.length >> 1], renderSkips: last ? last.skips : null, baseLatencyMs: Math.round(c.baseLatency * 1000) };
  });
  console.log(JSON.stringify(r));
  await b.close();
})();
