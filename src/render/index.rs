//! Master index page rendering.
//!
//! Builds an askama context for `templates/index.html` listing all projects
//! as cards with aggregate totals.

use askama::Template;

/// Context for rendering the master `index.html`.
#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexContext {
    pub css: String,
    pub index_js: String,
    pub version: String,
    pub total_projects: usize,
    pub total_sessions: u32,
    pub total_messages: u32,
    pub total_tokens_display: String,
    pub total_input_tokens_display: String,
    pub total_output_tokens_display: String,
    pub total_cache_read_display: String,
    pub total_cache_write_display: String,
    pub date_range: String,
    pub projects: Vec<ProjectCard>,
}

pub struct ProjectCard {
    pub name: String,
    pub short_name: String,
    pub display_name: String,
    pub session_count: u32,
    pub message_count: u32,
    pub token_total: String,
    pub token_total_raw: u64,
    pub last_activity: Option<String>,
    pub last_activity_display: String,
}

/// Aggregate token counts across all projects.
pub struct TokenTotals {
    pub total: u64,
    pub input: u64,
    pub output: u64,
    pub cache_creation: u64,
    pub cache_read: u64,
}

/// Build an [`IndexContext`] from cached project metadata.
pub fn build_context(
    css: String,
    projects: Vec<super::IndexProjectData>,
    total_messages: u32,
    tokens: TokenTotals,
    earliest: Option<String>,
    latest: Option<String>,
) -> IndexContext {
    let date_range = match (earliest.as_deref(), latest.as_deref()) {
        (Some(e), Some(l)) => format!("{} – {}", format_short_date(e), format_short_date(l)),
        (Some(e), None) => format_short_date(e),
        _ => "—".to_string(),
    };

    let project_cards: Vec<ProjectCard> = projects
        .into_iter()
        .map(|p| ProjectCard {
            name: p.name,
            short_name: p.short_name,
            display_name: p.display_name,
            session_count: p.session_count,
            message_count: p.message_count,
            token_total: format_token_count(p.total_tokens),
            token_total_raw: p.total_tokens,
            last_activity_display: p
                .last_activity
                .as_deref()
                .map(super::project::format_relative_time)
                .unwrap_or_else(|| "—".to_string()),
            last_activity: p.last_activity,
        })
        .collect();

    let total_projects = project_cards.len();

    IndexContext {
        css,
        index_js: crate::assets::INDEX_JS.to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        total_projects,
        total_sessions: project_cards.iter().map(|p| p.session_count).sum(),
        total_messages,
        total_tokens_display: format_token_count(tokens.total),
        total_input_tokens_display: format_token_count(tokens.input),
        total_output_tokens_display: format_token_count(tokens.output),
        total_cache_read_display: format_token_count(tokens.cache_read),
        total_cache_write_display: format_token_count(tokens.cache_creation),
        date_range,
        projects: project_cards,
    }
}

fn format_token_count(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

fn format_short_date(s: &str) -> String {
    // Take just the date part (YYYY-MM-DD) and show as "Mon DD".
    if s.len() >= 10 {
        let date_part = &s[..10];
        if let Ok(d) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
            return d.format("%b %d").to_string();
        }
    }
    s.to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_index_context_is_self_contained() {
        let css = crate::assets::CSS.to_string();
        let ctx = build_context(
            css.clone(),
            vec![super::super::IndexProjectData {
                name: "test-proj".into(),
                session_count: 3,
                message_count: 45,
                total_tokens: 15000,
                short_name: "test-proj".into(),
                display_name: "test-proj".into(),
                last_activity: Some("2025-06-15T12:00:00Z".into()),
            }],
            45,
            TokenTotals {
                total: 15000,
                input: 5000,
                output: 3000,
                cache_creation: 4000,
                cache_read: 3000,
            },
            Some("2025-06-15T10:00:00Z".into()),
            Some("2025-06-15T12:00:00Z".into()),
        );
        let html = ctx.render().expect("template should render");
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("test-proj"));
        assert!(html.contains("15.0k"));
        assert!(html.contains("15000"), "raw token count for sorting");
        // Relative time should be present.
        assert!(
            html.contains("h ago")
                || html.contains("m ago")
                || html.contains("Jun")
                || html.contains("—"),
            "relative time should be present"
        );
        // Index interactivity controls.
        assert!(html.contains("data-view-mode"), "view switcher buttons should be present");
        assert!(html.contains("index-search-input"), "search input should be present");
        assert!(html.contains("date-chip"), "date filter chips should be present");
        assert!(html.contains("data-view=\"cards\""), "default view mode attr should be present");
        assert!(html.contains("view-switcher"), "segmented view switcher should be present");
        // Data attributes for filtering.
        assert!(html.contains("data-display-name"), "display name data attr for search");
        assert!(html.contains("data-short-name"), "short name data attr for search");
        assert!(html.contains("data-last-activity"), "last activity data attr for date filter");
        assert!(html.contains("data-sessions"), "sessions data attr for sorting");
        assert!(!html.contains("http://"));
        assert!(!html.contains("https://"));
    }
}
