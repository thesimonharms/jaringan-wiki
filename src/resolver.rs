//! JRG PageResolver for wiki content.
//!
//! Serves wiki pages as Jaringan pages over the JRG protocol.
//! Handles `jrg://wiki/<slug>` URLs and provides directory listing
//! for `jrg://wiki/`.

use std::sync::Arc;

use jaringan_protocol::{
    PageResolver, Request, ResolveError, Response, StatusCode,
};
use tokio::sync::RwLock;

use crate::wiki::Wiki;

/// A PageResolver that serves wiki content as Jaringan pages.
pub struct WikiResolver {
    wiki: Arc<RwLock<Wiki>>,
}

impl WikiResolver {
    /// Create a new resolver backed by the given wiki.
    pub fn new(wiki: Wiki) -> Self {
        Self { wiki: Arc::new(RwLock::new(wiki)) }
    }

    pub fn wiki(&self) -> &Arc<RwLock<Wiki>> {
        &self.wiki
    }
}

impl PageResolver for WikiResolver {
    fn fetch(&self, request: &Request) -> Result<Response, ResolveError> {
        let path = request.url.path();

        // Strip leading slash and "wiki/" prefix
        let slug = path
            .trim_start_matches('/')
            .strip_prefix("wiki/")
            .unwrap_or(path.trim_start_matches('/'));

        let wiki = self.wiki.blocking_read();

        if slug.is_empty() || slug == "index" {
            // Serve index / directory listing
            return Ok(Response::page(
                StatusCode::Ok,
                build_index_page(&wiki),
            ));
        }

        let page = match wiki.pages.get(slug) {
            Some(p) => p,
            None => {
                return Ok(Response::page(
                    StatusCode::NotFound,
                    format!("# Not Found\n\nNo wiki page with slug `{slug}` exists.\n\n[← Wiki Index](jrg://wiki/)"),
                ));
            }
        };

        let jrg_content = build_jrg_page(&wiki, page);
        Ok(Response::page(StatusCode::Ok, jrg_content))
    }
}

/// Build a Jaringan document from a wiki page.
fn build_jrg_page(wiki: &Wiki, page: &crate::wiki::WikiPage) -> String {
    let mut jrg = String::new();

    // Page title
    jrg.push_str(&format!("# {}\n\n", page.title));

    // Metadata block
    jrg.push_str(&format!("> Updated: {}\n", page.updated_at));
    if !page.tags.is_empty() {
        jrg.push_str(&format!("> Tags: {}\n\n", page.tags.join(", ")));
    }

    // Convert wiki links [[slug]] to JRG links
    let body = convert_wiki_links(&page.body, &wiki.pages);
    jrg.push_str(&body);

    // Footer with navigation
    jrg.push_str("\n---\n\n");
    jrg.push_str("[← Wiki Index](jrg://wiki/)\n");

    jrg
}

/// Build the wiki index page (directory listing).
fn build_index_page(wiki: &Wiki) -> String {
    let mut jrg = format!("# {}\n\n", wiki.config.title);

    if !wiki.config.author.is_empty() {
        jrg.push_str(&format!("> Author: {}\n", wiki.config.author));
    }
    jrg.push_str(&format!("> Pages: {}\n\n", wiki.pages.len()));

    jrg.push_str("## Pages\n\n");

    // List all pages
    let mut entries: Vec<_> = wiki.pages.values().collect();
    entries.sort_by(|a, b| a.slug.cmp(&b.slug));

    for page in &entries {
        jrg.push_str(&format!("- [{}](jrg://wiki/{})\n", page.title, page.slug));
        if !page.tags.is_empty() {
            jrg.push_str(&format!("  `{}`\n", page.tags.join(", ")));
        }
    }

    jrg.push_str("\n---\n");
    jrg.push_str("> Powered by [jaringan-wiki](https://github.com/thesimonharms/jaringan-wiki)\n");

    jrg
}

/// Convert `[[slug]]` links to JRG links.
/// If the linked page exists, use its title. Otherwise use the slug.
fn convert_wiki_links(
    body: &str,
    pages: &std::collections::HashMap<String, crate::wiki::WikiPage>,
) -> String {
    let mut result = String::new();
    let mut remaining = body;

    while let Some(start) = remaining.find("[[") {
        // Push text before the link
        result.push_str(&remaining[..start]);
        remaining = &remaining[start + 2..];

        if let Some(end) = remaining.find("]]") {
            let inner = &remaining[..end];
            let (slug, display) = if let Some(pipe) = inner.find('|') {
                (&inner[..pipe], Some(&inner[pipe + 1..]))
            } else {
                (inner, None)
            };

            let title = display
                .map(|s| s.to_owned())
                .or_else(|| pages.get(slug).map(|p| p.title.clone()))
                .unwrap_or_else(|| slug.to_owned());

            result.push_str(&format!("[{}](jrg://wiki/{slug})", title));
            remaining = &remaining[end + 2..];
        } else {
            // No closing ]], push everything back
            result.push_str(&format!("[[{remaining}"));
            remaining = "";
        }
    }

    result.push_str(remaining);
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_wiki_links() {
        let body = "See [[getting-started]] and [[api|API Docs]].";
        let pages = std::collections::HashMap::new();
        let result = convert_wiki_links(body, &pages);
        assert_eq!(
            result,
            "See [getting-started](jrg://wiki/getting-started) and [API Docs](jrg://wiki/api)."
        );
    }
}
