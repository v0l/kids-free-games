CREATE TABLE players (
  id TEXT PRIMARY KEY,
  name TEXT NOT NULL DEFAULT '',
  avatar INTEGER NOT NULL DEFAULT 0,
  created_at INTEGER NOT NULL,
  last_seen INTEGER NOT NULL
);

CREATE TABLE tokens (
  hash TEXT PRIMARY KEY,
  player_id TEXT NOT NULL,
  label TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  last_used INTEGER NOT NULL
);
CREATE INDEX tokens_by_player ON tokens (player_id);

CREATE TABLE link_codes (
  code TEXT PRIMARY KEY,
  player_id TEXT NOT NULL,
  expires_at INTEGER NOT NULL
);
CREATE INDEX link_codes_by_player ON link_codes (player_id);

CREATE TABLE saves (
  player_id TEXT NOT NULL,
  game TEXT NOT NULL,
  rev INTEGER NOT NULL,
  data TEXT NOT NULL,
  updated_at INTEGER NOT NULL,
  PRIMARY KEY (player_id, game)
) WITHOUT ROWID;

CREATE TABLE scores (
  game TEXT NOT NULL,
  player_id TEXT NOT NULL,
  score INTEGER NOT NULL,
  achieved_at INTEGER NOT NULL,
  plays INTEGER NOT NULL DEFAULT 1,
  PRIMARY KEY (game, player_id)
) WITHOUT ROWID;
CREATE INDEX scores_high ON scores (game, score DESC, achieved_at ASC, player_id ASC);
CREATE INDEX scores_low ON scores (game, score ASC, achieved_at ASC, player_id ASC);

CREATE TABLE play_sessions (
  player_id TEXT NOT NULL,
  id TEXT NOT NULL,
  game TEXT NOT NULL,
  started_at INTEGER NOT NULL,
  seconds INTEGER NOT NULL,
  PRIMARY KEY (player_id, id)
) WITHOUT ROWID;
CREATE INDEX play_sessions_by_time ON play_sessions (player_id, started_at);
