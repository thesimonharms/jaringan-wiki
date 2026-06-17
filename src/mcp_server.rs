//! MCP (Model Context Protocol) server for jaringan-wiki.
//!
//! Exposes wiki operations as MCP tools over stdio transport, so AI
//! agents can read, write, search, and manage wiki pages through
//! structured tool calls instead of raw CLI commands.
//!
//! # Usage
//!
//! ```bash
//! # Start the MCP server (Hermes / Claude Code / etc. connect via stdio)
//! jaringan-wiki-mcp --wiki-path ~/my-wiki
//! ```
//!
//! # MCP tools exposed
//!
//! - `wiki_read` — read a page by slug
//! - `wiki_write` — create or update a page
//! - `wiki_search` — full-text search across pages
//! - `wiki_list`  — list all page slugs and titles

use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;

use jaringan_wiki::{SearchIndex, Wiki};

// ── CLI ─────────────────────────────────────────────────────────────

#[derive(Parser)]
#[command(name = "jaringan-wiki-mcp", version, about = "MCP server for jaringan-wiki")]
struct Cli {
    /// Path to the wiki directory
    #[arg(short, long, default_value = ".")]
    wiki_path: PathBuf,
}

// ── JSON-RPC types ──────────────────────────────────────────────────

#[derive(serde::Serialize, serde::Deserialize)]
struct RpcMessage {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<serde_json::Value>,
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<RpcError>,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct RpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<serde_json::Value>,
}

fn rpc_error(code: i32, msg: impl Into<String>) -> RpcMessage {
    RpcMessage {
        id: None,
        jsonrpc: "2.0".into(),
        method: None,
        params: None,
        result: None,
        error: Some(RpcError {
            code,
            message: msg.into(),
            data: None,
        }),
    }
}

fn rpc_result(id: serde_json::Value, result: serde_json::Value) -> RpcMessage {
    RpcMessage {
        id: Some(id),
        jsonrpc: "2.0".into(),
        method: None,
        params: None,
        result: Some(result),
        error: None,
    }
}

fn rpc_notification(method: &str, params: serde_json::Value) -> RpcMessage {
    RpcMessage {
        id: None,
        jsonrpc: "2.0".into(),
        method: Some(method.into()),
        params: Some(params),
        result: None,
        error: None,
    }
}

// ── Tool definitions (MCP schema) ───────────────────────────────────

/// Build the `tools/list` response.
fn tool_list_response() -> serde_json::Value {
    serde_json::json!([
        {
            "name": "wiki_read",
            "description": "Read a wiki page by slug. Returns the page title, body (markdown), tags, and links.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "slug": {
                        "type": "string",
                        "description": "Page slug (e.g. 'welcome', 'research-notes')"
                    }
                },
                "required": ["slug"]
            }
        },
        {
            "name": "wiki_write",
            "description": "Create or update a wiki page. Content should include YAML front matter with title and tags if desired, or just markdown body.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "slug": {
                        "type": "string",
                        "description": "Page slug (URL-safe name)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Full page content in markdown. May include YAML front matter (---\\ntitle: ...\\ntags: [...]\\n---\\n\\nBody)"
                    }
                },
                "required": ["slug", "content"]
            }
        },
        {
            "name": "wiki_search",
            "description": "Full-text search across all wiki pages. Returns matching page slugs, titles, relevance scores, and text snippets.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    }
                },
                "required": ["query"]
            }
        },
        {
            "name": "wiki_list",
            "description": "List all wiki pages with their slugs, titles, tags, and update timestamps.",
            "inputSchema": {
                "type": "object",
                "properties": {}
            }
        },
    ])
}

// ── Tool handlers ───────────────────────────────────────────────────

fn handle_tool_call(
    wiki: &Wiki,
    tool: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    match tool {
        "wiki_read" => {
            let slug = args.get("slug")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing required argument: slug".to_string())?;
            let page = wiki.pages.get(slug)
                .ok_or_else(|| format!("page not found: {slug}"))?;
            Ok(serde_json::json!({
                "slug": page.slug,
                "title": page.title,
                "body": page.body,
                "tags": page.tags,
                "links": page.links,
                "updated_at": page.updated_at,
            }))
        }
        "wiki_write" => {
            let slug = args.get("slug")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing required argument: slug".to_string())?;
            let content = args.get("content")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing required argument: content".to_string())?;

            // Validate slug is safe
            if !slug.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '/') {
                return Err("slug may only contain alphanumeric chars, hyphens, underscores, and forward slashes".into());
            }

            let file_path = wiki.root.join(format!("{slug}.jrg"));
            std::fs::write(&file_path, content)
                .map_err(|e| format!("failed to write {slug}.jrg: {e}"))?;

            Ok(serde_json::json!({
                "status": "ok",
                "slug": slug,
                "path": file_path.to_string_lossy(),
            }))
        }
        "wiki_search" => {
            let query = args.get("query")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "missing required argument: query".to_string())?;
            let results = SearchIndex::search(wiki, query);
            let items: Vec<serde_json::Value> = results.iter().map(|r| {
                serde_json::json!({
                    "slug": r.slug,
                    "title": r.title,
                    "relevance": r.relevance,
                    "snippet": r.snippet,
                })
            }).collect();
            Ok(serde_json::json!({ "results": items, "count": items.len() }))
        }
        "wiki_list" => {
            let mut entries: Vec<serde_json::Value> = wiki.pages.values().map(|p| {
                serde_json::json!({
                    "slug": p.slug,
                    "title": p.title,
                    "tags": p.tags,
                    "updated_at": p.updated_at,
                    "links": p.links,
                })
            }).collect();
            entries.sort_by(|a, b| {
                a["slug"].as_str().unwrap_or("").cmp(b["slug"].as_str().unwrap_or(""))
            });
            Ok(serde_json::json!({ "pages": entries, "count": entries.len() }))
        }
        _ => Err(format!("unknown tool: {tool}")),
    }
}

