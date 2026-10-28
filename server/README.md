# Kids Games API

Cloudflare Worker in Rust (`worker` crate, D1 database). Syncs saves, leaderboards and play time between a child's devices. Multi-player will be added as a Durable Object (one room per match) behind `/v1/rooms`.

## Run and deploy

```sh
cd server
wrangler d1 create kids-games
wrangler d1 migrations apply kids-games --local
wrangler dev --local --port 8799
./tests/e2e.sh
cargo test
wrangler d1 migrations apply kids-games --remote
wrangler deploy
```

Put the id from `d1 create` in `wrangler.toml`. `strip = "debuginfo"` in `Cargo.toml` is deliberate: full strip breaks wasm-bindgen.

## Accounts

No passwords. `POST /v1/players` makes a player and returns a bearer token, which the app keeps in its save file. Tokens are stored hashed. To use the same player on another device, the first device calls `POST /v1/link` for a code like `K7QM-2XPD` (valid 10 minutes, one use) and the second device sends it to `POST /v1/link/redeem`. Names follow the app rules (letters, up to 12). Only name, avatar and scores are visible to others.

## Endpoints

All bodies are JSON. Errors are `{"error": "code", "message": "..."}`. Send `Authorization: Bearer <token>` except where marked public.

| Method and path | Auth | Purpose |
|---|---|---|
| `GET /v1/health` | public | liveness |
| `GET /v1/games` | public | game ids, which direction is better, save slots, limits |
| `POST /v1/players` `{name, avatar, device?}` | public | register, returns `{player, token}` |
| `POST /v1/link/redeem` `{code, device?}` | public | trade a link code for a token |
| `GET /v1/me` | yes | profile and total play time |
| `PATCH /v1/me` `{name?, avatar?}` | yes | rename |
| `DELETE /v1/me` | yes | erase the player and everything of theirs |
| `POST /v1/link` | yes | make a link code for another device |
| `GET /v1/saves` | yes | list slots with rev and size |
| `GET /v1/saves/{slot}` | yes | `{rev, data, updated_at}` |
| `PUT /v1/saves/{slot}` `{data, base_rev}` | yes | compare-and-set write |
| `POST /v1/scores/{game}` `{score}` | yes | submit, returns `{best, improved, rank}` |
| `GET /v1/scores/{game}?limit=10` | yes | top entries plus your own rank |
| `POST /v1/playtime` `{sessions: [{id, game, started_at, seconds}]}` | yes | upload play sessions |
| `GET /v1/playtime?days=14` | yes | totals by game and by UTC day |

### Saves

A slot is `profile` or a game id. `data` is an opaque string up to 64 KiB (the app's `key=value` text). Write with the `rev` you last read (`0` to create). A stale `base_rev` returns `409 rev_mismatch` with the current `rev`: fetch, merge (take the higher level, more stars) and write again.

### Scores

One row per player and game (best score, plays). `memory` counts moves so lower wins; every other game is higher-wins (`GET /v1/games`). Ties go to whoever got there first. The app submits on game over exactly as it does for the local board.

### Play time

The app generates a random `id` (8 to 64 of `A-Za-z0-9_-`) when a game starts and uploads `{id, game, started_at, seconds}` when it ends or is paused, and again later while it keeps running. Uploads are idempotent: the longest `seconds` for an id wins, so retries and resends after going offline are safe. Sessions are capped at 4 hours and cannot end in the future.

## Admin dashboard

`GET /admin` serves a single-page dashboard (overview, players, saves, scores, per-game boards) that manages data through `/v1/admin/*`. Those routes take one shared key as the bearer token, set with `wrangler secret put ADMIN_KEY` (locally, `ADMIN_KEY=...` in `.dev.vars`). With no key set they return 404. The page keeps the key in the browser's localStorage after sign-in.

| Method and path | Purpose |
|---|---|
| `GET /v1/admin/stats` | totals, per-game play time, 14-day chart |
| `GET /v1/admin/players?q=&sort=&limit=&offset=` | search and page players |
| `POST /v1/admin/players` `{name, avatar}` | create, returns a link code |
| `GET/PATCH/DELETE /v1/admin/players/{id}` | detail, rename, erase |
| `POST /v1/admin/players/{id}/link` | new link code |
| `GET/PUT/DELETE /v1/admin/players/{id}/saves/{slot}` | read, overwrite (no rev check), delete |
| `GET /v1/admin/scores/{game}` | full board |
| `PUT/DELETE /v1/admin/scores/{game}/{id}` | set or remove a score |

## Limits

Rate limits (Workers rate limiting binding): 120 requests per minute per player, 10 registrations and 10 link redemptions per minute per IP. Request bodies are capped at 128 KiB.

## Adding a game

Add it to `GAMES` in `src/games.rs`. A test reads `../src/games/*.rs` and fails if the app has a game the server does not know.
