PRAGMA journal_mode=WAL;

CREATE TABLE IF NOT EXISTS symbols (
    id              INTEGER PRIMARY KEY,
    name            TEXT    NOT NULL,
    kind            TEXT    NOT NULL,
    file            TEXT    NOT NULL,
    line            INTEGER,
    signature       TEXT,
    is_exported     INTEGER NOT NULL DEFAULT 0,
    prov_source     TEXT    NOT NULL,
    prov_confidence REAL    NOT NULL
);

CREATE TABLE IF NOT EXISTS edges (
    id          INTEGER PRIMARY KEY,
    src_file    TEXT,
    src_symbol  TEXT,
    rel         TEXT NOT NULL,
    dst_file    TEXT,
    dst_symbol  TEXT,
    context     TEXT,
    line        INTEGER
);

CREATE TABLE IF NOT EXISTS file_meta (
    file     TEXT PRIMARY KEY,
    language TEXT,
    lines    INTEGER,
    exports  INTEGER,
    imports  INTEGER,
    hash     TEXT
);

CREATE TABLE IF NOT EXISTS boundary_events (
    id              INTEGER PRIMARY KEY,
    fn_name         TEXT NOT NULL,
    file            TEXT NOT NULL,
    line            INTEGER,
    direction       TEXT NOT NULL,
    medium          TEXT NOT NULL,
    key_raw         TEXT NOT NULL,
    key_norm        TEXT NOT NULL,
    local_var       TEXT,
    raw_context     TEXT,
    prov_source     TEXT NOT NULL,
    prov_confidence REAL NOT NULL,
    prov_plugin     TEXT NOT NULL,
    prov_note       TEXT
);

CREATE TABLE IF NOT EXISTS boundary_flows (
    id          INTEGER PRIMARY KEY,
    write_fn    TEXT NOT NULL,
    write_file  TEXT NOT NULL,
    read_fn     TEXT NOT NULL,
    read_file   TEXT NOT NULL,
    medium      TEXT NOT NULL,
    key_norm    TEXT NOT NULL,
    confidence  REAL NOT NULL
);

CREATE TABLE IF NOT EXISTS entry_points (
    id          INTEGER PRIMARY KEY,
    fn_name     TEXT NOT NULL,
    file        TEXT NOT NULL,
    line        INTEGER,
    kind        TEXT NOT NULL,
    framework   TEXT,
    path        TEXT,
    method      TEXT,
    confidence  REAL NOT NULL,
    heuristics  TEXT NOT NULL,
    middleware  TEXT
);

CREATE TABLE IF NOT EXISTS traces (
    id              INTEGER PRIMARY KEY,
    trace_id        TEXT UNIQUE NOT NULL,
    root_fn         TEXT NOT NULL,
    root_file       TEXT NOT NULL,
    service         TEXT NOT NULL,
    text            TEXT NOT NULL,
    compact         TEXT NOT NULL,
    otlp            TEXT,
    span_count      INTEGER,
    fn_names        TEXT,
    media           TEXT,
    value_names     TEXT,
    min_confidence  REAL,
    created_at      TEXT
);

CREATE TABLE IF NOT EXISTS pipeline_checkpoints (
    step        TEXT NOT NULL,
    key         TEXT NOT NULL,
    status      TEXT NOT NULL,
    detail      TEXT,
    updated_at  TEXT,
    PRIMARY KEY (step, key)
);

CREATE INDEX IF NOT EXISTS idx_symbols_name  ON symbols(name);
CREATE INDEX IF NOT EXISTS idx_symbols_file  ON symbols(file);
CREATE INDEX IF NOT EXISTS idx_edges_src     ON edges(src_symbol);
CREATE INDEX IF NOT EXISTS idx_edges_dst     ON edges(dst_symbol);
CREATE INDEX IF NOT EXISTS idx_edges_rel     ON edges(rel);
CREATE INDEX IF NOT EXISTS idx_be_fn         ON boundary_events(fn_name);
CREATE INDEX IF NOT EXISTS idx_be_key        ON boundary_events(key_norm);
CREATE INDEX IF NOT EXISTS idx_be_medium     ON boundary_events(medium);
CREATE INDEX IF NOT EXISTS idx_bf_key        ON boundary_flows(key_norm);
CREATE INDEX IF NOT EXISTS idx_ep_fn         ON entry_points(fn_name);
CREATE INDEX IF NOT EXISTS idx_traces_root   ON traces(root_fn);
