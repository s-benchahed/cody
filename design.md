# Codebase Semantic Index — Design Document (v2)

## Overview

A system that converts a polyglot codebase into a compact, semantically-rich
representation that LLMs can use to answer architectural and flow questions
without reading raw source code.

The core insight: **interactions between components are more valuable than
implementations**. A function's purpose can be inferred from how it connects
to the rest of the system. Traces — synthetic walks of the call+data_flow
graph — encode these interactions in a format LLMs already understand well.

At query time, the LLM receives a pre-built trace (~500 tokens) rather than
raw source (~50k tokens). It only fetches raw source for specific functions
when it needs implementation detail.

### What changed from v1

- **Language plugin architecture**: every language is a self-contained plugin.
  Adding a new language means adding one file, nothing else.
- **Tree-sitter everywhere**: Python, Go, Rust, and Java now use tree-sitter
  instead of regex. Regex is a last-resort fallback, not the default.
- **Provenance on every fact**: every symbol, edge, and boundary event carries
  a `source` tag (`ast | regex | llm | inferred`) and a `confidence` score
  (0.0–1.0). Downstream steps can filter by confidence.
- **Richer entry-point detection**: five independent heuristics, not just
  "exported + no callers".
- **SDK-based annotation**: uses the Anthropic Python SDK for retries, rate
  limiting, and timeout handling. Raw HTTP is gone.
- **Checkpoint/resume**: every step records progress to the DB so a partial
  failure can be resumed without re-running completed work.

---

## What already exists

The following files are already built and working. Do not rewrite them.
Extend them where noted.

### `build_index.py`

Builds a SQLite database from source code.

**Schema (existing — do not drop these tables):**
```sql
symbols(id, name, kind, file, line, signature, is_exported)
edges(id, src_file, src_symbol, rel, dst_file, dst_symbol, context, line)
file_meta(file, language, lines, exports, imports)
```

**Edge relation types already extracted:**
- `imports` — file-level dependency
- `calls` — function calls another function
- `reads` — function reads a member path (req.user.id)
- `writes` — function writes a member path (req.user = ...)
- `extends` — class inheritance
- `data_flow` — stitched post-hoc: writer of path X → reader of path X

**Entry point:** `python build_index.py <dir> --db index.db`

### `query_index.py`

CLI query interface over the index. Commands:
`lookup`, `callers`, `callees`, `deps`, `rdeps`, `path`, `flow`,
`carriers`, `dataflow`, `search`, `summary`, `stats`

**Entry point:** `python query_index.py --db index.db <command> <args>`

---

## File structure

```
indexer/
  build_index.py        ← EXISTS. Do not rewrite.
  query_index.py        ← EXISTS. Do not rewrite.

  plugins/              ← NEW. One file per language.
    base.py             ← LanguagePlugin protocol + shared AST helpers
    javascript.py       ← JS/TS (tree-sitter, already done in build_index)
    python.py           ← Python (tree-sitter-python)
    go.py               ← Go (tree-sitter-go)
    rust.py             ← Rust (tree-sitter-rust)
    java.py             ← Java/Kotlin (tree-sitter-java)
    fallback.py         ← Regex-only fallback for unsupported languages

  patterns.py           ← NEW (uses plugins, not standalone regex)
  annotate.py           ← NEW
  traces.py             ← NEW (replaces partial trace.py)
  embed.py              ← NEW
  search.py             ← NEW

  run_pipeline.py       ← NEW (orchestrates all build steps)
```

---

## Language Plugin Architecture

### `plugins/base.py`

Defines the protocol every language plugin must implement.

```python
from typing import Protocol, runtime_checkable

@runtime_checkable
class LanguagePlugin(Protocol):
    language:    str          # canonical name: "python", "go", "rust", ...
    extensions:  list[str]   # [".py"], [".go"], [".rs", ".toml"], ...
    tree_sitter: bool        # True if backed by tree-sitter, False = regex fallback

    def extract_symbols(
        self,
        src: str,
        filepath: str,
    ) -> list[Symbol]:
        """
        Return all functions, methods, classes, and constants.
        Must set provenance='ast' if tree-sitter, 'regex' if fallback.
        """

    def extract_edges(
        self,
        src: str,
        filepath: str,
        symbols: list[Symbol],
    ) -> list[Edge]:
        """
        Return call, import, extends, reads, writes edges.
        """

    def extract_boundary_events(
        self,
        src: str,
        filepath: str,
        symbol_map: dict[int, str],  # line → enclosing function name
    ) -> list[BoundaryEvent]:
        """
        Return boundary crossings (HTTP, Redis, Kafka, SQL, etc.).
        """

    def find_entry_point_hints(
        self,
        src: str,
        filepath: str,
        symbols: list[Symbol],
    ) -> list[EntryPointHint]:
        """
        Return language-specific entry point signals: route decorators,
        consumer annotations, main functions, etc.
        See entry-point detection section for hint types.
        """
```