// ── Main event loop ─────────────────────────────────────────────────

fn main() -> Result<()> {
    let cli = Cli::parse();
    let wiki_path = if cli.wiki_path.is_absolute() {
        cli.wiki_path
    } else {
        std::env::current_dir()?.join(&cli.wiki_path)
    };

    // Load the wiki at startup
    let mut wiki = Wiki::load(&wiki_path)
        .with_context(|| format!("failed to load wiki at {}", wiki_path.display()))?;

    eprintln!("[wiki-mcp] loaded wiki: {} ({} pages)", wiki.config.title, wiki.pages.len());

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut stdout = stdout.lock();

    // Send server info as a notification (not strictly MCP spec, but helpful)
    let init_notification = rpc_notification("wiki_init", serde_json::json!({
        "wiki": wiki.config.title,
        "pages": wiki.pages.len(),
        "path": wiki_path.to_string_lossy(),
    }));
    let _ = writeln!(stdout, "{}", serde_json::to_string(&init_notification).unwrap());
    stdout.flush()?;

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[wiki-mcp] stdin error: {e}");
                break;
            }
        };

        if line.trim().is_empty() {
            continue;
        }

        let msg: RpcMessage = match serde_json::from_str(&line) {
            Ok(m) => m,
            Err(e) => {
                let err = rpc_error(-32700, format!("parse error: {e}"));
                let _ = writeln!(stdout, "{}", serde_json::to_string(&err).unwrap());
                stdout.flush()?;
                continue;
            }
        };

        let method = match &msg.method {
            Some(m) => m.as_str(),
            None => {
                // Notification or response — ignore in server role
                continue;
            }
        };

        let response = match method {
            "initialize" => {
                // MCP handshake
                rpc_result(
                    msg.id.unwrap_or(serde_json::Value::Null),
                    serde_json::json!({
                        "protocolVersion": "2025-03-26",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "jaringan-wiki-mcp",
                            "version": env!("CARGO_PKG_VERSION"),
                        }
                    }),
                )
            }
            "notifications/initialized" => {
                // Handshake complete — no response needed
                continue;
            }
            "tools/list" => {
                rpc_result(
                    msg.id.unwrap_or(serde_json::Value::Null),
                    serde_json::json!({ "tools": tool_list_response() }),
                )
            }
            "tools/call" => {
                let params = msg.params.as_ref();
                let tool_name = params
                    .and_then(|p| p.get("name"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let args = params
                    .and_then(|p| p.get("arguments"))
                    .cloned()
                    .unwrap_or(serde_json::json!({}));

                match handle_tool_call(&wiki, tool_name, &args) {
                    Ok(result) => {
                        rpc_result(
                            msg.id.unwrap_or(serde_json::Value::Null),
                            serde_json::json!({
                                "content": [{
                                    "type": "text",
                                    "text": serde_json::to_string_pretty(&result).unwrap_or_default(),
                                }]
                            }),
                        )
                    }
                    Err(e) => {
                        rpc_result(
                            msg.id.unwrap_or(serde_json::Value::Null),
                            serde_json::json!({
                                "isError": true,
                                "content": [{
                                    "type": "text",
                                    "text": format!("Error: {e}"),
                                }]
                            }),
                        )
                    }
                }
            }
            _ => {
                rpc_error(-32601, format!("method not found: {method}"))
            }
        };

        let line = serde_json::to_string(&response)?;
        writeln!(stdout, "{line}")?;
        stdout.flush()?;

        // Reload wiki after write operations so subsequent reads are up-to-date
        if method == "tools/call" {
            if let Some(params) = &msg.params {
                if let Some(name) = params.get("name").and_then(|v| v.as_str()) {
                    if name == "wiki_write" {
                        wiki = Wiki::load(&wiki_path)
                            .unwrap_or(wiki);
                    }
                }
            }
        }
    }

    Ok(())
}
