# cody

A static codebase semantic indexer for multi-language, multi-service repositories.

cody extracts a structured index from source code using tree-sitter — no LLM during indexing. The index captures symbols, call graphs, I/O boundaries, HTTP routes, middleware, and gRPC flows. LLM is only used at query time (optional `search` command).

**Best used alongside code reading, not as a replacement.** cody orients you quickly — find the right files, understand service dependencies, trace execution paths — then you read the targeted files for detail.

---

## Supported languages

| Language | Symbols | Call graph | Boundaries | Routes | Middleware |
|---|---|---|---|---|---|
| Rust | ✓ | ✓ | ✓ | Axum, Actix, Rocket | Extractors |
| TypeScript | ✓ | ✓ | ✓ | Express, Fastify, NestJS | Guards, use() |
| JavaScript | ✓ | ✓ | ✓ | Express, Fastify | use() |
| Python | ✓ | ✓ | ✓ | FastAPI, Flask, Django | Depends() |
| Ruby | ✓ | ✓ | ✓ | Rails | before_action |

**Boundary mediums detected:** `redis`, `sql`, `kafka`, `http_header`, `grpc`

---

## Installation

**From source (recommended):**
```bash
git clone https://github.com/s-benchahed/cody
cd cody
cargo build --release
# binary at ./target/release/cody
```

**Install to PATH:**
```bash
cargo install --path cody
```

Requires Rust 1.75+.

---

## Quick start

```bash
# 1. Index your project
cody index ./my-project --db my-project.db

# 2. See what's in it
cody query --db my-project.db stats

# 3. Explore
cody query --db my-project.db topology
cody query --db my-project.db traces <entry-point-fn>
cody query --db my-project.db medium redis
```

---

## CLI reference

### `cody index`

```
cody index <dir> [OPTIONS]

Options:
  --db <path>            SQLite output path [default: index.db]
  --depth <n>            Max call graph depth [default: 6]
  --lsp                  Enable LSP type verification (removes false positives)
  --min-confidence <f>   Minimum confidence threshold [default: 0.5]
  --all-entrypoints      Include low-confidence entry points in traces
  --skip-embed           Skip embedding step
```

Re-running on the same directory is incremental — only changed files are reprocessed.

### `cody query --db <path> <COMMAND>`

> **Note:** `--db` must come before the subcommand, not after.

| Command | Args | Description |
|---|---|---|
| `stats` | | Row counts for all tables |
| `lookup` | `<name>` | Find symbol definition. Falls back to fuzzy substring match. |
| `callers` | `<symbol>` | What functions call this symbol |
| `callees` | `<symbol>` | What this symbol calls |
| `deps` | `<file>` | File-level import dependencies |
| `path` | `<from> <to>` | Shortest call path between two functions |
| `boundaries` | `<fn>` | I/O operations (redis/sql/kafka/grpc) touched by a function |
| `medium` | `<medium>` | All boundary events for a given medium across the codebase |
| `cross` | `<svc_a> <svc_b>` | Keys written by service A and read by service B |
| `traces` | `<fn>` | Full execution trace from an entry point |
| `topology` | | Service dependency graph derived from boundary flows |

### `cody embed`

```
cody embed --db <path> --api-key $OPENAI_API_KEY [--model text-embedding-3-small]
```

Embeds all traces using the OpenAI embeddings API. Required before `search`.

### `cody search`

```
cody search --db <path> --api-key $ANTHROPIC_API_KEY "<question>"
```

Natural language search over indexed traces using Claude.

---

## LSP enrichment (`--lsp`)

Without LSP, boundary detection uses AST patterns and regex with ~0.7–0.9 confidence. With `--lsp`, cody spawns language servers to hover-query the receiver type of each boundary call, verifying or rejecting the detection.

**Effect:** Removes false positives (e.g. `.get()` on a SQL row misidentified as Redis).

**Required language servers (install before running `--lsp`):**

```bash
# Rust
rustup component add rust-analyzer

# TypeScript / JavaScript
npm install -g typescript-language-server typescript

# Python
pip install pyright
```

LSP indexing takes 30–120s depending on project size (dominated by workspace load time).

---

## Claude Code integration

See [`docs/claude-code.md`](docs/claude-code.md) for a one-command setup that adds a `/cody` slash command to Claude Code.

---

## How it works

```
[1] Walk         Find all source files matching supported extensions
[2] Hash         SHA-256 each file, skip unchanged (incremental)
[3] Parse        tree-sitter parse per language
[4] Extract      Symbols + call edges + boundary events + entry point hints
[5] Ingest       Write to SQLite (one transaction per file)
[6] LSP enrich   Optional: spawn language servers, hover-verify boundary types
[7] Stitch       Match boundary writes to reads → boundary_flows table
[8] Entrypoints  5 heuristics: exported leaves, route decorators, queue consumers,
                 main functions, cron/scheduler annotations
[9] Traces       DFS from each entry point up to configured depth → traces table
```

Steps 3, 4, and 9 are parallelised with rayon. Steps 5 and 8 are sequential (SQLite writes).

---

## Database schema

The index is a plain SQLite file — query it directly with `sqlite3` for ad-hoc analysis.

```sql
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

---

## Limitations

- **No cross-file type inference without LSP** — boundary detection on dynamic languages (JS/Python/Ruby) may produce false positives without `--lsp`
- **Traces require entry points** — if a function has no detected entry point, it won't appear in `traces`
- **Go, Java, C/C++ not supported** — tree-sitter grammars exist but are not yet integrated
- **No semantic deduplication** — two methods with the same name in different classes are treated separately

---

## License

MIT — see [LICENSE](LICENSE).