### Provenance model

Every extracted fact carries a provenance tag:

```python
@dataclass
class Provenance:
    source:     str    # "ast" | "regex" | "llm" | "inferred"
    confidence: float  # 0.0–1.0
    plugin:     str    # e.g. "python", "go", "fallback"
    note:       str    # optional human-readable reason
```

Confidence guidelines:
- `ast` → 0.95 (tree-sitter misses dynamic dispatch, not a bug in the parser)
- `regex` → 0.70 (patterns can fire on comments, strings, test doubles)
- `llm` → 0.60 baseline, raised to 0.80 if it corroborates an `ast` fact
- `inferred` (stitched data_flow / boundary_flow) → min(confidence of inputs)

Facts with `confidence < 0.5` are stored but excluded from trace generation
by default. Pass `--low-confidence` to include them.

### Plugin registry

```python
# plugins/__init__.py
REGISTRY: dict[str, LanguagePlugin] = {}

def register(plugin: LanguagePlugin):
    for ext in plugin.extensions:
        REGISTRY[ext] = plugin

def get_plugin(filepath: str) -> LanguagePlugin:
    ext = Path(filepath).suffix.lower()
    return REGISTRY.get(ext, FALLBACK_PLUGIN)
```

Plugins register themselves on import:

```python
# plugins/python.py
from .base import register
...
register(PythonPlugin())
```

`build_index.py` and `patterns.py` both call `get_plugin(filepath)` — one
import path, one place to add a language.

### Tree-sitter grammars required

| Plugin | Grammar package |
|---|---|
| `javascript.py` | `tree-sitter-javascript` (already present) |
| `typescript.py` | `tree-sitter-typescript` (already present) |
| `python.py` | `tree-sitter-python` |
| `go.py` | `tree-sitter-go` |
| `rust.py` | `tree-sitter-rust` |
| `java.py` | `tree-sitter-java` |

`fallback.py` uses regex only and sets `tree_sitter = False` on all results.

---

## Step 1 — `patterns.py`: Boundary Event Pattern Registry

### Purpose

Detect when a value crosses a process boundary — HTTP header, cookie, Redis
key, Kafka topic, SQL table, environment variable, etc. This is deterministic
and requires no LLM.

### What a boundary event is

```python
@dataclass
class BoundaryEvent:
    direction:   str   # "read" | "write"
    medium:      str   # "http_header" | "http_body" | "http_query" |
                       # "cookie" | "redis" | "kafka" | "sql" |
                       # "grpc" | "env" | "jwt_claim" | "websocket" |
                       # "rabbitmq" | "sqs" | "pubsub" | "filesystem"
    key:         str   # the literal identifier at the boundary
    local_var:   str   # what the code calls it locally
    file:        str
    line:        int
    fn_name:     str
    raw_context: str   # the actual line of code
    prov:        Provenance
```

### Key normalisation

Strip all string interpolation to `{}`:
```
"session:{userId}"     → "session:{}"
`session:${userId}`    → "session:{}"
fmt.Sprintf("session:%d", id) → "session:{}"
f"session:{user_id}"  → "session:{}"
"session:" + userId    → "session:{}"
```

This allows cross-language stitching. The Python service writing
`f"session:{user_id}"` and the Node.js service reading `session:${userId}`
normalise to the same key `"session:{}"`.

### How patterns are now structured

Each language plugin's `extract_boundary_events` method handles patterns
for its own language using its AST. The shared `patterns.py` module
coordinates across plugins and runs the stitching step.

Pattern definitions move from a flat `PATTERNS` list to plugin-specific
methods. This allows AST-based extraction where possible (higher confidence)
and regex fallback only for languages without a plugin.

For the fallback plugin, maintain the original `PATTERNS` list format:

```python
PATTERNS: list[dict] = [
    {
        "id":        "http_header_write_js",
        "medium":    "http_header",
        "direction": "write",
        "languages": ["javascript", "typescript"],
        "pattern":   r'res\.(setHeader|set)\s*\(\s*[\'"]([^\'"]+)[\'"]',
        "key_group": 2,
        "var_group": None,
        "confidence": 0.70,
    },
    ...
]
```

### Required patterns — implement ALL of these

For languages with tree-sitter plugins, these are AST queries.
For the fallback plugin, these are regex patterns.

**HTTP headers:**
- `res.setHeader(name, value)` — JS write
- `res.set(name, value)` — Express write
- `req.headers[name]` / `req.headers.get(name)` — JS read
- `request.headers.get(name)` — Python read
- `r.Header.Get(name)` — Go read
- `r.Header.Set(name, value)` — Go write
- `c.Request.Header.Get(name)` — Gin read
- `c.Header(name, value)` — Gin write
- `response.headers[name]` — Python requests read
- `@Header(name)` decorator — NestJS / Spring

