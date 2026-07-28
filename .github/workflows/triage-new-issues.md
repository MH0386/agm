---
description: Triage newly opened issues — apply type/priority labels, detect duplicates, ask clarifying questions, and assign to the right team member.
engine:
  id: pi
  extensions:
    - "@pi/web-search"
    - "@pi/file-browser"
on:
  issues:
    types: [opened]
permissions:
  contents: read
  issues: read
  pull-requests: read
tools:
  cli-proxy: true
  github:
    mode: gh-proxy
    toolsets: [default]
safe-outputs:
  add-labels:
    max: 3
  add-comment:
    max: 1
  assign-to-user:
    max: 1
---

# Triage New Issues

## Task

When a new issue is opened in this repository, triage it in a single pass:

1. **Read the issue** — title, body, and any existing labels. Ignore bot accounts and issues already labeled `duplicate` or `needs-info`.
2. **Detect duplicates** — search recent and open issues with `gh issue list --state all --search "<key terms>" --limit 20`. If a clear duplicate exists, apply the `duplicate` label and post a short comment linking to the canonical issue (`#<number>`). Do not close; a human will close.
3. **Classify type** — pick exactly one: `bug`, `enhancement`, `documentation`, or `question`. Default to `question` when ambiguous.
4. **Assign priority** — pick exactly one: `priority: high`, `priority: medium`, or `priority: low`. Default to `priority: low`. Escalate to `high` only for data loss, security, or broken core flows.
5. **Check clarity** — if the body is under ~30 words, missing repro steps for a bug, or has no concrete request, post **one** focused clarifying question via `add-comment` and apply `needs-info`. Otherwise skip the comment.
6. **Assign owner** — pick the single best assignee from the repo's recent issue assignees and active commenters (`gh issue list --state all --json assignees,number --limit 50`). If no one is a clear fit, leave the issue unassigned and mention the suggested owner in the triage comment.

## Safe Outputs

- Apply labels via `add-labels` (type + priority, plus `duplicate` or `needs-info` when applicable).
- Post a clarifying question or duplicate link via `add-comment` — at most once.
- Assign at most one user via `assign-to-user`. Never assign bots.
- Call `noop` with a one-line explanation when the issue is already triaged, was opened by a bot, or no action is warranted.
