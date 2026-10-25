#!/usr/bin/env python3
import json
import os
import random
import sys
import time
import urllib.request

GRAVITY, FLAP, BIRD_R, PIPE_W = 1500.0, -520.0, 22.0, 96.0


def flappy_case(rng):
    gap = rng.uniform(240, 330)
    gap_y = rng.uniform(250, 500)
    y = gap_y + rng.uniform(-gap * 0.9, gap * 0.9)
    vy = rng.uniform(-500, 500)
    dist = rng.uniform(0, 260)
    speed = 200.0

    def outcome(flap):
        yy, v = y, (FLAP if flap else vy)
        t, dt = 0.0, 1 / 120
        top, bot = gap_y - gap / 2, gap_y + gap / 2
        while t < (dist + PIPE_W + BIRD_R) / speed:
            v += GRAVITY * dt
            yy += v * dt
            t += dt
            x = dist - speed * t
            if yy + BIRD_R > 678:
                return "hits the ground"
            if -PIPE_W - BIRD_R < x < BIRD_R:
                if yy - BIRD_R + 5 < top:
                    return "hits the top pipe"
                if yy + BIRD_R - 5 > bot:
                    return "hits the bottom pipe"
        off = (yy - gap_y) / (gap / 2)
        return "passes the gap " + ("near the top" if off < -0.5 else "near the bottom" if off > 0.5 else "through the middle")

    f, w = outcome(True), outcome(False)
    safe = {a: o.startswith("passes") for a, o in (("flap", f), ("wait", w))}
    if safe["flap"] == safe["wait"]:
        if not safe["flap"]:
            return None
        right = "flap" if "middle" in f and "middle" not in w else "wait" if "middle" in w and "middle" not in f else None
        if right is None:
            return None
    else:
        right = "flap" if safe["flap"] else "wait"
    raw = {"bird_y": round(y), "bird_vy": round(vy), "gap_top": round(gap_y - gap / 2), "gap_bottom": round(gap_y + gap / 2), "pipe_ahead_px": round(dist)}
    described = {"if_flap_now": f, "if_wait": w}
    q = {"type": "choice", "instructions": "Pick the action whose outcome keeps the bird alive. Passing the gap is good; hitting a pipe or the ground ends the game. If both pass, prefer through the middle.", "criteria": {"flap": "flap now", "wait": "do not flap"}}
    return {"game": "flappy", "raw": raw, "described": described, "question": q, "right": right}


def crossy_case(rng):
    cell = 64
    px = rng.randrange(2, 12) * cell
    cars = [rng.uniform(-200, 1000) for _ in range(rng.randint(1, 3))]
    d = rng.choice([-1, 1])
    speed = rng.uniform(80, 260)
    hop = 0.15
    car_w = 110

    def hit_if_hop():
        for t in [hop + i * 0.05 for i in range(8)]:
            for c in cars:
                cx = c + d * speed * t
                if cx - car_w / 2 < px + cell / 2 and cx + car_w / 2 > px - cell / 2:
                    return True
        return False

    danger = hit_if_hop()
    right = "wait" if danger else "up"
    near = min(cars, key=lambda c: abs(c - px))
    gap = near - px
    toward = (gap > 0 and d < 0) or (gap < 0 and d > 0)
    secs = abs(gap) / speed
    raw = {"chicken_x": px, "next_row": "road", "cars_x": [round(c) for c in cars], "direction": "right" if d > 0 else "left", "car_speed": round(speed)}
    described = {"next_row": "road", "nearest_car": f"{abs(round(gap))} px {'right' if gap > 0 else 'left'} of the chicken, driving {'towards' if toward else 'away from'} it", "if_hop_now": "a car hits the chicken" if danger else "the chicken lands safely", "if_wait": "the chicken stays safe on the grass"}
    q = {"type": "choice", "instructions": "The goal is to cross as many rows as possible without getting hit. Hop whenever the hop lands safely; wait only if hopping now would get the chicken hit.", "criteria": {"up": "hop forward one row", "wait": "stay still"}}
    return {"game": "crossy", "raw": raw, "described": described, "question": q, "right": right}


def cases(n, seed=7):
    rng = random.Random(seed)
    want = n // 4
    buckets = {}
    while any(len(buckets.get(k, [])) < want for k in [("flappy", "flap"), ("flappy", "wait"), ("crossy", "up"), ("crossy", "wait")]):
        for make in (flappy_case, crossy_case):
            c = make(rng)
            if c:
                b = buckets.setdefault((c["game"], c["right"]), [])
                if len(b) < want:
                    b.append(c)
    return [c for b in buckets.values() for c in b]


def call(url, model, state, q, key=None):
    if url.endswith("/v1/decide"):
        body = {"kind": q["type"], "state": state, "question": q["instructions"], "options": [f"{k}: {v}" for k, v in q["criteria"].items()]}
        req = urllib.request.Request(url, json.dumps(body).encode(), {"Content-Type": "application/json"})
        t = time.time()
        d = json.loads(urllib.request.urlopen(req, timeout=60).read())
        probs = d.get("probabilities") or d.get("probs") or {}
        if isinstance(probs, list):
            probs = dict(zip(q["criteria"], probs))
        pick = max(probs, key=probs.get) if probs else d.get("choice", "")
        return pick.split(":")[0], time.time() - t, 0.0
    body = {"state": state, "questions": {"act": q}}
    if model:
        body["model"] = model
    req = urllib.request.Request(url, json.dumps(body).encode(), {"Content-Type": "application/json", **({"Authorization": f"Bearer {key}"} if key else {})})
    t = time.time()
    d = json.loads(urllib.request.urlopen(req, timeout=60).read())
    return d["answers"]["act"]["choice"], time.time() - t, d.get("usage", {}).get("cost", 0)


def main():
    name, url, model = sys.argv[1], sys.argv[2], (sys.argv[3] if len(sys.argv) > 3 else "")
    n = int(os.environ.get("N", "40"))
    key = os.environ.get("KEY")
    res = {}
    for c in cases(n):
        for view in ("raw", "described"):
            got, dt, cost = call(url, model, c[view], c["question"], key)
            r = res.setdefault((c["game"], view), [0, 0, 0.0, 0.0])
            r[0] += got == c["right"]
            r[1] += 1
            r[2] += dt
            r[3] += cost
    for (g, v), (ok, tot, dt, cost) in sorted(res.items()):
        print(f"{name:10s} {g:7s} {v:10s} {ok}/{tot} right ({100*ok/tot:3.0f}%)  {1000*dt/tot:5.0f} ms/call  ${cost:.4f}")


if __name__ == "__main__":
    main()