**Cookies:**
- `res.cookie(name, value)` — Express write
- `req.cookies[name]` / `req.cookies.name` — Express read
- `request.cookies.get(name)` — Python read
- `response.set_cookie(name, value)` — Python write
- `http.SetCookie(w, cookie)` — Go write
- `r.Cookie(name)` — Go read

**Redis:**
- `client.set(key, value)` — JS/Python write
- `client.get(key)` — JS/Python read
- `client.hset(hash, field, value)` — hash write
- `client.hget(hash, field)` — hash read
- `client.setex(key, ttl, value)` — write with TTL
- `redis.Set(ctx, key, value)` — Go write
- `redis.Get(ctx, key)` — Go read

**Kafka:**
- `producer.send({ topic, messages })` — JS write
- `consumer.subscribe({ topics })` — JS read
- `@KafkaListener(topics = name)` — Java/Kotlin annotation
- `producer.Produce(msg)` — Go write
- `consumer.Subscribe(topics)` — Go read
- `kafka.Producer.produce(topic, value)` — Python write
- `kafka.Consumer.poll()` with topic — Python read

**SQL — extract table names:**
- `db.query(sql)` — raw SQL, extract table from FROM/INTO/UPDATE
- `prisma.tableName.findMany/findFirst/findUnique` — Prisma read
- `prisma.tableName.create/update/upsert/delete` — Prisma write
- `Model.objects.filter()/get()/all()` — Django ORM read
- `Model.objects.create()/save()` — Django ORM write
- `db.Where(...).Find(&result)` — GORM read
- `db.Create(&model)` — GORM write
- `session.query(Model)` — SQLAlchemy read
- `session.add(model)` — SQLAlchemy write

**Environment variables:**
- `process.env.NAME` — JS read
- `os.environ.get('NAME')` / `os.getenv('NAME')` — Python read
- `os.Getenv("NAME")` — Go read
- `std::env::var("NAME")` — Rust read

**JWT claims:**
- `jwt.sign(payload, secret)` — extract payload keys as writes
- `jwt.verify(token, secret)` — result fields are reads
- `jwt.decode(token)` — Python read

**Message queues:**
- `channel.publish(exchange, routingKey, content)` — RabbitMQ write
- `channel.consume(queue, callback)` — RabbitMQ read
- `sqs.sendMessage({ QueueUrl, MessageBody })` — SQS write
- `sqs.receiveMessage({ QueueUrl })` — SQS read
- `pubsub.topic(name).publish(data)` — GCP Pub/Sub write
- `pubsub.subscription(name).on('message')` — GCP Pub/Sub read

**WebSocket:**
- `socket.emit(event, data)` — write
- `socket.on(event, handler)` — read
- `ws.send(data)` — write
- `ws.on('message', handler)` — read

**Filesystem (IPC):**
- `fs.writeFile(path, data)` — write
- `fs.readFile(path)` — read

### `patterns.py` exports

```python
def extract_boundary_events(
    src: str,
    filepath: str,
    language: str,
    symbol_map: dict[int, str],
) -> list[BoundaryEvent]:
    """
    Delegates to the plugin for the given language.
    Falls back to regex patterns if no plugin is registered.
    """

def normalise_key(raw: str) -> str:
    """
    Strip interpolation from a key string to canonical form.
    "session:{userId}" → "session:{}"
    """

def stitch_boundary_events(
    events: list[BoundaryEvent],
) -> list[tuple[BoundaryEvent, BoundaryEvent]]:
    """
    Find (write, read) pairs with the same (medium, normalised_key).
    Only stitch pairs where both events have confidence >= 0.5.
    Returns list of (writer, reader) pairs, each with inferred provenance.
    """
```

### Integration with build_index.py

Add to the schema:

```sql
CREATE TABLE IF NOT EXISTS boundary_events (
    id          INTEGER PRIMARY KEY,
    fn_name     TEXT NOT NULL,
    file        TEXT NOT NULL,
    line        INTEGER,
    direction   TEXT NOT NULL,
    medium      TEXT NOT NULL,
    key_raw     TEXT NOT NULL,
    key_norm    TEXT NOT NULL,
    local_var   TEXT,
    raw_context TEXT,
    prov_source     TEXT NOT NULL,   -- ast | regex | llm | inferred
    prov_confidence REAL NOT NULL,   -- 0.0–1.0
    prov_plugin     TEXT NOT NULL,
    prov_note       TEXT
);

CREATE TABLE IF NOT EXISTS boundary_flows (
    id           INTEGER PRIMARY KEY,
    write_fn     TEXT NOT NULL,
    write_file   TEXT NOT NULL,
    read_fn      TEXT NOT NULL,
    read_file    TEXT NOT NULL,
    medium       TEXT NOT NULL,
    key_norm     TEXT NOT NULL,
    confidence   REAL NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_be_key  ON boundary_events(key_norm);
CREATE INDEX IF NOT EXISTS idx_be_fn   ON boundary_events(fn_name);
CREATE INDEX IF NOT EXISTS idx_be_conf ON boundary_events(prov_confidence);
CREATE INDEX IF NOT EXISTS idx_bf_key  ON boundary_flows(key_norm);
```

