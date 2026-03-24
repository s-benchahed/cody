# Claude Code Integration

cody ships with a `/cody` slash command for Claude Code that lets you explore any codebase from your Claude Code session by reading its `codemap.md` file.

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
```

## Add a project CLAUDE.md snippet

For each project you index, add a section to its `CLAUDE.md` so Claude Code automatically knows where the codemap is:

```markdown
## Cody Codemap

Codemap: `/absolute/path/to/codemap.md`
Binary: `cody` (or `/absolute/path/to/cody/target/release/cody`)

Regenerate after significant changes:
  cody . --out codemap.md [--lsp]
```

See `examples/project-claude.md` for a copy-paste template.

## Workflow

**One-time setup per project:**
```bash
# Generate the codemap
cody ./my-project --out my-project/codemap.md
```

**Using the slash command:**
```
/cody <question or task>
```

Examples:
```
/cody how does authentication work in this service?
/cody what does the login endpoint read and write?
/cody show me all routes that require admin access
/cody how do service-a and service-b communicate?
```

Claude reads `codemap.md` to orient itself, then reads targeted source files only for detail.

## What the command does

1. Reads the `codemap.md` file (path from CLAUDE.md or auto-detected)
2. Uses the topology and route sections to answer your question
3. Reads specific source files only when needed for implementation detail

No CLI queries, no database — just reading a file.

## Permissions reference

The install script adds these to `~/.claude/settings.json`:

```json
{
  "permissions": {
    "allow": [
      "Bash(cody:*)",
      "Bash(./target/release/cody:*)"
    ]
  }
}
```
