# Cody Codemap — Project CLAUDE.md Snippet

Copy the section below into your project's `CLAUDE.md` file and fill in the paths.

---

```markdown
## Cody Codemap

A semantic codemap of this project is at `/absolute/path/to/codemap.md`
(generated with cody — static indexer using tree-sitter, no LLM during indexing).

**Read it before exploring source files.** It shows:
- Service topology (which services call which, via what medium/keys)
- All HTTP routes grouped by service and auth middleware
- I/O boundaries per endpoint (redis, sql, kafka, grpc reads and writes)

```bash
# Regenerate after significant code changes
cody /path/to/project --out /absolute/path/to/codemap.md [--lsp]
```
```