Add to `query_index.py`:
- `boundaries <fn_name>` — all boundary events for a function
- `medium <medium_name>` — all events for a medium (e.g. `medium redis`)
- `cross <key_norm>` — all functions reading/writing a boundary key
- `lowconf` — list events with confidence < 0.7 (review candidates)

---

## Step 2 — Entry Point Detection

### Purpose

Identify functions that are called from outside the codebase. These become
the roots of traces. The v1 heuristic (exported + no internal callers) is
necessary but not sufficient — it misses route handlers, queue consumers,
DI-injected services, and scheduled jobs.

### Five independent heuristics

Run all five; combine results with a union. Each match records which
heuristics fired, so confidence accumulates.

#### Heuristic 1: Structural (existing)

```sql
SELECT s.name, s.file, s.line
FROM symbols s
WHERE s.is_exported = 1
AND s.name NOT IN (
    SELECT dst_symbol FROM edges
    WHERE rel = 'calls' AND dst_symbol IS NOT NULL
)
```

Confidence: 0.70 (can fire on utility functions that happen to be uncalled)

#### Heuristic 2: Route decorator / registration

Language-specific patterns via plugins. Each plugin's
`find_entry_point_hints` detects framework-level route registration:

```python
@dataclass
class EntryPointHint:
    fn_name:    str
    file:       str
    line:       int
    kind:       str    # "route" | "consumer" | "main" | "cron" | "event" | "test"
    framework:  str    # "express" | "fastapi" | "gin" | "spring" | ...
    path:       str    # route path, topic name, cron expression, etc.
    method:     str    # HTTP method or empty string
    confidence: float
```

Examples of what plugins must detect:

```python
# FastAPI / Flask (Python plugin)
@app.get("/users/{id}")
@router.post("/orders")

# Express (JS plugin)
router.get('/health', handler)
app.post('/api/v1/users', handler)

# Gin (Go plugin)
r.GET("/ping", handler)
r.POST("/users", createUser)

# Spring (Java plugin)
@GetMapping("/users")
@PostMapping("/orders")
@KafkaListener(topics = "user.created")

# NestJS (JS plugin)
@Get('users')
@MessagePattern('user.created')
```

Confidence: 0.90 (framework annotation is unambiguous)

#### Heuristic 3: Queue / event consumer annotation

Kafka, RabbitMQ, SQS, Pub/Sub, WebSocket connection handlers.
Detected by the same plugin `find_entry_point_hints` method, kind = "consumer".

Examples:
```python
consumer.subscribe(['user.events'])   # Python kafka
@EventPattern('user.created')         # NestJS
channel.consume('order.queue', ...)   # RabbitMQ
```

Confidence: 0.90

#### Heuristic 4: Main / CLI entry

Functions named `main`, `run`, `start`, `serve`, or decorated with
`@click.command()`, `@app.cli.command()`, or detected as the entry of
an executable module.

Confidence: 0.80

#### Heuristic 5: Scheduled / cron job

```python
@celery.task
@scheduler.task('cron', ...)
cron.schedule(fn, ...)
```

Confidence: 0.85

### Entry point table

```sql
CREATE TABLE IF NOT EXISTS entry_points (
    id           INTEGER PRIMARY KEY,
    fn_name      TEXT NOT NULL,
    file         TEXT NOT NULL,
    line         INTEGER,
    kind         TEXT NOT NULL,
    framework    TEXT,
    path         TEXT,
    method       TEXT,
    confidence   REAL NOT NULL,
    heuristics   TEXT NOT NULL    -- JSON array of heuristic names that fired
);
```

Functions that match multiple heuristics get `confidence = max(individual scores)`.
Entry points with `confidence < 0.6` are stored but excluded from trace
generation by default. Pass `--all-entrypoints` to include them.

---

## Step 3 — `annotate.py`: Cheap LLM Annotation Pass

### Purpose

Add semantic meaning on top of the deterministic skeleton. The LLM does
exactly three things and nothing else:

1. **Name values** — assign `◆snake_case_name` to each boundary event's
   local variable and to data_flow edge values
2. **Write one-sentence purpose** per function
3. **Catch boundary events the pattern registry missed** — framework-specific
   or unusual patterns

### Critical constraint

The LLM output is validated against the deterministic index. If the LLM
claims a function calls something that tree-sitter did not find, the claim
is dropped. The deterministic layer is the ground truth.

### Annotation prompt (per function)

