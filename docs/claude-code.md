# Claude Code Integration

cody ships with a `/cody` slash command for Claude Code that lets you query any indexed codebase directly from your Claude Code session.

## Install in one step

```bash
bash scripts/install-claude-code.sh
```

This copies the slash command to `~/.claude/commands/cody.md` and adds the required permissions to `~/.claude/settings.json`.

## Manual install

```bash
# 1. Copy the slash command
cp examples/claude-command/cody.md ~/.claude/commands/cody.md

# 2. Add permissions to ~/.claude/settings.json
# Add these entries to the "allow" array:
#   "Bash(cody:*)"
#   "Bash(./target/release/cody:*)"
#   "Bash(sqlite3:*)"
```

## Add a project CLAUDE.md snippet

For each project you index, add a section to its `CLAUDE.md` so Claude Code automatically knows where the index is:

```markdown
## Cody Index

Index: `/absolute/path/to/your-project.db`
Binary: `cody` (or `/absolute/path/to/cody/target/release/cody`)

Re-index after significant changes:
  cody index . --db your-project.db [--lsp]
```

See `examples/project-claude.md` for a copy-paste template.

## Using the slash command

```
/cody <question or task>
```

Examples:
```
/cody how does authentication work in this service?
/cody what redis keys does the order processing flow write?
/cody show me the service topology
/cody which routes require admin privileges?
```

Claude will use `cody query` commands and `sqlite3` to answer from the index, then read targeted source files only for detail.

## What the command auto-detects

- **Binary**: tries `cody` (PATH), falls back to `./target/release/cody`
- **Database**: uses the most recently modified `*.db` file in the current directory

If multiple `.db` files exist, specify the one you want:
```
/cody --db ./my-project.db how does auth work?
```
Or set `DB` explicitly in your shell before invoking.

## Permissions reference

The install script adds these to `~/.claude/settings.json`:

```json
{
  "permissions": {
    "allow": [
      "Bash(cody:*)",
      "Bash(./target/release/cody:*)",
      "Bash(sqlite3:*)"
    ]
  }
}
```

If you keep cody at a non-standard path, add:
```json
"Bash(/your/path/to/cody:*)"
```
