let ctx = null;
let node = null;
let queue = [];
const sent = new Set();

function post(msg, transfer) {
  if (node) node.port.postMessage(msg, transfer);
  else queue.push([msg, transfer]);
}

export function kg_audio_supported() {
  const C = window.AudioContext || window.webkitAudioContext;
  return !!C && typeof AudioWorkletNode !== "undefined" && !!window.isSecureContext;
}

export function kg_audio_open() {
  if (ctx) return ctx.sampleRate;
  const C = window.AudioContext || window.webkitAudioContext;
  ctx = new C({ latencyHint: "interactive" });
  window.kgAudioStats = { backend: "worklet", started: 0 };
  ctx.audioWorklet
    .addModule(new URL("kg-mixer.js", document.baseURI))
    .then(() => {
      node = new AudioWorkletNode(ctx, "kg-mixer", {
        numberOfInputs: 0,
        numberOfOutputs: 1,
        outputChannelCount: [2],
      });
      node.port.onmessage = () => window.kgAudioStats.started++;
      node.connect(ctx.destination);
      for (const [m, t] of queue) node.port.postMessage(m, t);
      queue = [];
    })
    .catch((e) => console.warn("audio worklet failed", e));
  return ctx.sampleRate;
}

export function kg_audio_play(id, rate, data, gain, music) {
  if (!sent.has(id)) {
    sent.add(id);
    const copy = new Float32Array(data);
    post({ t: "clip", id, rate, data: copy }, [copy.buffer]);
  }
  post({ t: "start", id, gain, music }, []);
}

export function kg_audio_play_once(rate, data, gain) {
  const copy = new Float32Array(data);
  post({ t: "start", rate, data: copy, gain, music: false }, [copy.buffer]);
}

export function kg_audio_stop_music() {
  post({ t: "stop" }, []);
}

export function kg_audio_music_gain(gain) {
  post({ t: "gain", gain }, []);
}