```
You are extracting semantic annotations from code. Be precise and concise.

FUNCTION: {fn_name}
FILE: {filepath}

SOURCE:
{function_source_code}

STRUCTURAL CONTEXT (from static analysis — ground truth):
calls:  {list of functions this calls}
reads:  {list of member paths read}
writes: {list of member paths written}
boundary_events: {list of boundary events found by pattern registry}

Produce a JSON object with exactly these fields:
{
  "purpose": "one sentence, ≤15 words, active voice",
  "value_names": {
    "local_var_name": "◆snake_case_semantic_name",
    ...
  },
  "missed_boundaries": [
    {
      "direction": "read|write",
      "medium": "http_header|cookie|redis|kafka|sql|...",
      "key": "the literal key",
      "local_var": "variable name in source"
    }
  ]
}

Rules:
- value_names: only name variables that cross a boundary or flow between functions
- ◆names must be globally meaningful, not local (◆session_token not ◆token)
- missed_boundaries: only include if you are certain. If unsure, omit.
- purpose: describe what the function does for the system, not how
```

### Validation pipeline

After receiving LLM output, run these checks in order. Each failure is
logged with function name and reason. Failed checks do not abort the run —
the deterministic skeleton is used without that annotation.

```python
class AnnotationValidator:
    def validate(self, raw: str, fn_name: str, index_facts: dict) -> ValidationResult:
        """
        Steps:
        1. JSON parse. If malformed → retry once with explicit error feedback.
           If still malformed → FAIL: "unparseable JSON after retry"

        2. Schema check: required fields present, correct types.
           → FAIL: "missing field: {field}"

        3. purpose: ≤ 20 words (allow slight overage), active voice.
           If > 30 words → truncate with warning, do not fail.

        4. value_names: all ◆names must match /^◆[a-z][a-z0-9_]+$/.
           → DROP individual names that don't match, log warning.

        5. missed_boundaries: medium must be in KNOWN_MEDIUMS.
           → DROP entries with unknown medium.

        6. missed_boundaries: cross-check against AST. If LLM claims a
           boundary for a line that tree-sitter parsed and found nothing,
           assign confidence = 0.55 (possible but unverified).
           If line was not parsed (dynamic code), assign confidence = 0.65.
        """
```

### Caching

Cache by `sha256(function_source_code)`. Incremental rebuilds only annotate
changed functions.

```sql
CREATE TABLE IF NOT EXISTS annotations (
    source_hash      TEXT PRIMARY KEY,
    fn_name          TEXT NOT NULL,
    file             TEXT NOT NULL,
    purpose          TEXT,
    value_names      TEXT,         -- JSON
    validation_notes TEXT,         -- JSON array of warnings
    annotated_at     TEXT
);
```

### SDK usage

Use the Anthropic Python SDK. Do not use raw HTTP.

```python
import anthropic

client = anthropic.Anthropic(api_key=api_key)

def annotate_function(fn_name, source, context, model, max_retries=2):
    for attempt in range(max_retries + 1):
        try:
            msg = client.messages.create(
                model=model,
                max_tokens=512,
                messages=[{"role": "user", "content": build_prompt(fn_name, source, context)}],
                timeout=30.0,
            )
            return validator.validate(msg.content[0].text, fn_name, context)
        except anthropic.RateLimitError:
            time.sleep(2 ** attempt)   # exponential backoff
        except anthropic.APITimeoutError:
            if attempt == max_retries:
                return ValidationResult.skip("timeout")
        except anthropic.APIError as e:
            log.warning(f"API error for {fn_name}: {e}")
            return ValidationResult.skip(str(e))
```

### Checkpointing

Write progress to the DB after every batch of 50 functions:

```sql
CREATE TABLE IF NOT EXISTS pipeline_checkpoints (
    step        TEXT NOT NULL,
    key         TEXT NOT NULL,    -- e.g. fn hash or file path
    status      TEXT NOT NULL,    -- "done" | "skipped" | "failed"
    detail      TEXT,
    updated_at  TEXT,
    PRIMARY KEY (step, key)
);
```

On resume, skip any key already marked "done" or "skipped".
Failed keys are retried unless `--no-retry-failed` is passed.

### Model selection

Default: `claude-haiku-4-5-20251001`. Configurable via `--model`.

### Entry point

```
python annotate.py --db index.db --api-key $ANTHROPIC_API_KEY [--model haiku] [--no-retry-failed]
```

Shows progress: `Annotating 847/2341 functions... (cached: 1494, failed: 3)`

---

## Step 4 — `traces.py`: Synthetic Trace Generator

### Purpose

Walk the call + data_flow + boundary_flow graph from every entry point and
produce a synthetic trace — a hierarchical representation of how data flows
through the system for that entry point.

### Span model

