//! CLI argument parsing and command dispatch.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use askama::Template;
use clap::{Parser, Subcommand, ValueEnum};

use crate::cache::{Cache, CachedSessionMeta};
use crate::render::markdown_export::DetailLevel;
use crate::render::{IndexProjectData, ProjectSessionData};

/// weavr — Claude Code transcript exporter.
#[derive(Parser)]
#[command(name = "weavr", version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Input JSONL file path (shorthand for `export <INPUT>`).
    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// Projects directory (default: ~/.claude/projects/).
    #[arg(long)]
    pub projects_dir: Option<PathBuf>,

    /// Export all projects (default when no INPUT_PATH is given).
    #[arg(long)]
    pub all_projects: bool,

    /// Skip per-session HTML files; only produce combined + index pages.
    #[arg(long)]
    pub no_individual_sessions: bool,

    /// Disable SQLite cache reads and writes.
    #[arg(long)]
    pub no_cache: bool,

    /// Clear the SQLite cache and rebuild from scratch.
    #[arg(long)]
    pub clear_cache: bool,

    /// Filter to a single session by ID or unique prefix.
    #[arg(long)]
    pub session_id: Option<String>,

    /// Export only the project whose name contains this string (case-insensitive).
    #[arg(long)]
    pub project: Option<String>,

    /// Wipe the output directory before writing.
    #[arg(long)]
    pub clear_output: bool,

    /// Output directory (default: ./weavr-out/).
    #[arg(long)]
    pub output_dir: Option<PathBuf>,

    /// Filter sessions starting on or after this date.
    /// Accepts: today, yesterday, last week, last month, YYYY-MM-DD.
    #[arg(long, global = true)]
    pub from_date: Option<String>,

    /// Filter sessions ending on or before this date.
    /// Accepts: today, yesterday, last week, last month, YYYY-MM-DD.
    #[arg(long, global = true)]
    pub to_date: Option<String>,

    /// Split long sessions across multiple HTML pages (messages per page).
    #[arg(long, global = true)]
    pub page_size: Option<usize>,

    /// Open the exported file in the default browser (single-session mode).
    #[arg(long, global = true)]
    pub open_browser: bool,

    /// Enable verbose debug logging via tracing.
    #[arg(long, global = true)]
    pub debug: bool,
}

#[derive(Subcommand)]
pub enum Command {
    /// Emit a stub transcript HTML file for design verification.
    #[command(hide = true)]
    Stub {
        /// Output file path (default: weavr-stub.html in current dir).
        #[arg(short, long, default_value = "weavr-stub.html")]
        output: String,
    },

    /// Export a JSONL session file to HTML or Markdown.
    Export {
        /// Input JSONL file path.
        input: PathBuf,

        /// Output file path (default: <input_stem>.html or .md).
        #[arg(short, long)]
        output: Option<String>,

        /// Output format.
        #[arg(long, value_enum, default_value_t = Format::Html)]
        format: Format,

        /// Detail level for markdown output.
        #[arg(long, value_enum, default_value_t = DetailLevel::Full)]
        detail: DetailLevel,

        /// Compact mode: strip timestamps and horizontal rules (markdown only).
        #[arg(long, default_value_t = false)]
        compact: bool,

        /// Open the output file in the default browser.
        #[arg(long)]
        open_browser: bool,
    },

    /// Update weavr to the latest version from GitHub Releases.
    SelfUpdate,
}

/// Output format for the export command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
#[clap(rename_all = "lowercase")]
pub enum Format {
    /// Self-contained HTML (default).
    Html,
    /// Markdown.
    #[clap(name = "md")]
    #[clap(alias = "markdown")]
    Markdown,
}

