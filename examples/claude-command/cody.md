---
description: Use cody (codebase semantic indexer) to analyze a project. Run queries against the index.
argument-hint: "<question or task>"
---

You have access to **cody** — a static codebase semantic indexer for multi-language, multi-service repos.

## Philosophy

cody builds a SQLite index from source code using tree-sitter (no LLM during indexing). The index captures:
- **symbols**: functions, classes, methods across JS/TS/Python/Ruby/Rust
- **edges**: call graph (who calls what), data flow, imports
- **boundary_events**: I/O operations (redis, kafka, sql, http_header, grpc)
- **entry_points**: exposed endpoints with route paths, HTTP methods, middleware
- **boundary_flows**: write→read pairs stitched across services
- **traces**: human-readable execution paths from entry points to I/O

Use the index first to navigate, then read targeted source files for detail.

## Setup

```bash
# Auto-detect binary and most recent .db file in current directory
CODY=$(which cody 2>/dev/null || echo "./target/release/cody")
DB=$(ls -t *.db 2>/dev/null | head -1)

# If no .db found, build one first:
# cody index . --db my-project.db [--lsp]
```

## CLI reference — exact syntax

```
cody query --db <path> <COMMAND> [ARGS]
```

> **`--db` must come BEFORE the subcommand.**

| Command | Args | Description |
|---|---|---|
| `stats` | | Row counts for all tables |
| `lookup` | `<name>` | Exact match, falls back to fuzzy substring |
| `callers` | `<symbol>` | Who calls this function |
| `callees` | `<symbol>` | What this function calls |
| `deps` | `<file>` | File-level import dependencies |
| `path` | `<from> <to>` | Shortest call path between two functions |
| `boundaries` | `<fn>` | I/O operations touched by a function |
| `medium` | `<medium>` | All events for redis/kafka/sql/http_header/grpc |
| `cross` | `<svc_a> <svc_b>` | Keys written by A and read by B |
| `traces` | `<fn>` | Full execution trace from an entry point |
| `topology` | | Service dependency graph |

```bash
# CORRECT
$CODY query --db $DB lookup login
$CODY query --db $DB boundaries lp_auth_middleware
$CODY query --db $DB traces login

# WRONG — exit code 2: --db after subcommand
$CODY query lookup --db $DB login

# WRONG — exit code 1: never quote --db and the path together as one variable
DB2="--db /path/to/db"
$CODY query $DB2 lookup foo   # shell passes this as ONE argument, not two
```

## Example session

```bash
CODY=$(which cody 2>/dev/null || echo "./target/release/cody")
DB=$(ls -t *.db 2>/dev/null | head -1)

# Overview
$CODY query --db $DB stats
$CODY query --db $DB topology

# Find a symbol (fuzzy if no exact match)
$CODY query --db $DB lookup AuthMiddleware

# What I/O does a function touch?
$CODY query --db $DB boundaries process_order

# Full trace from an entry point
$CODY query --db $DB traces create_order

# All redis operations
$CODY query --db $DB medium redis

# All routes with middleware
sqlite3 $DB 'SELECT fn_name, path, method, middleware FROM entry_points WHERE path IS NOT NULL ORDER BY path;'
```

## Schema reference

```
symbols(id, name, kind, file, line, signature, is_exported, prov_source, prov_confidence)
edges(id, src_file, src_symbol, rel, dst_file, dst_symbol, context, line)
file_meta(file, language, lines, exports, imports, hash)
boundary_events(id, fn_name, file, line, direction, medium, key_raw, key_norm,
                local_var, raw_context, prov_source, prov_confidence, prov_plugin, prov_note)
boundary_flows(id, write_fn, write_file, read_fn, read_file, medium, key_norm, confidence)
entry_points(id, fn_name, file, line, kind, framework, path, method,
             confidence, heuristics, middleware)
traces(id, trace_id, root_fn, root_file, service, text, compact, otlp,
       span_count, fn_names, media, value_names, min_confidence, created_at)
```

## Useful sqlite3 queries

```bash
# Routes with auth middleware
sqlite3 $DB 'SELECT fn_name, path, method, middleware FROM entry_points WHERE path IS NOT NULL ORDER BY path;'

# Find symbols by name pattern (column is "name", not "fn_name")
sqlite3 $DB 'SELECT name, kind, file, line FROM symbols WHERE name LIKE "%auth%" ORDER BY file;'

# High-confidence boundary events
sqlite3 $DB 'SELECT fn_name, medium, key_raw, prov_confidence FROM boundary_events WHERE prov_confidence > 0.8 ORDER BY prov_confidence DESC;'

# gRPC flows between services
sqlite3 $DB 'SELECT medium, key_raw, fn_name, file FROM boundary_events WHERE medium = "grpc";'
```

## Task

$ARGUMENTS

If no task is given, show this help and ask what to explore.

Use the cody CLI and sqlite3 commands above. Do NOT read source files first — use the index to orient, then read only the specific files needed for detail.