```python
@dataclass
class Span:
    id:           str
    trace_id:     str
    parent_id:    Optional[str]
    fn_name:      str
    service:      str            # derived from file path (first meaningful dir)
    file:         str
    line:         Optional[int]
    edge_kind:    str            # root | call | data_flow | boundary_flow
    depth:        int
    purpose:      Optional[str]
    reads:        list[str]
    writes:       list[str]
    boundary_in:  list[BoundaryEvent]
    boundary_out: list[BoundaryEvent]
    baggage:      list[str]      # ◆value_names flowing through this span
    confidence:   float          # min confidence of facts contributing to this span
    children:     list['Span']
```

### Trace serialisation — LLM-optimised text format

```
TRACE: processOrder  [checkout-service]  checkout/order.js:45
baggage: ◆session_token ◆user_id ◆order_items

├─ processOrder  [checkout]  order.js:45
│  reads:  req.cookies["sessionToken"] → ◆session_token
│  reads:  req.body.items → ◆order_items
│  │
│  ├─ call → validateSession(◆session_token)  [auth]  auth/session.js:12
│  │         reads: redis["session:{}"] → ◆session_data
│  │         writes: ◆session_data.userId → ◆user_id
│  │
│  ├─ call → checkInventory(◆order_items)  [inventory]  inventory/check.js:8
│  │         reads: sql[inventory] WHERE sku IN ◆order_items
│  │         guard: all items in stock | FALSE → EXIT 409
│  │
│  ├─ data_flow ~~► fulfillmentHandler  [fulfillment]  fulfillment/handler.js:23
│  │  via: kafka["order.created"]
│  │  carries: ◆user_id ◆order_items
│  │
│  └─ write: sql[orders] ← { ◆user_id, ◆order_items }
│
```

Low-confidence spans (< 0.5) are rendered with a `?` prefix:
```
│  ?├─ call → maybeHandler  [unknown]  ...   # confidence: 0.42
```

### Trace generation algorithm

```python
def generate_trace(
    conn,
    root_fn: str,
    root_file: str,
    max_depth: int = 6,
    max_tokens: int = 2000,
    min_confidence: float = 0.5,
) -> Span:
```

Walk order:
1. Start at root, create root span
2. For each `calls` edge: create child span
3. For each `data_flow` edge: create child span
4. For each `boundary_flow` where current fn is writer: create child span
5. Skip edges with confidence < min_confidence (unless `--low-confidence`)
6. Cycle detection: skip if fn_name already in current path
7. Token budget: estimate tokens as `len(serialize(span)) // 4`, stop expanding
   the current subtree when over budget (do not silently truncate mid-tree —
   add a `[truncated: N children omitted]` marker)

### Serialisation functions

```python
def serialize_trace(root: Span) -> str:
    """LLM-optimised text format shown above."""

def serialize_otlp(root: Span) -> dict:
    """OpenTelemetry-compatible JSON for Jaeger/Zipkin."""

def serialize_compact(root: Span) -> str:
    """
    Ultra-compact, one line per span. Used for embedding, not LLM reading.

    processOrder [checkout] → validateSession [auth] → redis:session:{} READ
    processOrder [checkout] → checkInventory [inventory] → sql:inventory READ
    processOrder [checkout] ~~kafka:order.created~~► fulfillmentHandler [fulfillment]
    processOrder [checkout] → sql:orders WRITE
    """
```

### Storage

```sql
CREATE TABLE IF NOT EXISTS traces (
    id           INTEGER PRIMARY KEY,
    trace_id     TEXT UNIQUE NOT NULL,
    root_fn      TEXT NOT NULL,
    root_file    TEXT NOT NULL,
    service      TEXT NOT NULL,
    text         TEXT NOT NULL,
    compact      TEXT NOT NULL,
    otlp         TEXT,
    span_count   INTEGER,
    fn_names     TEXT,           -- JSON array
    media        TEXT,           -- JSON array of all media touched
    value_names  TEXT,           -- JSON array of all ◆names in trace
    min_confidence REAL,         -- lowest confidence span in trace
    created_at   TEXT
);
```

### Entry point

```
python traces.py --db index.db [--depth 6] [--otlp] [--low-confidence] [--min-confidence 0.5]
```

---

## Step 5 — `embed.py`: Trace Embedding

### Purpose

Embed each trace for semantic retrieval. Uses sqlite-vec — no external
infrastructure required.

### Embedding strategy

Embed the `compact` field concatenated with metadata:
```
{root_fn} {service} {" ".join(fn_names)} {" ".join(media)} {" ".join(value_names)}
```

The compact format encodes structure without prose — better retrieval on
structural questions ("what touches redis", "show me the auth flow").

