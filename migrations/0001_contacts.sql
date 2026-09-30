-- Central contacts store, shared across all apps.
--
-- Two tables on purpose. `contacts` is the canonical record of a person, keyed
-- by email, with one row no matter how many apps they touch. `contact_events`
-- records each interaction and which app it came from, so adding a second app
-- means inserting events, never duplicating people.

CREATE TABLE IF NOT EXISTS contacts (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  email       TEXT NOT NULL UNIQUE COLLATE NOCASE,
  name        TEXT NOT NULL,
  created_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  updated_at  TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
);

CREATE INDEX IF NOT EXISTS idx_contacts_created_at ON contacts (created_at);

CREATE TABLE IF NOT EXISTS contact_events (
  id          INTEGER PRIMARY KEY AUTOINCREMENT,
  contact_id  INTEGER NOT NULL REFERENCES contacts (id) ON DELETE CASCADE,
  app         TEXT NOT NULL,
  event       TEXT NOT NULL,
  occurred_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
  -- JSON blob: referrer, list-subscription outcome, anything app-specific.
  metadata    TEXT
);

CREATE INDEX IF NOT EXISTS idx_events_contact ON contact_events (contact_id);
CREATE INDEX IF NOT EXISTS idx_events_app_event ON contact_events (app, event);
CREATE INDEX IF NOT EXISTS idx_events_occurred_at ON contact_events (occurred_at);
