# cody

A static codebase semantic indexer for multi-language, multi-service repositories.

cody scans source code using tree-sitter and produces a single `codemap.md` file — no LLM during indexing. The codemap captures HTTP routes, I/O boundaries (redis, sql, kafka, grpc), middleware/auth, and cross-service data flows, organized by service.

**Best used alongside code reading, not as a replacement.** cody orients you quickly — understand service dependencies, see all routes and their auth, trace what each endpoint reads and writes — then read the targeted source files for detail.

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
# Generate codemap for your project
cody ./my-project --out codemap.md

# Or with LSP verification (removes false positives):
cody ./my-project --out codemap.md --lsp

# Then read it
cat codemap.md
```

---

## CLI reference

```
cody <dir> [OPTIONS]

Options:
  --out <path>           Output file path [default: codemap.md]
  --depth <n>            Max call graph depth [default: 6]
  --lsp                  Enable LSP type verification (removes false positives)
  --min-confidence <f>   Minimum confidence threshold [default: 0.5]
```

Re-running on the same directory is incremental — only changed files are reprocessed (cached in `.cody-cache`).

---

## Output format

```
# Codemap — my-project
Generated: 2026-03-23 | Files: 350 | Languages: rust, typescript

## Service Topology
service     →  handlers    grpc: GetProfileRequest
client-app  →  service     grpc: LoginRequest, DeleteAccountRequest

## service [rust]

### Public
POST /login
  in:   body{LoginRequest}
  redis:  reads x-timezone
  grpc: → (response)

### [auth: with_lp_auth]
POST /feed/get
  in:   body{GetFeedRequest}
  redis:  reads authorization, x-timezone
  grpc: → (request), → (response)

### Background
lp_auth_middleware
  redis:  reads authorization, x-timezone
```

Routes are grouped by service, then by auth middleware (Public / `[auth: X]` / Background).

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
[1] Walk      Find all source files matching supported extensions
[2] Hash      SHA-256 each file, skip unchanged (incremental via .cody-cache)
[3] Parse     tree-sitter parse per language
[4] Extract   Symbols + call edges + boundary events + entry point hints
[5] Enrich    Optional: spawn language servers, hover-verify boundary types
[6] Assemble  Build adjacency map + boundary index + entry points (in-memory)
[7] Write     Generate codemap.md
```

Steps 3 and 4 are parallelised with rayon.

---

## Limitations

- **No cross-file type inference without LSP** — boundary detection on dynamic languages (JS/Python/Ruby) may produce false positives without `--lsp`
- **Coverage requires detected entry points** — functions with no detected entry point (not exported, no route, no main) won't appear in the codemap
- **Go, Java, C/C++ not supported** — tree-sitter grammars exist but are not yet integrated
- **No semantic deduplication** — two methods with the same name in different classes are treated separately

---

## License

MIT — see [LICENSE](LICENSE).