### Schema

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS trace_embeddings USING vec0(
    trace_id TEXT PRIMARY KEY,
    embedding FLOAT[1536]
);
```

### Embedding model

Default: `text-embedding-3-small` (OpenAI, 1536 dims).
Alternative: `nomic-embed-text` via Ollama (free, local, 768 dims).
Configurable via `--embedding-model`.

Incremental: only embeds traces not yet in `trace_embeddings`.

### Entry point

```
python embed.py --db index.db --api-key $OPENAI_API_KEY [--embedding-model text-embedding-3-small]
```

---

## Step 6 — `search.py`: Query Interface

### Purpose

The interface the LLM uses at query time. Five tools that allow navigation
from a high-level question down to raw source.

### Tool 1: `search_traces`

```python
def search_traces(question: str, top_n: int = 3) -> list[dict]:
    """
    Embed the question, find top_n most similar traces.
    Returns [{trace_id, root_fn, service, text, score, min_confidence}].
    """
```

### Tool 2: `get_skeleton`

```python
def get_skeleton(fn_name: str) -> str:
    """
    Compact semantic skeleton for a function.
    Combines structural edges + boundary events + annotations.
    Marks low-confidence facts with '?'.

    FN  authMiddleware  middleware/auth.js:12
    purpose: validates session cookie, enriches request with user identity
      READ    cookie["sessionToken"]           → ◆session_token
      CALL    verifyToken(◆session_token)      → ◆token_payload    | THROWS → 401
      READ    ◆token_payload.userId            → ◆user_id
      CALL    getUserPerms(◆user_id)           → ◆permissions
      GUARD   "write" ∈ ◆permissions           | FALSE → 403
      WRITE   req.user                         ← { ◆user_id, ◆permissions }
      WRITE   http_header["X-User-Id"]         ← ◆user_id
      CALL    next()
    """
```

### Tool 3: `get_source`

```python
def get_source(fn_name: str, context_lines: int = 5) -> str:
    """Raw source for a function, with file:line header."""
```

### Tool 4: `get_carriers`

```python
def get_carriers(value_name: str) -> str:
    """All spans/functions that carry a ◆value or boundary key."""
```

### Tool 5: `get_traces_touching`

```python
def get_traces_touching(medium: str, key: str = None) -> str:
    """
    All traces touching a boundary medium/key.
    Useful for impact analysis: "what breaks if I change the users table?"
    """
```

### Query runner

```python
def run_query(question: str, db_path: str, api_key: str) -> str:
    """
    1. search_traces(question) → top 3 traces
    2. Build system prompt with tool definitions
    3. Call claude-sonnet-4-6 (default)
    4. Handle tool calls in a loop until final answer
    5. Return answer
    The LLM never sees raw source unless it explicitly calls get_source.
    """
```

### System prompt

```
You are a codebase navigation assistant with access to a pre-built semantic
index. The index contains synthetic traces — pre-computed paths showing how
data flows through the system.

Tools:
- search_traces(question): find relevant pre-built traces
- get_skeleton(fn_name): compact semantic skeleton of one function
- get_source(fn_name): raw source code for a function
- get_carriers(value): all components that handle a value
- get_traces_touching(medium, key): traces touching a resource

Strategy:
1. Always call search_traces first
2. Read the traces — they often contain the full answer
3. For detail on a specific function, call get_skeleton
4. Only call get_source if you need the exact implementation
5. Never speculate — use the tools

Notation:
- ◆name = semantic value identity that persists across component boundaries
- ~~► = data_flow edge (value crosses via shared state, not direct call)
- via: kafka["topic"] = the medium and key at a boundary crossing
- GUARD: condition | FALSE → outcome = access control check
- ? prefix = low-confidence fact (< 0.5), treat with caution
```

### Entry point

```
python search.py --db index.db --api-key $ANTHROPIC_API_KEY "how does the session token reach the database?"
```

---

## Step 7 — `run_pipeline.py`: Build Orchestrator

### Usage

```
python run_pipeline.py <dir> --db index.db --api-key $ANTHROPIC_API_KEY

Options:
  --skip-annotate         skip LLM annotation
  --skip-embed            skip embedding
  --depth 6               trace depth
  --model haiku           annotation model
  --embedding-model text-embedding-3-small
  --min-confidence 0.5    exclude facts below this threshold
  --all-entrypoints       include low-confidence entry points
  --low-confidence        include low-confidence spans in traces
  --no-retry-failed       skip previously failed annotation jobs
```

### Steps

```
1. build_index      → symbols, edges, reads, writes, data_flow
2. detect_plugins   → log which languages were detected and which plugin handled each
3. extract_patterns → boundary_events, boundary_flows
4. detect_entrypoints → entry_points table (all 5 heuristics)
5. annotate         → purposes, value_names            (skippable)
6. generate_traces  → traces table
7. embed            → trace_embeddings                 (skippable)
```

Print timing per step. Print final summary:
```
Index built in 4.2s
  Languages detected: typescript (847 files, plugin: ast), python (312 files, plugin: ast),
                      go (89 files, plugin: ast), java (23 files, plugin: ast)
  2,341 functions  |  18,429 edges (calls: 12k, data_flow: 3k, reads: 2k, writes: 1k)
  847 boundary events  (redis: 142 [ast], kafka: 89 [ast], sql: 312 [ast],
                        http_header: 198 [ast/regex], cookie: 106 [ast])
  12 low-confidence events excluded (run with --low-confidence to include)
  234 boundary flows

