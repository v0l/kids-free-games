#!/usr/bin/env python3
import argparse
import json
import os
import subprocess
import sys
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VOICE = "Harper"


def speak(text, voice):
    url = os.environ.get("AGENTBENCH_URL", "http://127.0.0.1:7420") + "/v1/audio/speech"
    token = os.environ.get("AGENTBENCH_TOKEN")
    if not token:
        sys.exit("AGENTBENCH_TOKEN is not set")
    req = urllib.request.Request(
        url,
        data=json.dumps({"input": text, "voice": voice}).encode(),
        headers={"Authorization": f"Bearer {token}", "Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=120) as r:
        return r.read()


def to_ogg(wav, out):
    with tempfile.NamedTemporaryFile(suffix=".wav") as f:
        f.write(wav)
        f.flush()
        subprocess.run(
            ["ffmpeg", "-v", "error", "-y", "-i", f.name, "-ac", "1",
             "-c:a", "libvorbis", "-q:a", "4", str(out)],
            check=True,
        )


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("lines", nargs="*", help="name=text pairs, e.g. say_great.0='Great job!'")
    ap.add_argument("--file", help="a file of name | text lines")
    ap.add_argument("--voice", default=VOICE)
    ap.add_argument("--out", default=str(ROOT / "sound"))
    ap.add_argument("--proof", help="also write every clip into one WAV with gaps, to check")
    a = ap.parse_args()
    pairs = [l.split("=", 1) for l in a.lines]
    if a.file:
        for l in Path(a.file).read_text().splitlines():
            if l.strip() and not l.startswith("#"):
                pairs.append([p.strip() for p in l.split("|", 1)])
    if not pairs:
        ap.error("nothing to say")
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    proof = []
    for name, text in pairs:
        wav = speak(text, a.voice)
        to_ogg(wav, out / f"{name}.ogg")
        proof.append(out / f"{name}.ogg")
        print(f"{name}.ogg  {text}")
    if a.proof:
        with tempfile.TemporaryDirectory() as d:
            gap = Path(d) / "gap.wav"
            subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "lavfi", "-i",
                            "anullsrc=r=24000:cl=mono", "-t", "0.6", str(gap)], check=True)
            parts = []
            for i, p in enumerate(proof):
                w = Path(d) / f"{i}.wav"
                subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(p),
                                "-ar", "24000", "-ac", "1", str(w)], check=True)
                parts += [w, gap]
            listing = Path(d) / "list.txt"
            listing.write_text("".join(f"file '{p}'\n" for p in parts))
            subprocess.run(["ffmpeg", "-v", "error", "-y", "-f", "concat", "-safe", "0",
                            "-i", str(listing), a.proof], check=True)
        print(f"proof: {a.proof}")


if __name__ == "__main__":
    main()
