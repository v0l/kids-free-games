const MAX_VOICES = 24;
const RAMP_PER_SEC = 6;

class KgMixer extends AudioWorkletProcessor {
  constructor() {
    super();
    this.clips = new Map();
    this.voices = [];
    this.music = 0;
    this.target = 0;
    this.ramp = RAMP_PER_SEC / sampleRate;
    this.port.onmessage = (e) => this.command(e.data);
  }

  command(c) {
    if (c.t === "clip") {
      this.clips.set(c.id, { rate: c.rate, data: c.data });
    } else if (c.t === "start") {
      const clip = c.data ? { rate: c.rate, data: c.data } : this.clips.get(c.id);
      if (!clip) return;
      if (c.music) {
        this.voices = this.voices.filter((v) => !v.music);
        this.music = 0;
      } else if (this.voices.filter((v) => !v.music).length >= MAX_VOICES) {
        const i = this.voices.findIndex((v) => !v.music);
        if (i >= 0) this.voices.splice(i, 1);
      }
      this.voices.push({ data: clip.data, pos: 0, step: clip.rate / sampleRate, gain: c.gain, music: c.music });
      if (!c.music) this.port.postMessage({ started: currentTime });
    } else if (c.t === "stop") {
      this.voices = this.voices.filter((v) => !v.music);
    } else if (c.t === "gain") {
      this.target = c.gain;
    }
  }

  process(_inputs, outputs) {
    const out = outputs[0];
    const left = out[0];
    const n = left.length;
    left.fill(0);
    const target = this.target;
    const ramp = this.ramp;
    let music = this.music;
    let hasMusic = false;
    for (const v of this.voices) {
      const data = v.data;
      const len = data.length;
      if (len === 0) continue;
      let m = music;
      for (let f = 0; f < n; f++) {
        if (v.pos >= len) {
          if (v.music) v.pos -= len;
          else break;
        }
        const i = v.pos | 0;
        const t = v.pos - i;
        const a = data[i];
        const b = i + 1 < len ? data[i + 1] : v.music ? data[0] : 0;
        let g = v.gain;
        if (v.music) {
          const d = target - m;
          m += d > ramp ? ramp : d < -ramp ? -ramp : d;
          g = m;
        }
        left[f] += (a + (b - a) * t) * g;
        v.pos += v.step;
      }
      if (v.music) {
        music = m;
        hasMusic = true;
      }
    }
    this.music = hasMusic ? music : target;
    this.voices = this.voices.filter((v) => v.music || v.pos < v.data.length);
    for (let f = 0; f < n; f++) {
      const s = left[f];
      left[f] = s > 1 ? 1 : s < -1 ? -1 : s;
    }
    for (let c = 1; c < out.length; c++) out[c].set(left);
    return true;
  }
}

registerProcessor("kg-mixer", KgMixer);
