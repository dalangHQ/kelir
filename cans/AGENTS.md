# CANS Agent Instructions

You are working in a CANS project. Specs live in `cans/`.

## Reading

Find concept → read canonical home → follow `see:` ONE hop. Never chain.

```
# GOOD: read canonical, then one ref
cans/02-authentication.md  →  "Sessions" node
cans/04-api.md             →  "Session rules: see 02-authentication.md#Sessions"

# FORBIDDEN: following a see: inside the referenced file
04-api.md → see 02-auth.md → see 06-ops.md   ← DEEP HOP, stop at 02-auth.md
```

Read surrounding hierarchy for context, not just the leaf:

```
# You need "Refresh allowed for 30 days"
# Read the parent chain:
- Authentication          ← context
  - Sessions              ← parent
    - Expire after 24 hours
    - Refresh allowed for 30 days   ← target
```

## Writing

One canonical home per concept. Reference, don't duplicate.

```
# GOOD: 04-api.md references the canonical home
- Authentication
  - Session rules: see 02-authentication.md#Sessions

# BAD: duplicating session rules in 04-api.md
- Authentication
  - Sessions expire after 24 hours    ← DUPLICATE, use see: instead
  - Refresh allowed for 30 days       ← DUPLICATE
```

Preserve real identifiers exactly:

```
# GOOD
- POST /users
- packages/api
- users.created_at

# BAD
- post-users
- packages-api
- users-created-at
```

Mark unknowns as TBD. Don't guess:

```
# GOOD
- Rate limit: TBD (waiting on infra decision)

# BAD
- Rate limit: 1000 req/min   ← invented, no source
```

If a level adds no meaning, don't create it:

```
# BAD (filler)
- Authentication
  - Capability
    - Behavior
      - Sign up

# GOOD (dense)
- Authentication
  - Sign up
```

## Changing

Find canonical home → update there → check references → verify affected code.

```
# Changing session expiry from 24h to 48h:
1. Edit cans/02-authentication.md  →  "Expire after 48 hours"
2. Run: cans check --refs-only
3. Verify back-pointers still valid
4. Do NOT edit 04-api.md (it only has see:, not the value)
```

Never silently resolve conflicts. Report them:

```
# If you disagree with a spec:
- Add to _collab/conflicts.md
- Format: - status: unresolved | file: 04-api.md | issue: <description>
- Do NOT overwrite the spec
```

Smallest correct change. Don't restructure what isn't broken.

## ADRs

Decisions get an ADR. ADRs record WHY. Specs are the truth.

```
cans new adr "postgres-over-mysql"
# Creates: _adr/003-postgres-over-mysql.md
# Fill: Context → Decision → Alternatives → Consequences
# Reference spec: see 03-data.md#Storage
```

## Tasks

Implementation tracking via task files:

```
cans new task "add-dark-mode"
# Creates: _tasks/add-dark-mode.md
```

Task file structure:

```markdown
# add-dark-mode
- Owner: agent-1
- ADR: _adr/001-css-variables-over-tailwind.md
- Tasks
  - [x] Add ThemeContext provider ← agent-1
  - [ ] Create toggle component ← agent-1
  - [ ] Test: no FOUC ← agent-2
- Review
  - [ ] Spec approved ← @human
```

When done: `cans done add-dark-mode`
Blocks if `← @human` gates unchecked. Always.

## Collaboration

One owner per task file. Claim before editing.

```
# If task has ← agent-3, do NOT edit it
# If you need to hand off, add to _collab/handoffs.md:
- from: agent-1 → to: agent-3 | task: add-dark-mode | context: color audit pending
```

## Token Budget

Before reading:

```bash
cans budget read "sessions" --json
# Returns scored plan. Read ONLY listed files, in order.
```

Before writing:

```bash
cans budget write "sessions" --json
# Returns canEdit[] and mustNotEdit[].
# ONLY edit files in canEdit. Never touch mustNotEdit.
```

Example budget read output:

```json
{
  "plan": [
    { "file": "02-authentication.md", "anchor": "Sessions", "score": 100 },
    { "file": "04-api.md", "anchor": null, "score": 60 }
  ],
  "skipped": ["00-overview.md", "03-data.md"],
  "totalTokens": 180,
  "budgetLimit": 4096
}
```

Read files in plan order. Stop at budgetLimit.

## Machine Output

Use `--json` for structured output. Parse JSON, never human text.

```bash
cans check --json          # structured issues
cans status --json         # workspace state
cans done add-dark-mode --json  # gate results
```

All JSON has `ok: boolean` and `exitCode: number` at top level.

## Quality Checklist

Before finalizing any change:

- [ ] Is each concept in exactly one place?
- [ ] Does indentation add meaning? (No filler levels)
- [ ] Can anything be simpler?
- [ ] Did I duplicate an authoritative rule? (Use `see:` instead)
- [ ] Did I invent anything? (Mark TBD if unsure)
- [ ] Is ambiguity visible? (Don't hide unknowns)
- [ ] Did I create a deep hop? (FORBIDDEN: A→B→C)
- [ ] Did I preserve real identifiers exactly?
- [ ] Did I run `cans check` after editing?
