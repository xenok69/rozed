# .rozedignore — Ignoring Files

`.rozedignore` is an optional file that tells rozed which local files to skip when watching for changes. It uses the same syntax as `.gitignore`.

Place it in the **project root** alongside `rozed.toml`.

---

## Why Use It

The file watcher fires on every save inside a mapped folder. Sometimes you want to exclude:

- Generated or compiled output files
- Test fixtures with large or noisy source
- Temporary files written by other tools
- Files you edit locally but never want pushed to Studio

`.rozedignore` lets you exclude them without removing them from your mappings.

---

## Syntax

`.rozedignore` uses [gitignore pattern syntax](https://git-scm.com/docs/gitignore#_pattern_format). Patterns are matched against paths **relative to the project root**.

### Ignore a specific file

```
src/shared/legacy.module.luau
```

### Ignore all files in a subfolder

```
src/shared/generated/
```

### Ignore by suffix (all test files)

```
*.spec.luau
```

### Ignore a folder anywhere in the tree

```
**/temp/
```

### Negate a pattern (re-include something previously excluded)

```
src/shared/generated/
!src/shared/generated/index.module.luau
```

---

## Full Example

```gitignore
# Ignore all generated output
src/shared/generated/

# Ignore test files everywhere
*.spec.luau
*.test.luau

# Ignore a specific legacy file
src/server/legacy_init.server.luau

# Ignore temp files from any tool
**/.tmp/
```

---

## Notes

- `.rozedignore` is read once on server startup. Changes to it require a restart.
- If the file doesn't exist, all matched `.luau` files are watched normally.
- The ignore only applies to the **push** direction (Zed → Studio). The pull flow (Studio → disk) is not affected.
- Patterns are matched using the `ignore` crate (the same engine used by `ripgrep` and `fd`), so behaviour is consistent with standard gitignore rules.