Entry points: 142 detected
  route: 98 (confidence ≥ 0.90), consumer: 31, main: 8, cron: 5
  6 low-confidence entry points excluded

Annotated in 38.2s  (847 LLM calls, $0.18)
  cached: 1,494  |  annotated: 847  |  skipped: 0  |  failed: 3
  3 annotation failures logged to stderr

Traces generated in 1.1s
  142 traces, avg 14 spans, avg 510 tokens
  8 traces contain truncated subtrees (run with --depth 8 to expand)

Embedded in 12.3s

Ready. Query with:
  python search.py --db index.db "how does auth work?"
```

---

## Key design constraints

### Do not break existing functionality

`build_index.py` and `query_index.py` must continue to work exactly as before.
All new tables are additive. The plugin system extends `build_index.py` by
calling `get_plugin(filepath)` instead of its current language branch.

### Deterministic layer is ground truth

If LLM annotation contradicts tree-sitter extraction:
- tree-sitter wins
- LLM annotation stored with `prov_source = "llm"` and `confidence = 0.60`
- `get_skeleton` shows deterministic structure; annotates with LLM names where
  confidence threshold is met

### Fail gracefully

- `--skip-annotate`: skeletons use `$v1`-style names. Fully functional.
- `--skip-embed`: `search_traces` unavailable, other tools work. Warning printed.
- LLM API unavailable: pipeline stops at annotation step, suggests `--skip-annotate`.
- Per-function annotation failures: logged, skeleton used without annotation.
- Checkpoint table allows resume from any failed step.

### Incremental rebuilds

All steps are incremental by default:
- `build_index`: re-extracts only changed files (hash-based)
- `annotate`: re-annotates only changed functions (hash-based)
- `traces`: regenerates only traces whose root function changed
- `embed`: re-embeds only changed traces

### Language support

Adding a new language requires exactly one thing: a new file in `plugins/`.
The file must implement `LanguagePlugin`, register itself, and ideally use
a tree-sitter grammar. Regex fallback is available for prototyping.

---

## Testing

```
tests/
  fixtures/
    sample/
      middleware/auth.js
      api/userResolver.js
      api/adminPanel.js
      utils/jwt.js
    multi_service/
      service_a/index.js        ← writes X-User-Id header, publishes to kafka
      service_b/handler.go      ← reads X-User-Id header, reads from kafka
      service_c/consumer.py     ← reads from kafka, writes to postgres

  test_plugins.py     ← each plugin extracts symbols/edges correctly
  test_patterns.py    ← boundary event extraction, key normalisation, confidence
  test_entrypoints.py ← all 5 heuristics, combined confidence, exclusion threshold
  test_annotate.py    ← validation pipeline, retry logic, checkpointing
  test_traces.py      ← trace generation, truncation markers, confidence filtering
  test_search.py      ← tool functions (mock LLM)
  test_pipeline.py    ← end-to-end on fixtures
```

The `multi_service` fixture is critical — it validates cross-language boundary
stitching across JS, Go, and Python, which is the hardest part of the system.

Each test in `test_patterns.py` must assert `prov_source` and that
`prov_confidence` falls in the expected range for the extraction method used.

---

## Dependencies

```
# Already used
tree-sitter
tree-sitter-javascript

# New tree-sitter grammars
tree-sitter-python
tree-sitter-go
tree-sitter-rust
tree-sitter-java

# New infrastructure
sqlite-vec          # vector search in SQLite
openai              # embeddings
anthropic           # annotation LLM + query LLM (SDK, not raw HTTP)

# Optional
ollama              # local embeddings
```

Install:
```
pip install tree-sitter tree-sitter-javascript tree-sitter-python \
            tree-sitter-go tree-sitter-rust tree-sitter-java \
            sqlite-vec openai anthropic
```

---

## What success looks like

Given the `multi_service` fixture:

```
python search.py --db index.db "how does the user id flow from service_a to postgres?"
```

Expected answer (paraphrased):
> service_a writes the user ID to the X-User-Id HTTP header (ast, confidence: 0.95).
> service_b reads this header and publishes it to the Kafka topic "user.events"
> (ast, confidence: 0.95). service_c consumes "user.events" and writes the user ID
> to the postgres users table (ast, confidence: 0.95).

The LLM should answer this without calling `get_source`. The answer should
reference the specific header name, topic name, and table name. All contributing
facts should have `prov_source = "ast"` — no regex or LLM inference required for
the happy path.
