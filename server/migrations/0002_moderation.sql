CREATE TABLE reports (
  reporter_id TEXT NOT NULL,
  reported_id TEXT NOT NULL,
  game TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  PRIMARY KEY (reporter_id, reported_id)
) WITHOUT ROWID;
CREATE INDEX reports_by_reported ON reports (reported_id);
