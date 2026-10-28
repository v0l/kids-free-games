import argparse
import os
import struct
import time

import numpy as np
import torch
import torch.nn as nn
import torch.nn.functional as F

import kgrl


class Net(nn.Module):
    def __init__(self, channels, side, scalars, actions):
        super().__init__()
        self.channels, self.side, self.scalars, self.actions = channels, side, scalars, actions
        self.trunk = nn.Sequential(
            nn.Conv2d(channels, 32, 3, stride=2, padding=1),
            nn.ReLU(),
            nn.Conv2d(32, 32, 3, stride=2, padding=1),
            nn.ReLU(),
            nn.Conv2d(32, 32, 3, stride=2, padding=1),
            nn.ReLU(),
        )
        flat = 32 * (side // 8) ** 2
        self.actor = nn.Sequential(
            nn.Linear(flat + scalars, 128), nn.ReLU(), nn.Linear(128, actions)
        )
        self.critic = nn.Sequential(
            nn.Linear(flat + scalars, 128), nn.ReLU(), nn.Linear(128, 1)
        )
        nn.init.orthogonal_(self.actor[-1].weight, 0.01)
        nn.init.zeros_(self.actor[-1].bias)

    def features(self, grid, scalars):
        if grid.dtype == torch.uint8:
            grid = grid.float() / 255.0
        x = grid.view(-1, self.channels, self.side, self.side)
        x = self.trunk(x).flatten(1)
        return torch.cat([x, scalars], 1)

    def forward(self, grid, scalars):
        h = self.features(grid, scalars)
        return self.actor(h), self.critic(h).squeeze(-1)


def export(net, path):
    out = bytearray(b"KGB2")
    u = lambda v: out.extend(struct.pack("<I", v))
    for v in (net.side, net.channels, net.scalars, net.actions):
        u(v)

    def floats(t):
        out.extend(t.detach().float().cpu().contiguous().numpy().astype("<f4").tobytes())

    def write(layers):
        u(len(layers))
        for l in layers:
            if isinstance(l, nn.Conv2d):
                u(1)
                for v in (l.in_channels, l.out_channels, l.kernel_size[0], l.stride[0], l.padding[0]):
                    u(v)
                floats(l.weight)
                floats(l.bias)
            elif isinstance(l, nn.Linear):
                u(2)
                u(l.in_features)
                u(l.out_features)
                floats(l.weight)
                floats(l.bias)
            elif isinstance(l, nn.ReLU):
                u(3)
            elif isinstance(l, nn.Tanh):
                u(4)

    write(list(net.trunk))
    write(list(net.actor))
    tmp = path + ".tmp"
    with open(tmp, "wb") as f:
        f.write(bytes(out))
    os.replace(tmp, path)
    return bytes(out)


def parity(net, blob, env):
    g, s = env.observe()
    cpu = Net(net.channels, net.side, net.scalars, net.actions)
    cpu.load_state_dict({k: v.detach().cpu() for k, v in net.state_dict().items()})
    with torch.no_grad():
        want, _ = cpu(torch.tensor(g[:4]), torch.tensor(s[:4]))
    worst = 0.0
    for i in range(4):
        grid = (g[i].astype(np.float32) / 255.0).tolist()
        got = np.array(kgrl.paper_logits(blob, grid, s[i].tolist()))
        ref = want[i].numpy()
        worst = max(worst, float(np.abs(got - ref).max() / (1.0 + np.abs(ref).max())))
    if worst > 1e-4:
        raise SystemExit(f"rust and torch disagree: relative error {worst}")
    return worst


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--envs", type=int, default=96)
    ap.add_argument("--steps", type=int, default=64)
    ap.add_argument("--updates", type=int, default=2000)
    ap.add_argument("--secs", type=float, default=90.0)
    ap.add_argument("--lr", type=float, default=2.5e-4)
    ap.add_argument("--out", default="assets/brains/paper_rl.kgb")
    ap.add_argument("--resume", default="")
    ap.add_argument("--eval-every", type=int, default=50)
    ap.add_argument("--torch-threads", type=int, default=2)
    args = ap.parse_args()

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    torch.set_num_threads(args.torch_threads)
    torch.manual_seed(1)
    env = kgrl.PaperVec(args.envs, 1, 1.0, args.secs)
    channels, side, scalars, actions = kgrl.PaperVec.spec()
    n = env.agents
    net = Net(channels, side, scalars, actions).to(device)
    if args.resume:
        net.load_state_dict(torch.load(args.resume, map_location=device))
    opt = torch.optim.Adam(net.parameters(), lr=args.lr, eps=1e-5)
    params = sum(p.numel() for p in net.trunk.parameters()) + sum(
        p.numel() for p in net.actor.parameters()
    )
    print(f"device {device}, {n} agents, policy {params} weights", flush=True)

    T, gamma, lam = args.steps, 0.995, 0.95
    gsize = channels * side * side
    obs_g = torch.zeros(T, n, gsize, device=device, dtype=torch.uint8)
    obs_s = torch.zeros(T, n, scalars, device=device)
    acts = torch.zeros(T, n, dtype=torch.long, device=device)
    logp = torch.zeros(T, n, device=device)
    vals = torch.zeros(T, n, device=device)
    rews = torch.zeros(T, n, device=device)
    dones = torch.zeros(T, n, device=device)

    g, s = env.observe()
    g = torch.tensor(g, device=device)
    s = torch.tensor(s, device=device)
    start = time.time()
    best = -1e9
    for update in range(1, args.updates + 1):
        frac = 1.0 - (update - 1) / args.updates
        for pg in opt.param_groups:
            pg["lr"] = args.lr * max(frac, 0.1)
        for t in range(T):
            with torch.no_grad():
                logits, v = net(g, s)
                dist = torch.distributions.Categorical(logits=logits)
                a = dist.sample()
            obs_g[t], obs_s[t], acts[t], logp[t], vals[t] = g, s, a, dist.log_prob(a), v
            r, d = env.step(a.cpu().numpy().astype(np.int64))
            rews[t] = torch.tensor(r, device=device)
            dones[t] = torch.tensor(d, device=device, dtype=torch.float32)
            g, s = env.observe()
            g = torch.tensor(g, device=device)
            s = torch.tensor(s, device=device)
        with torch.no_grad():
            _, nv = net(g, s)
            adv = torch.zeros_like(rews)
            last = torch.zeros(n, device=device)
            for t in reversed(range(T)):
                nxt = nv if t == T - 1 else vals[t + 1]
                live = 1.0 - dones[t]
                delta = rews[t] + gamma * nxt * live - vals[t]
                last = delta + gamma * lam * live * last
                adv[t] = last
            ret = adv + vals
        bg, bs = obs_g.view(T * n, -1), obs_s.view(T * n, -1)
        ba, blp, badv, bret = acts.view(-1), logp.view(-1), adv.view(-1), ret.view(-1)
        badv = (badv - badv.mean()) / (badv.std() + 1e-8)
        idx = torch.randperm(T * n, device=device)
        mb = 4096
        for _ in range(3):
            for i in range(0, T * n, mb):
                j = idx[i : i + mb]
                logits, v = net(bg[j], bs[j])
                dist = torch.distributions.Categorical(logits=logits)
                ratio = (dist.log_prob(ba[j]) - blp[j]).exp()
                pg = -torch.min(ratio * badv[j], ratio.clamp(0.8, 1.2) * badv[j]).mean()
                vl = F.mse_loss(v, bret[j])
                ent = dist.entropy().mean()
                loss = pg + 0.5 * vl - 0.01 * ent
                opt.zero_grad()
                loss.backward()
                nn.utils.clip_grad_norm_(net.parameters(), 0.5)
                opt.step()
            idx = torch.randperm(T * n, device=device)
        if update % 10 == 0:
            ep, peaks = env.finished()
            sps = update * T * n / (time.time() - start)
            msg = f"upd {update} sps {sps:.0f} ent {ent.item():.2f} vloss {vl.item():.3f}"
            if ep:
                msg += f" ep_ret {np.mean(ep):.2f} peak {np.mean(peaks) * 100:.2f}%"
            msg += f" circling {env.circling() * 100:.1f}%"
            print(msg, flush=True)
        if update % args.eval_every == 0 or update == args.updates:
            blob = export(net, args.out + ".last")
            err = parity(net, blob, env)
            peak, deaths, kills, inside, circling = kgrl.paper_eval(blob, 16, 90.0, 1.0, "bots")
            print(
                f"EVAL upd {update}: vs old bots peak {peak * 100:.2f}% deaths {deaths:.2f} "
                f"kills {kills:.2f} inside {inside * 100:.0f}% circling {circling * 100:.1f}% "
                f"parity {err:.1e}",
                flush=True,
            )
            torch.save(net.state_dict(), args.out + ".pt")
            score = peak - 0.02 * deaths - 0.05 * circling
            if score > best:
                best = score
                export(net, args.out)
                print(f"  new best saved to {args.out}", flush=True)


if __name__ == "__main__":
    main()