impl Cli {
    pub fn run(self) -> anyhow::Result<()> {
        // --debug enables tracing.
        if self.debug {
            tracing_subscriber::fmt()
                .with_env_filter(
                    tracing_subscriber::EnvFilter::try_from_default_env()
                        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug")),
                )
                .init();
        }

        // Passive update notice (non-blocking, throttled).
        crate::update::maybe_print_update_notice();

        match self.command {
            Some(Command::SelfUpdate) => crate::update::self_update().map(|msg| {
                println!("{msg}");
            }),
            Some(Command::Stub { output }) => run_stub(&output),
            Some(Command::Export {
                input,
                output,
                format,
                detail,
                compact,
                open_browser,
            }) => run_export(ExportConfig {
                input: &input,
                output: output.as_deref(),
                format,
                detail,
                compact,
                open_browser,
                from_date: self.from_date.as_deref(),
                to_date: self.to_date.as_deref(),
                page_size: self.page_size,
            }),
            None => {
                // If --input is provided, treat as single export.
                if let Some(ref input) = self.input {
                    // If --output-dir is given, place the output file there.
                    let output_in_dir: Option<String> = self.output_dir.as_ref().and_then(|dir| {
                        let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("session");
                        dir.join(format!("{stem}.html")).to_str().map(|s| s.to_string())
                    });
                    run_export(ExportConfig {
                        input,
                        output: output_in_dir.as_deref(),
                        format: Format::Html,
                        detail: DetailLevel::Full,
                        compact: false,
                        open_browser: self.open_browser,
                        from_date: self.from_date.as_deref(),
                        to_date: self.to_date.as_deref(),
                        page_size: self.page_size,
                    })
                } else {
                    // Default: all-projects export.
                    run_all_projects(self)
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Single-session export
// ---------------------------------------------------------------------------

fn run_stub(output: &str) -> anyhow::Result<()> {
    let ctx = crate::render::html::stub_context();
    let html = ctx.render()?;
    std::fs::write(output, &html)?;
    println!("Wrote stub HTML to {output}");
    println!("  Size: {} bytes", html.len());
    let self_contained = !html.contains("http://") && !html.contains("https://");
    println!("  Self-contained: {}", if self_contained { "yes" } else { "NO" });
    Ok(())
}

struct ExportConfig<'a> {
    input: &'a Path,
    output: Option<&'a str>,
    format: Format,
    detail: DetailLevel,
    compact: bool,
    open_browser: bool,
    from_date: Option<&'a str>,
    to_date: Option<&'a str>,
    page_size: Option<usize>,
}

fn run_export(cfg: ExportConfig<'_>) -> anyhow::Result<()> {
    let t0 = Instant::now();
    let result = crate::parser::parse_file(cfg.input)?;
    if !result.warnings.is_empty() {
        for w in &result.warnings {
            eprintln!("Warning: line {}: {}", w.line, w.message);
        }
    }

    let session = crate::session::build_session(&result.entries);
    if session.messages.is_empty() {
        anyhow::bail!("No messages found in input file");
    }

    let agg = crate::aggregate::aggregate(&session);

    // Date filter: skip if session doesn't match the date range.
    if !session_matches_date_range(&agg, cfg.from_date, cfg.to_date)? {
        anyhow::bail!(
            "Session does not match the requested date range (from={:?}, to={:?})",
            cfg.from_date,
            cfg.to_date
        );
    }

    match cfg.format {
        Format::Html => {
            let css = crate::assets::CSS.to_string();
            let base_stem = match cfg.output {
                Some(o) => {
                    if let Some(stem) = o.strip_suffix(".html") {
                        stem.to_string()
                    } else {
                        o.to_string()
                    }
                }
                None => {
                    cfg.input.file_stem().and_then(|s| s.to_str()).unwrap_or("session").to_string()
                }
            };

            if let Some(ps) = cfg.page_size {
                if let Some(pages) = crate::render::pagination::paginate(&session, ps) {
                    let total = pages.len();
                    for page in &pages {
                        let filename = crate::render::pagination::page_filename(&base_stem, page);
                        let ctx = crate::render::html::build_context_paginated(
                            &session,
                            &agg,
                            css.clone(),
                            page,
                            None,
                        );
                        let html = ctx.render()?;
                        std::fs::write(&filename, &html)?;
                        println!(
                            "  {} page {}/{} {} {} messages",
                            green("✓"),
                            page.number,
                            total,
                            dim("·"),
                            page.message_range.len()
                        );
                    }
                    println!();
                    println!("  {}  HTML (paginated, {total} pages)", dim("Format:  "));
                    println!("  {}  {}", dim("Messages:"), agg.message_count);
                    println!(
                        "  {}  {} in / {} out",
                        dim("Tokens:  "),
                        fmt_tokens(agg.total_input_tokens),
                        fmt_tokens(agg.total_output_tokens)
                    );
                    println!("  {}  {}", dim("Took:    "), cyan(&fmt_elapsed(t0.elapsed())));
                    return Ok(());
                }
            }

            // Non-paginated path.
            let ctx = crate::render::html::build_context(&session, &agg, css, None);
            let html = ctx.render()?;

            let output_path = match cfg.output {
                Some(o) => o.to_string(),
                None => format!("{}.html", base_stem),
            };

            std::fs::write(&output_path, &html)?;
            println!("{} {} {}", bold("Exported"), dim("→"), bold(&output_path));
            println!();
            println!("  {}  HTML", dim("Format:  "));
            println!("  {}  {}", dim("Messages:"), agg.message_count);
            println!(
                "  {}  {} in / {} out",
                dim("Tokens:  "),
                fmt_tokens(agg.total_input_tokens),
                fmt_tokens(agg.total_output_tokens)
            );
            println!("  {}  {}", dim("Took:    "), cyan(&fmt_elapsed(t0.elapsed())));

            if cfg.open_browser {
                let _ = std::process::Command::new("open").arg(&output_path).spawn();
            }
        }

        Format::Markdown => {
            let md = crate::render::markdown_export::render_session(
                &session,
                &agg,
                cfg.detail,
                cfg.compact,
            );

            let output_path = match cfg.output {
                Some(o) => o.to_string(),
                None => {
                    let stem = cfg.input.file_stem().and_then(|s| s.to_str()).unwrap_or("session");
                    format!("{}.md", stem)
                }
            };

            std::fs::write(&output_path, &md)?;
            println!("{} {} {}", bold("Exported"), dim("→"), bold(&output_path));
            println!();
            println!("  {}  Markdown", dim("Format:  "));
            println!("  {}  {:?}", dim("Detail:  "), cfg.detail);
            println!("  {}  {}", dim("Compact: "), if cfg.compact { "yes" } else { "no" });
            println!("  {}  {}", dim("Messages:"), agg.message_count);
            println!(
                "  {}  {} in / {} out",
                dim("Tokens:  "),
                fmt_tokens(agg.total_input_tokens),
                fmt_tokens(agg.total_output_tokens)
            );
            println!("  {}  {}", dim("Took:    "), cyan(&fmt_elapsed(t0.elapsed())));
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// All-projects pipeline
// ---------------------------------------------------------------------------

fn run_all_projects(cli: Cli) -> anyhow::Result<()> {
    let projects_dir = cli.projects_dir.unwrap_or_else(crate::project::default_projects_dir);

    if !projects_dir.is_dir() {
        anyhow::bail!(
            "Projects directory not found: {}. Use --projects-dir to specify a path.",
            projects_dir.display()
        );
    }

    let output_dir = cli.output_dir.unwrap_or_else(|| PathBuf::from("weavr-out"));

    if cli.clear_output && output_dir.exists() {
        fs::remove_dir_all(&output_dir)?;
    }
    fs::create_dir_all(&output_dir)?;

    // Cache.
    let cache_path = projects_dir.join("weavr-cache.db");
    if cli.clear_cache && cache_path.exists() {
        let _ = fs::remove_file(&cache_path);
    }

    let use_cache = !cli.no_cache;
    let cache: Option<Cache> = if use_cache { Cache::open(&cache_path).ok() } else { None };

    // Parse date filters once.
    let from_dt =
        if let Some(ref s) = cli.from_date { Some(crate::dates::parse_date(s)?) } else { None };
    let to_dt =
        if let Some(ref s) = cli.to_date { Some(crate::dates::parse_date(s)?) } else { None };

    // Discover projects.
    let mut projects = crate::project::discover_projects(&projects_dir);

    // Filter by --session-id if provided.
    if let Some(ref sid) = cli.session_id {
        projects = filter_by_session_id(projects, sid)?;
    }

    // Filter by --project if provided.
    if let Some(ref name) = cli.project {
        projects = filter_by_project_name(projects, name)?;
    }

    if projects.is_empty() {
        println!("No sessions found in {}", projects_dir.display());
        return Ok(());
    }

    let t0 = Instant::now();
    let total_sessions: usize = projects.iter().map(|p| p.sessions.len()).sum();
    println!(
        "{} {} {} {} {} {} {}",
        bold("Exporting"),
        cyan(&total_sessions.to_string()),
        bold(&format!("session{}", if total_sessions == 1 { "" } else { "s" })),
        dim("across"),
        cyan(&projects.len().to_string()),
        bold(&format!("project{}", if projects.len() == 1 { "" } else { "s" })),
        dim(&format!("→ {}", output_dir.display()))
    );
    println!();

    let css = crate::assets::CSS.to_string();
    let mut all_project_data: Vec<IndexProjectData> = Vec::new();
    let mut global_messages: u32 = 0;
    let mut global_tokens: u64 = 0;
    let mut global_input_tokens: u64 = 0;
    let mut global_output_tokens: u64 = 0;
    let mut global_cache_creation_tokens: u64 = 0;
    let mut global_cache_read_tokens: u64 = 0;
    let mut global_earliest: Option<String> = None;
    let mut global_latest: Option<String> = None;
    let mut total_exported: usize = 0;

    for project in &projects {
        let project_out = output_dir.join(&project.name);
        fs::create_dir_all(&project_out)?;

        let short_name = project_name_short(&project.name, &project.path);
        let display_name = project.display_name().to_string();

        let mut session_datas: Vec<ProjectSessionData> = Vec::new();
        let mut project_messages: u32 = 0;
        let mut project_tokens: u64 = 0;
        let mut project_input_tokens: u64 = 0;
        let mut project_output_tokens: u64 = 0;
        let mut project_cache_creation_tokens: u64 = 0;
        let mut project_cache_read_tokens: u64 = 0;
        let mut project_last_activity: Option<String> = None;

        for sf in &project.sessions {
            // Check cache first.
            let file_meta = fs::metadata(&sf.path).ok();
            let file_mtime = file_meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let file_size = file_meta.map(|m| m.len()).unwrap_or(0);

            let cached: Option<CachedSessionMeta> =
                cache.as_ref().and_then(|c| c.get(&sf.id, file_mtime));

            let (
                title,
                msg_count,
                input_tok,
                output_tok,
                cache_create,
                cache_read,
                first_ts,
                last_ts,
                first_user_prompt,
            ) = if let Some(ref cm) = cached {
                // Use cached metadata.
                (
                    cm.title.clone(),
                    cm.message_count,
                    cm.total_input_tokens,
                    cm.total_output_tokens,
                    cm.total_cache_creation_tokens,
                    cm.total_cache_read_tokens,
                    cm.first_timestamp.clone(),
                    cm.last_timestamp.clone(),
                    cm.first_user_prompt.clone(),
                )
            } else {
                // Parse and aggregate.
                let parse_result = match crate::parser::parse_file(&sf.path) {
                    Ok(r) => r,
                    Err(e) => {
                        eprintln!("Warning: skipping {}: {}", sf.id, e);
                        continue;
                    }
                };
                let session = crate::session::build_session(&parse_result.entries);
                let agg = crate::aggregate::aggregate(&session);

                let title = agg.summaries.first().cloned();
                let msg_count = agg.message_count as u32;
                let it = agg.total_input_tokens;
                let ot = agg.total_output_tokens;
                let cc = agg.total_cache_creation_tokens;
                let cr = agg.total_cache_read_tokens;
                let first = agg.first_timestamp.map(|t| t.to_rfc3339());
                let last = agg.last_timestamp.map(|t| t.to_rfc3339());
                let fup = crate::render::html::find_first_user_prompt(&session);

                // Store in cache.
                if let Some(ref c) = cache {
                    c.put(
                        &CachedSessionMeta {
                            session_id: sf.id.clone(),
                            project_name: project.name.clone(),
                            title: title.clone(),
                            first_timestamp: first.clone(),
                            last_timestamp: last.clone(),
                            message_count: msg_count,
                            total_input_tokens: it,
                            total_output_tokens: ot,
                            total_cache_creation_tokens: cc,
                            total_cache_read_tokens: cr,
                            first_user_prompt: fup.clone(),
                        },
                        file_mtime,
                        file_size,
                    );
                }

                (title, msg_count, it, ot, cc, cr, first, last, fup)
            };

            // Date filter.
            if !cached_timestamps_match_date_range(
                first_ts.as_deref(),
                last_ts.as_deref(),
                from_dt,
                to_dt,
            ) {
                continue;
            }

            let total_session_tokens = input_tok + output_tok + cache_create + cache_read;

            // Export per-session HTML if not skipped.
            if !cli.no_individual_sessions {
                // Only re-parse if we used cache metadata.
                let session_html_path = project_out.join(format!("{}.html", sf.id));
                if !session_html_path.exists() {
                    let parse_result = crate::parser::parse_file(&sf.path)?;
                    let session = crate::session::build_session(&parse_result.entries);
                    let agg = crate::aggregate::aggregate(&session);

                    if let Some(ps) = cli.page_size {
                        if let Some(pages) = crate::render::pagination::paginate(&session, ps) {
                            for page in &pages {
                                let filename =
                                    crate::render::pagination::page_filename(&sf.id, page);
                                let page_path = project_out.join(&filename);
                                let ctx = crate::render::html::build_context_paginated(
                                    &session,
                                    &agg,
                                    css.clone(),
                                    page,
                                    Some(&project.name),
                                );
                                let html = ctx.render()?;
                                fs::write(&page_path, &html)?;
                            }
                        }
                    } else {
                        let ctx = crate::render::html::build_context(
                            &session,
                            &agg,
                            css.clone(),
                            Some(&project.name),
                        );
                        let html = ctx.render()?;
                        fs::write(&session_html_path, &html)?;
                    }
                }
            }

            total_exported += 1;
            session_datas.push(ProjectSessionData {
                id: sf.id.clone(),
                title,
                message_count: msg_count,
                total_tokens: total_session_tokens,
                first_user_prompt,
                started_at: first_ts.clone(),
            });

            project_messages += msg_count;
            project_tokens += total_session_tokens;
            project_input_tokens += input_tok;
            project_output_tokens += output_tok;
            project_cache_creation_tokens += cache_create;
            project_cache_read_tokens += cache_read;

            // Track per-project last activity.
            if let Some(ref ts) = last_ts {
                match project_last_activity.as_deref() {
                    None => project_last_activity = Some(ts.clone()),
                    Some(cur) if ts.as_str() > cur => project_last_activity = Some(ts.clone()),
                    _ => {}
                }
            }

            // Track global time range.
            if let Some(ref ts) = first_ts {
                if global_earliest.is_none()
                    || ts.as_str() < global_earliest.as_deref().unwrap_or("")
                {
                    global_earliest = Some(ts.clone());
                }
            }
            if let Some(ref ts) = last_ts {
                if global_latest.is_none() || ts.as_str() > global_latest.as_deref().unwrap_or("") {
                    global_latest = Some(ts.clone());
                }
            }
        }

        if session_datas.is_empty() {
            continue;
        }

        // Per-project combined page.
        let n = session_datas.len();
        let project_ctx = crate::render::project::build_context(
            css.clone(),
            project.name.clone(),
            display_name.clone(),
            session_datas,
        );
        let project_html = project_ctx.render()?;
        let combined_path = project_out.join("combined_transcripts.html");
        fs::write(&combined_path, &project_html)?;
        println!(
            "  {} {}  {}",
            green("✓"),
            bold(&display_name),
            dim(&format!("{} session{}", n, if n == 1 { "" } else { "s" }))
        );

        all_project_data.push(IndexProjectData {
            name: project.name.clone(),
            session_count: project.sessions.len().try_into().unwrap_or(u32::MAX),
            message_count: project_messages,
            total_tokens: project_tokens,
            short_name,
            display_name,
            last_activity: project_last_activity,
        });

        global_messages += project_messages;
        global_tokens += project_tokens;
        global_input_tokens += project_input_tokens;
        global_output_tokens += project_output_tokens;
        global_cache_creation_tokens += project_cache_creation_tokens;
        global_cache_read_tokens += project_cache_read_tokens;
    }

    // Master index.
    let index_ctx = crate::render::index::build_context(
        css,
        all_project_data,
        global_messages,
        crate::render::index::TokenTotals {
            total: global_tokens,
            input: global_input_tokens,
            output: global_output_tokens,
            cache_creation: global_cache_creation_tokens,
            cache_read: global_cache_read_tokens,
        },
        global_earliest,
        global_latest,
    );
    let index_html = index_ctx.render()?;
    fs::write(output_dir.join("index.html"), &index_html)?;
    println!("  {} {}", green("✓"), dim("index.html"));
    println!();

    let mode = if cli.no_individual_sessions { "combined + index" } else { "full" };
    println!(
        "{} {} session{}",
        bold(&green("Done:")),
        cyan(&total_exported.to_string()),
        if total_exported == 1 { "" } else { "s" },
    );
    println!();
    println!("  {}  HTML ({mode})", dim("Format:  "));
    println!(
        "  {}  {}",
        dim("Sessions:"),
        fmt_num(u64::try_from(total_exported).unwrap_or(u64::MAX))
    );
    println!("  {}  {}", dim("Messages:"), fmt_num(u64::from(global_messages)));
    println!(
        "  {}  {} ({} in + {} out + {} cache read + {} cache write)",
        dim("Tokens:  "),
        fmt_tokens(global_tokens),
        fmt_tokens(global_input_tokens),
        fmt_tokens(global_output_tokens),
        fmt_tokens(global_cache_read_tokens),
        fmt_tokens(global_cache_creation_tokens)
    );
    println!("  {}  {}", dim("Took:    "), cyan(&fmt_elapsed(t0.elapsed())));

    if cli.open_browser {
        let index_path = output_dir.join("index.html");
        let _ = std::process::Command::new("open").arg(&index_path).spawn();
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Filter to projects whose name or display name contains `query` (case-insensitive).
fn filter_by_project_name(
    projects: Vec<crate::project::Project>,
    query: &str,
) -> anyhow::Result<Vec<crate::project::Project>> {
    let q = query.to_lowercase();
    let matching: Vec<crate::project::Project> = projects
        .into_iter()
        .filter(|p| {
            p.name.to_lowercase().contains(&q) || p.display_name().to_lowercase().contains(&q)
        })
        .collect();

    if matching.is_empty() {
        anyhow::bail!("No project found matching \"{}\"", query);
    }

    Ok(matching)
}

/// Filter projects to only include the session matching `prefix`.
fn filter_by_session_id(
    projects: Vec<crate::project::Project>,
    prefix: &str,
) -> anyhow::Result<Vec<crate::project::Project>> {
    let mut matching: Vec<(&crate::project::Project, &crate::project::SessionFile)> = Vec::new();

    for p in &projects {
        for s in &p.sessions {
            if s.id.starts_with(prefix) {
                matching.push((p, s));
            }
        }
    }

    if matching.is_empty() {
        eprintln!("No session matches prefix \"{}\"", prefix);
        return Ok(Vec::new());
    }

    if matching.len() > 1 {
        eprintln!("Ambiguous prefix \"{}\" matches {} sessions:", prefix, matching.len());
        for (proj, sess) in &matching {
            eprintln!("  {}/{}", proj.name, sess.id);
        }
        anyhow::bail!("Ambiguous session-id prefix. Use a longer prefix to disambiguate.");
    }

    let (project, session) = matching.into_iter().next().unwrap();
    Ok(vec![crate::project::Project {
        name: project.name.clone(),
        path: project.path.clone(),
        sessions: vec![session.clone()],
    }])
}

/// Check whether a session (with known aggregate timestamps) matches the
/// requested date range.
fn session_matches_date_range(
    agg: &crate::aggregate::SessionAggregate,
    from_date: Option<&str>,
    to_date: Option<&str>,
) -> anyhow::Result<bool> {
    let from_dt = if let Some(s) = from_date { Some(crate::dates::parse_date(s)?) } else { None };
    let to_dt = if let Some(s) = to_date { Some(crate::dates::parse_date(s)?) } else { None };

    let first = agg.first_timestamp;
    let last = agg.last_timestamp;

    Ok(match (from_dt, to_dt, first, last) {
        // No filter → always match.
        (None, None, _, _) => true,
        // Only from-date: session must end on or after it.
        (Some(f), None, _, Some(l)) => l >= f,
        // Only to-date: session must start on or before it (end of that day).
        (None, Some(t), Some(f), _) => {
            let to_end = t.end_day();
            f <= to_end
        }
        // Both: session's range must overlap with [from, to_end].
        (Some(f), Some(t), Some(first_ts), Some(last_ts)) => {
            let to_end = t.end_day();
            last_ts >= f && first_ts <= to_end
        }
        // If we don't have timestamps, include the session (can't filter).
        _ => true,
    })
}

/// Check whether a cached session (with string timestamps) matches the
/// requested date range.
fn cached_timestamps_match_date_range(
    first_ts: Option<&str>,
    last_ts: Option<&str>,
    from_dt: Option<chrono::DateTime<chrono::Utc>>,
    to_dt: Option<chrono::DateTime<chrono::Utc>>,
) -> bool {
    match (from_dt, to_dt, first_ts, last_ts) {
        (None, None, _, _) => true,
        (Some(f), None, _, Some(l)) => {
            if let Ok(last_dt) = chrono::DateTime::parse_from_rfc3339(l) {
                last_dt >= f
            } else {
                true
            }
        }
        (None, Some(t), Some(f), _) => {
            if let Ok(first_dt) = chrono::DateTime::parse_from_rfc3339(f) {
                let to_end = t.end_day();
                first_dt <= to_end
            } else {
                true
            }
        }
        (Some(f), Some(t), Some(first_s), Some(last_s)) => {
            if let (Ok(first_dt), Ok(last_dt)) = (
                chrono::DateTime::parse_from_rfc3339(first_s),
                chrono::DateTime::parse_from_rfc3339(last_s),
            ) {
                let to_end = t.end_day();
                last_dt >= f && first_dt <= to_end
            } else {
                true
            }
        }
        _ => true,
    }
}

/// Extract a human-readable short name from a project path.
///
/// Uses the directory name (last non-empty path segment) as the short name,
/// which is what `discover_projects` already uses as `project.name`.
fn project_name_short(name: &str, _path: &Path) -> String {
    // `name` is already the final path segment from `discover_projects`.
    name.to_string()
}

// ---------------------------------------------------------------------------
// Output helpers
// ---------------------------------------------------------------------------

fn is_tty() -> bool {
    use std::io::IsTerminal;
    std::io::stdout().is_terminal()
}

fn ansi(code: &str, s: &str) -> String {
    if is_tty() {
        format!("\x1b[{code}m{s}\x1b[0m")
    } else {
        s.to_string()
    }
}

fn bold(s: &str) -> String {
    ansi("1", s)
}
fn dim(s: &str) -> String {
    ansi("2", s)
}
fn green(s: &str) -> String {
    ansi("32", s)
}
fn cyan(s: &str) -> String {
    ansi("36", s)
}

fn fmt_tokens(n: u64) -> String {
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        let m = n as f64 / 1_000_000.0;
        if m >= 10.0 {
            format!("{m:.1}M")
        } else {
            format!("{m:.2}M")
        }
    } else {
        fmt_num(n)
    }
}

fn fmt_num(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

fn fmt_elapsed(d: std::time::Duration) -> String {
    let ms = d.as_millis();
    if ms < 1_000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.2}s", d.as_secs_f64())
    } else {
        let secs = d.as_secs();
        format!("{}m {}s", secs / 60, secs % 60)
    }
}

/// Extension trait to get end-of-day for a DateTime<Utc>.
trait EndOfDay {
    fn end_day(self) -> chrono::DateTime<chrono::Utc>;
}

impl EndOfDay for chrono::DateTime<chrono::Utc> {
    fn end_day(self) -> chrono::DateTime<chrono::Utc> {
        self.date_naive().and_hms_opt(23, 59, 59).unwrap().and_local_timezone(chrono::Utc).unwrap()
    }
}
