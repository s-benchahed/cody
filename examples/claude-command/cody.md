---
description: Use cody (codebase semantic indexer) to analyze a project. Reads the codemap.md file.
argument-hint: "<question or task>"
---

You have access to **cody** — a static codebase semantic indexer for multi-language, multi-service repos.

## Philosophy

cody generates a `codemap.md` file from source code using tree-sitter (no LLM during indexing). The codemap captures:
- **Service topology**: which services call which, via what gRPC/kafka/redis keys
- **Routes**: HTTP endpoints grouped by service and auth middleware
- **I/O boundaries**: what each endpoint reads and writes (redis, sql, kafka, grpc)
- **Middleware/auth**: which routes are public vs. authenticated, and with what middleware

Read the codemap first to orient yourself, then read targeted source files for detail.

## Setup

```bash
# Auto-detect binary
CODY=$(which cody 2>/dev/null || echo "./target/release/cody")

# Generate / refresh the codemap (run from the project root)
$CODY . --out codemap.md

# Or with LSP verification (removes false positives, takes 30-120s):
$CODY . --out codemap.md --lsp
```

## Reading the codemap

The codemap is a markdown file — **read it directly**. It is organized as:

```
## Service Topology
<src_service>  →  <dst_service>   <medium>: <key1>, <key2>

## <service> [<language>]

### Public
METHOD /path
  in:   body{Type}, headers{key}
  <medium>: reads <key>
  <medium>: writes <key>
  grpc: → (Type)

### [auth: <middleware>]
...

### Background
<fn_name>
  <medium>: reads <key>
```

## Workflow

1. Read `codemap.md` (or the path specified in CLAUDE.md)
2. Use the topology section to understand cross-service dependencies
3. Find the service and route you care about
4. Read targeted source files for implementation detail

```bash
# Refresh the codemap if it exists but may be stale
$CODY . --out codemap.md

# Read the codemap
cat codemap.md

# Or read the codemap file directly with the Read tool
```

## Regenerating

```bash
CODY=$(which cody 2>/dev/null || echo "./target/release/cody")

# Basic (fast, ~5s)
$CODY <dir> --out codemap.md

# With LSP verification (more accurate, 30-120s)
$CODY <dir> --out codemap.md --lsp

# Custom depth or confidence
$CODY <dir> --out codemap.md --depth 8 --min-confidence 0.4
```

## Task

$ARGUMENTS

If no task is given, read `codemap.md` and summarize what services and routes exist, then ask what to explore.

Read the codemap file first. Do NOT run source file searches before reading it — use the codemap to orient, then read only the specific source files needed for detail.
