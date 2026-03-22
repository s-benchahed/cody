# Cody Index — Project CLAUDE.md Snippet

Copy the section below into your project's `CLAUDE.md` file and fill in the paths.

---

```markdown
## Cody Index

A semantic index of this codebase is at `/absolute/path/to/your-project.db`
(built with LSP enrichment — boundary event types verified by language servers).

Use the `/cody` slash command or run queries directly before reading source files:

```bash
CODY=cody   # or /absolute/path/to/cody/target/release/cody
DB=/absolute/path/to/your-project.db

# Service dependency graph
$CODY query --db $DB topology

# All HTTP routes with auth middleware
sqlite3 $DB 'SELECT fn_name, path, method, middleware FROM entry_points WHERE path IS NOT NULL ORDER BY path;'

# Execution trace from an entry point
$CODY query --db $DB traces <fn_name>

# What I/O does a function touch?
$CODY query --db $DB boundaries <fn_name>

# Re-index after significant code changes
$CODY index /path/to/project --db /path/to/your-project.db --lsp
```
```
