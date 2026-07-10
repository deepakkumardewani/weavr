# weavr

[![CI](https://github.com/deepakkumardewani/weavr/actions/workflows/ci.yml/badge.svg)](https://github.com/deepakkumardewani/weavr/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/weavr.svg)](https://crates.io/crates/weavr)
[![Crates.io Downloads](https://img.shields.io/crates/d/weavr.svg)](https://crates.io/crates/weavr)
[![Rust](https://img.shields.io/badge/rust-1.80%2B-orange.svg)](https://www.rust-lang.org)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)
[![100% local](https://img.shields.io/badge/100%25%20local-no%20AI-brightgreen.svg)]()

**Demo:** [https://wea-vr.vercel.app/](https://wea-vr.vercel.app/)

A fast, self-contained Rust CLI that converts Claude Code transcript JSONL files into beautiful HTML and Markdown.

`weavr` is a Rust reimplementation of [claude-code-log](https://github.com/daaain/claude-code-log), focused on speed, zero-dependency output artefacts, and a clean Material 3 design.

[![weavr in action](demo.gif)](demo.gif)

## Navigation

- [Why weavr?](#why-weavr)
- [How it works](#how-it-works)
- [Quickstart](#quickstart)
- [Two modes](#two-modes)
- [Key Features](#key-features)
- [Installation](#installation)
- [Usage](#usage)
  - [Basic export](#basic-export)
  - [Markdown export](#markdown-export)
  - [All projects (batch)](#all-projects-batch)
  - [Filtering](#filtering)
  - [Pagination](#pagination)
  - [Cache & output control](#cache--output-control)
- [Output Formats](#output-formats)
- [Performance](#performance)
- [Roadmap](#roadmap)
- [License](#license)

## Why weavr?

[claude-code-log](https://github.com/daaain/claude-code-log) proved how useful it is to turn raw Claude Code transcripts into readable artefacts. weavr is a ground-up Rust rewrite around two goals:

- **Speed.** Single-pass JSONL parser, in-memory session DAG, and SQLite cache for incremental rebuilds. **18–46× faster** than the Python tool (see [Performance](#performance)).
- **Design.** Material 3 light/dark theme, flat dot-timeline, rich inline tool rendering (Bash IN/OUT, diffs, modals for Skill/Agent). Every HTML file is **fully self-contained** — fonts, CSS, JS all embedded. A CI gate rejects any external URL.

Single static binary via `brew`, `cargo binstall`, or `cargo install`, with built-in `self-update`. No Python runtime needed.

### weavr vs. claude-code-log

|                                               | **weavr** (Rust)                               | **claude-code-log** (Python) |
| --------------------------------------------- | ---------------------------------------------- | ---------------------------- |
| Speed (all projects)                          | **~1.3 s**                                     | ~24 s                        |
| Speed (single session)                        | **~28 ms**                                     | ~1.3 s                       |
| Distribution                                  | single static binary (brew / binstall / cargo) | `uvx` / `pip` (needs Python) |
| Self-contained output                         | ✅ zero external URLs (CI-enforced)            | partial                      |
| Incremental rebuilds                          | ✅ SQLite cache                                | ⚠️ re-renders each run       |
| Theme                                         | Material 3 light/dark, dot-timeline            | minimalist HTML              |
| HTML export                                   | ✅                                             | ✅                           |
| Markdown export + detail levels + `--compact` | ✅                                             | ✅                           |
| Token usage tracking                          | ✅                                             | ✅                           |
| Date-range filtering (natural language)       | ✅                                             | ✅                           |
| Client-side filter chips + in-page search     | ✅                                             | ✅ (runtime filtering)       |
| Multi-project hierarchy + master index        | ✅                                             | ✅                           |
| Self-update command                           | ✅                                             | —                            |

Both tools share the same JSONL input format. If you live in the terminal and want an interactive TUI today, claude-code-log is great. If you want the fastest possible export and offline-portable HTML, reach for weavr.

## How it works

```
 ~/.claude/projects/
   ├── my-project/session.jsonl          (Claude Code transcripts)
   └── other-app/session.jsonl
        │
        ▼
   ┌─────────────────────────────────────────────┐
   │  weavr                                      │
   │  ─────────────────────────────────────────  │
   │  JSONL parser → session DAG → SQLite cache  │
   │                     │                       │
   │              HTML renderer                  │
   │        (Material 3 · dot-timeline ·         │
   │         tool cards · diffs · search)        │
   └─────────────────────────────────────────────┘
        │
        ▼
   weavr-out/
   ├── index.html                        (master index, searchable)
   ├── my-project/combined_transcripts.html
   └── my-project/session.html           (fully self-contained)
```

Every output file is a **single portable HTML** — fonts, CSS, JS all embedded. No server, no CDN, no runtime. No AI involved — your transcripts never leave your machine.

## Quickstart

**1. Install**

```sh
brew install deepakkumardewani/weavr/weavr
# or: cargo binstall weavr
# or: cargo install weavr
```

**2. Export a session**

```sh
weavr -i ~/.claude/projects/my-project/session.jsonl
# → writes session.html
```

**3. Export everything**

```sh
weavr --all-projects --open-browser
# → walks ~/.claude/projects/, generates weavr-out/, opens index.html
```

## Two modes

weavr has two distinct operating modes:

| Mode            | Command                          | Produces                                  |
| --------------- | -------------------------------- | ----------------------------------------- |
| **Batch**       | `weavr` / `weavr --all-projects` | HTML index + per-session + combined pages |
| **Single-file** | `weavr export <INPUT>`           | one file (HTML or Markdown)               |

**Batch mode** walks `~/.claude/projects/` and generates a complete static site. It's HTML-only — no `--format` / `--detail` / `--compact` flags here.

**Single-file mode** (`weavr export`) gives you full control over format, detail level, and compactness. All format-related flags live on the `export` subcommand:

```sh
weavr export --help   # see all export options
```

The `-i` shorthand is a convenience alias for `export <INPUT>` (HTML only, full detail).

## Key Features

- **Self-Contained HTML** — fonts, CSS tokens, assets all embedded inline; CI-enforced
- **Light/Dark Themes** — Material 3 warm-neutral tokens; dark by default, toggle persisted to localStorage
- **Markdown Export** — `--format md` with five `--detail` levels and `--compact` mode
- **Flat Dot-Timeline** — chronological event stream: user messages, assistant text, thinking, tool calls
- **Rich Tool Rendering** — Bash IN/OUT, Read/Edit/Write diffs with intra-line highlights, Skill/Agent modals
- **Token Usage Tracking** — per-message and per-session input/output/cache tokens
- **Multi-Project Export** — `--all-projects` generates a navigable static site with master index
- **SQLite Cache** — fast incremental rebuilds; `--clear-cache` / `--no-cache` for control
- **Interactive HTML** — filter chips, in-page search (150ms debounce), theme toggle, project search, card/list view
- **Zero-Config** — sensible defaults; point at a JSONL file and go
- **100% Local** — pure offline tool; no AI used, no data sent anywhere, your transcripts stay on your machine

## Installation

### Via Homebrew (macOS)

```sh
brew install deepakkumardewani/weavr/weavr
```

### Via cargo-binstall

```sh
cargo binstall weavr
```

Fetches the latest prebuilt binary from GitHub Releases. Falls back to building from source if no matching binary is found.

### Via cargo install

```sh
cargo install weavr
```

Builds from source on crates.io. Requires Rust 1.80+.

### Direct download

Prebuilt binaries are on the [GitHub Releases](https://github.com/deepakkumardewani/weavr/releases) page. Download the `.tar.gz` for your platform, extract, and place `weavr` on your `PATH`.

| Platform              | Target triple              |
| --------------------- | -------------------------- |
| macOS (Apple Silicon) | `aarch64-apple-darwin`     |
| Linux (x86_64)        | `x86_64-unknown-linux-gnu` |

> Intel macOS has no prebuilt binary — install via `cargo install weavr`, or run the Apple Silicon build under Rosetta 2.

### From source

```sh
git clone https://github.com/deepakkumardewani/weavr.git
cd weavr
cargo build --release
# binary is at target/release/weavr
```

### Updating

```sh
weavr self-update       # update to the latest GitHub Release
```

A passive notice is printed when a newer version is available (throttled to once per 24 h). Set `WEAVR_NO_UPDATE_CHECK=1` to disable.

Package-manager installs should use their native update commands:

```sh
brew upgrade weavr                  # Homebrew
cargo install --force weavr         # cargo install
cargo binstall --force weavr        # cargo-binstall
```

### Requirements

- Rust 1.80+ (source builds only; prebuilt binaries have no dependencies)

## Usage

### Basic export

```sh
weavr export session.jsonl
# writes session.html
```

```sh
weavr export session.jsonl -o /tmp/review.html --open-browser
```

```sh
weavr -i session.jsonl
# shorthand — equivalent to `export <INPUT>` (HTML only, full detail)
```

```sh
weavr -i session.jsonl --output-dir ~/desktop
# places session.html in ~/desktop/
```

`--open-browser` is a global flag — it works with all modes (`export`, `-i`, and `--all-projects`).

**Example output:**

```
Exported → ~/desktop/session.html

  Format:    HTML
  Messages:  106
  Tokens:    43,950 in / 110,380 out
  Took:      8ms
```

Token counts display with thousands separators and switch to `M` notation above one million (e.g. `1.23M`, `12.5M`).

### Markdown export

```sh
weavr export session.jsonl --format md
# writes session.md
```

**Detail levels:**

```sh
weavr export session.jsonl --format md --detail full       # everything (default)
weavr export session.jsonl --format md --detail high       # messages + tool calls, no thinking
weavr export session.jsonl --format md --detail low        # messages only, no tool calls
weavr export session.jsonl --format md --detail minimal    # user + assistant text only
weavr export session.jsonl --format md --detail user-only  # user prompts only
```

| Level       | Includes                                                                |
| ----------- | ----------------------------------------------------------------------- |
| `user-only` | User prompts only — good input for an agent building a requirements doc |
| `minimal`   | User + assistant text                                                   |
| `low`       | + Key tool signals (WebSearch, WebFetch, Task delegations)              |
| `high`      | + All tool calls; drops thinking blocks and system metadata             |
| `full`      | Everything — thinking, system entries, all tool calls (default)         |

**Feeding a past session to an LLM:**

```sh
weavr export session.jsonl --format md --detail low --compact -o context.md
```

`--compact` strips timestamps, horizontal rules, and merges repeated same-type headings — significantly reducing token count.

### All projects (batch)

```sh
weavr --all-projects
# walks ~/.claude/projects/ and generates weavr-out/
#   index.html
#   <project>/combined_transcripts.html
#   <project>/<session>.html
```

```sh
weavr --all-projects --projects-dir /path/to/projects --output-dir ./out
```

```sh
weavr --all-projects --no-individual-sessions
# skip per-session HTML; only index + combined pages
```

```sh
weavr --all-projects --open-browser
# opens index.html in the default browser when done
```

**Example output:**

```
Exporting 160 sessions across 10 projects → ./weavr-out

  ✓ my-project    49 sessions
  ✓ other-app     13 sessions
  ✓ index.html

Done: 162 sessions

  Format:    HTML (full)
  Messages:  26,289
  Tokens:    17.7M in / 11.1M out
  Took:      673ms
```

### Filtering

<details><summary>Date-range, project, and session filtering</summary>

**By project:**

```sh
weavr --all-projects --project cclog
# exports only the project whose name contains "cclog" (case-insensitive)
```

```sh
weavr --all-projects --project addy --output-dir ./out --open-browser
# partial match — "addy" matches "addyosmaniskills", "addy-osmani", etc.
```

`--project` does a case-insensitive substring match against both the internal directory name and the display name. Multiple projects can match. Exits with an error if nothing matches.

**By date:**

```sh
weavr export session.jsonl --from-date yesterday --to-date today
weavr --all-projects --from-date 2025-06-01 --to-date 2025-06-30
```

Accepts: `today`, `yesterday`, `last week`, `last month`, and ISO dates (`YYYY-MM-DD`). Sessions whose timestamp range doesn't overlap the filter window are skipped.

**By session ID:**

```sh
weavr --all-projects --session-id 6162c547
# exports only the matching session (prefix match)
```

</details>

### Pagination

<details><summary>Split long sessions across multiple HTML files</summary>

```sh
weavr export session.jsonl --page-size 50
# splits into session-page-1.html, session-page-2.html, ...
```

`--page-size N` sets messages per page. The first page includes the full chrome; subsequent pages are content-only.

</details>

### Cache & output control

<details><summary>Cache, output directory, and debug flags</summary>

```sh
weavr --all-projects --no-cache      # skip cache entirely
weavr --all-projects --clear-cache   # drop and rebuild cache
```

```sh
weavr --all-projects --clear-output   # delete output dir before writing
```

```sh
weavr --debug export session.jsonl   # enable tracing output
weavr --debug --all-projects         # verbose logging for all-projects mode
```

</details>

## Output Formats

### HTML

- Warm-neutral light/dark theme with embedded design tokens
- Tool calls rendered as collapsible cards with syntax highlighting
- Unified diff view for `Edit` / `MultiEdit` tool calls
- Token usage displayed per message and as a session total
- Thinking blocks in expandable sections
- 100% self-contained — verified by a CI test that rejects any `http(s)://` URL in the output

### Markdown

- GitHub-Flavored Markdown compatible
- Tool calls collapse to fenced code blocks with the tool name and key parameters
- Diffs rendered as ` ```diff ` blocks with unified `+/-` hunks
- `--compact` and `--detail` work orthogonally

## Performance

Benchmarked with [`hyperfine`](https://github.com/sharkdp/hyperfine) (warmup + multiple runs, no cache) on Apple Silicon against `claude-code-log` over the same input. Re-run any time with `just bench`.

| Mode                               | weavr (Rust) | claude-code-log (Python) | Speedup   |
| ---------------------------------- | ------------ | ------------------------ | --------- |
| All projects (160 sessions, 97 MB) | **1.32 s**   | 24.10 s                  | **18.2×** |
| Single project (42 sessions)       | **466 ms**   | 9.68 s                   | **20.8×** |
| Single session (19 MB, ~500 msgs)  | **27.5 ms**  | 1.28 s                   | **46.5×** |

Full methodology and optimization details in [agent_docs/weavr-bench-results.md](agent_docs/weavr-bench-results.md).

## Roadmap

### Done (v1.0)

- [x] Single-session HTML export with rich tool rendering
- [x] Markdown export with detail levels (`full`/`high`/`low`/`minimal`/`user-only`) + `--compact`
- [x] Project hierarchy + master index + per-project combined pages
- [x] SQLite cache for fast incremental rebuilds
- [x] Material 3 light/dark theme with flat dot-timeline
- [x] Client-side interactivity — filter chips, in-page search, theme toggle, sidebar nav
- [x] Date-range filtering with natural language (`today`, `yesterday`, `last week`)
- [x] Pagination for long sessions
- [x] Self-contained output (zero external URLs, CI-enforced)
- [x] Multi-channel install (brew, binstall, cargo) + `self-update`

### Done (post-v1.0)

- [x] **Per-project export** — `--project <name>` filters batch export to matching projects (case-insensitive partial match)
- [x] **`--output-dir` with `-i`** — single-session shorthand now places output in the specified directory
- [x] **Global `--open-browser`** — works with `export`, `-i`, and `--all-projects` (opens `index.html` in batch mode)
- [x] **Improved CLI output** — colored progress (`✓` per project), elapsed time (`Took:`), `M`-notation for large token counts, aligned stats block consistent across all modes

### Planned

- [ ] **Interactive TUI** — terminal-based transcript browser
- [ ] **Interactive timeline** — zoomable, time-grouped view of message activity
- [ ] **Image rendering** — inline display of images embedded in transcripts
- [ ] **JSON export** — `--format json` for programmatic consumption
- [ ] **Windows support** — currently macOS and Linux only
- [ ] **Cost estimation** — per-session API cost breakdown
- [ ] **Live HTML sharing** — deploy generated output to a hosted URL for one-click viewing and sharing in the browser

## License

MIT
