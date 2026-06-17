//! jaringan-wiki CLI
//!
//! Manage and serve AI-powered private wikis over the Jaringan protocol.
//! Designed for both human and AI agent use — all data operations accept
//! piped stdin and produce parseable output.

use std::io::Read;
use std::net::TcpListener;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

use jaringan_wiki::{AiWiki, SearchIndex, Wiki, WikiResolver};

#[derive(Parser)]
#[command(name = "jaringan-wiki", version, about = "AI-powered wikis over Jaringan")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialise a new wiki directory with sample pages.
    Init {
        /// Directory to initialise
        path: PathBuf,
    },
    /// Add or update a page from raw markdown (agent-friendly).
    ///
    /// Pass content as the last argument(s), pipe via stdin with `--stdin`,
    /// or omit to read from stdin automatically when piped.
    ///
    /// Examples:
    ///
    /// ```ignore
    /// # As a positional argument:
    /// jaringan-wiki add . my-page '# Page Title
 
    /// Content here...'
    ///
    /// # From stdin (agent-friendly):
    /// echo '# Page Title
    /// 
    /// Content here...' | jaringan-wiki add . my-page --stdin
    /// ```
    Add {
        /// Wiki root directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Page slug (URL-safe name, e.g. "my-page")
        slug: String,
        /// Read content from stdin instead of positional args
        #[arg(long)]
        stdin: bool,
        /// Page title (auto-extracted from markdown heading if omitted)
        #[arg(short, long)]
        title: Option<String>,
        /// Comma-separated tags
        #[arg(short, long)]
        tags: Option<String>,
        /// Page body in markdown (pass as positional args, or use --stdin)
        #[arg(required_unless_present = "stdin")]
        markdown: Vec<String>,
    },
    /// Output a page's raw markdown to stdout.
    Get {
        /// Wiki root directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Page slug
        slug: String,
    },
    /// Delete a wiki page.
    Rm {
        /// Wiki root directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Page slug
        slug: String,
    },
    /// Create a stub page (use `add` for full-content creation).
    Create {
        /// Wiki root directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Page slug (URL-safe name)
        slug: String,
        /// Page title
        #[arg(short, long)]
        title: Option<String>,
    },
    /// Serve wiki over JRG TCP protocol.
    Serve {
        /// Wiki root directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Listen address for JRG TCP server
        #[arg(short, long, default_value = "127.0.0.1:7080")]
        listen: String,
    },
    /// Full-text search across wiki pages.
    Search {
        /// Wiki root directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Search query
        query: String,
        /// Output as JSON (agent-friendly)
        #[arg(long)]
        json: bool,
    },
    /// List all wiki pages.
    List {
        /// Wiki root directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Output as JSON (agent-friendly)
        #[arg(long)]
        json: bool,
    },
    /// Dump all pages as a JSON array (agent-friendly batch read).
    Dump {
        /// Wiki root directory
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// AI: generate a new page from a prompt (prints to stdout).
    Generate {
        /// Wiki root directory
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Description of the page to generate
        prompt: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { path } => cmd_init(path),
        Commands::Add { path, slug, stdin, title, tags, markdown } => {
            cmd_add(path, slug, stdin, title, tags, markdown)
        }
        Commands::Get { path, slug } => cmd_get(path, slug),
        Commands::Rm { path, slug } => cmd_rm(path, slug),
        Commands::Create { path, slug, title } => cmd_create(path, slug, title),
        Commands::Serve { path, listen } => cmd_serve(path, listen),
        Commands::Search { path, query, json } => cmd_search(path, &query, json),
        Commands::List { path, json } => cmd_list(path, json),
        Commands::Dump { path } => cmd_dump(path),
        Commands::Generate { path, prompt } => cmd_generate(path, &prompt),
    }
}

fn cmd_init(path: PathBuf) -> Result<()> {
    std::fs::create_dir_all(&path)
        .with_context(|| format!("failed to create {}", path.display()))?;

    let config_path = path.join("wiki.yaml");
    if !config_path.exists() {
        let config = r#"title: "My Wiki"
author: ""
signing_key: ~
ai_config_path: ~
"#;
        std::fs::write(&config_path, config).context("failed to write wiki.yaml")?;
    }

    // Write sample pages if they don't exist
    let samples: [(&str, &str, &str); 3] = [
        ("welcome", "Welcome", r##"---
title: Welcome
tags: [getting-started]
---

# Welcome to Your Wiki

This is the start of your personal knowledge base.

## Getting Started

- Add pages: `jaringan-wiki add <slug> <markdown>`
- Serve: `jaringan-wiki serve`
- Browse: `jrg://wiki/` (Jaringan Browser)

## Links

- [[creating-pages|Creating Pages]]
- [[ai-features|AI Features]]
"##),
        ("creating-pages", "Creating Pages", r##"---
title: Creating Pages
tags: [guide]
---

# Creating Pages

Add pages with:

    jaringan-wiki add <path> <slug> 'page content here'

Or pipe from stdin:

    cat page.md | jaringan-wiki add <path> <slug> --stdin

## Front Matter

Pages support optional YAML front matter with `title` and `tags`.

## Wiki Links

Link to other pages with `[[page-slug]]` or `[[page-slug|Display Text]]`.
"##),
        ("ai-features", "AI Features", r##"---
title: AI Features
tags: [ai, guide]
---

# AI Features

jaringan-wiki integrates with AI providers via the **baochuan** crate.

## Setup

Set one of: `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, or `XAI_API_KEY`.

## AI Agent Usage

This wiki is designed for AI agent workflows:

    # Build a knowledge base:
    jaringan-wiki add . research-notes '# Research Notes

    Key findings...'

    # Read pages:
    jaringan-wiki get . research-notes
    jaringan-wiki dump .

    # Search:
    jaringan-wiki search . "key findings" --json
"##),
    ];

    for (slug, title, body) in &samples {
        let page_path = path.join(format!("{slug}.md"));
        if !page_path.exists() {
            std::fs::write(&page_path, body)
                .with_context(|| format!("failed to write {slug}.md"))?;
            eprintln!("  Created: {slug} ({title})");
        }
    }

    println!("✓ Initialised wiki at {}", path.display());
    println!("  Config: {}", config_path.display());
    eprintln!("  Usage: jaringan-wiki add <slug> <markdown>");
    Ok(())
}

/// Add or update a page from raw markdown.
/// Supports both positional args and --stdin for piped input.
fn cmd_add(
    path: PathBuf,
    slug: String,
    use_stdin: bool,
    title: Option<String>,
    tags: Option<String>,
    markdown: Vec<String>,
) -> Result<()> {
    let root = if path.is_absolute() { path } else {
        std::env::current_dir()?.join(&path)
    };
    std::fs::create_dir_all(&root)?;

    // Read content
    let body = if use_stdin {
        let mut buf = String::new();
        std::io::stdin().read_to_string(&mut buf)
            .context("failed to read from stdin")?;
        buf
    } else {
        markdown.join(" ")
    };

    if body.trim().is_empty() {
        anyhow::bail!("no content provided — pass markdown as argument or use --stdin");
    }

    // Build the file content with optional front matter
    let mut content = body.trim().to_owned();

    // If the body doesn't start with front matter or a heading, add a heading
    if !content.starts_with("---") && !content.starts_with("# ") {
        let actual_title = title.clone().unwrap_or_else(|| slug.clone());
        if let Some(tag_str) = tags {
            let tag_list: Vec<&str> = tag_str.split(',').map(|t| t.trim()).collect();
            let tags_yaml = format!("tags: [{}]", tag_list.join(", "));
            content = format!("---\ntitle: {actual_title}\n{tags_yaml}\n---\n\n# {actual_title}\n\n{content}");
        } else {
            content = format!("---\ntitle: {actual_title}\ntags: []\n---\n\n# {actual_title}\n\n{content}");
        }
    } else if content.starts_with("---") {
        // Has front matter but might be missing tags
        // We'll just write it as-is — the user provided full content
    }

    // Write the file
    let file_path = root.join(format!("{slug}.md"));
    std::fs::write(&file_path, &content)
        .with_context(|| format!("failed to write {}", file_path.display()))?;

    // Reload so the wiki picks it up
    let wiki = Wiki::load(&root)?;
    let page = wiki.pages.get(&slug);

    if let Some(p) = page {
        println!("✓ {} ({})", p.slug, p.title);
    } else {
        println!("✓ {slug}");
    }

    Ok(())
}

/// Output a page's raw markdown to stdout (agent-friendly).
fn cmd_get(path: PathBuf, slug: String) -> Result<()> {
    let wiki = Wiki::load(&path)?;
    let page = wiki.pages.get(&slug)
        .ok_or_else(|| anyhow::anyhow!("page not found: {slug}"))?;

    print!("{}", page.body);
    Ok(())
}

/// Delete a wiki page.
fn cmd_rm(path: PathBuf, slug: String) -> Result<()> {
    let file_path = path.join(format!("{slug}.md"));
    if !file_path.exists() {
        anyhow::bail!("page not found: {slug}");
    }
    std::fs::remove_file(&file_path)
        .with_context(|| format!("failed to delete {}", file_path.display()))?;
    println!("✓ Deleted: {slug}");
    Ok(())
}

/// Create a stub page.
fn cmd_create(path: PathBuf, slug: String, title: Option<String>) -> Result<()> {
    let wiki = Wiki::load(&path)?;
    let title = title.unwrap_or_else(|| slug.clone());
    wiki.create_page(&slug, &title, "Write your content here.")?;
    println!("✓ Created: {slug} ({title})");
    eprintln!("  Edit: {}/{slug}.md", path.display());
    Ok(())
}

/// Serve wiki over JRG TCP.
fn cmd_serve(path: PathBuf, listen: String) -> Result<()> {
    let wiki = Wiki::load(&path)?;
    eprintln!("✓ Loaded wiki: {}", wiki.config.title);
    eprintln!("  Pages: {}", wiki.pages.len());
    eprintln!("  Listening: jrg://{listen}");
    eprintln!("  Browse: jrg://wiki/");

    let ai = AiWiki::from_env();
    if ai.is_enabled() {
        eprintln!("  AI features: enabled");
    } else {
        eprintln!("  AI features: disabled (set OPENAI_API_KEY/ANTHROPIC_API_KEY/XAI_API_KEY)");
    }

    let resolver = WikiResolver::new(wiki);
    let listener = TcpListener::bind(&listen)
        .with_context(|| format!("failed to bind to {listen}"))?;

    jaringan_protocol::serve(listener, resolver)
        .map_err(|e| anyhow::anyhow!("server error: {e}"))?;

    Ok(())
}

/// Full-text search.
fn cmd_search(path: PathBuf, query: &str, json: bool) -> Result<()> {
    let wiki = Wiki::load(&path)?;
    let results = SearchIndex::search(&wiki, query);

    if json {
        let output: Vec<serde_json::Value> = results.iter().map(|r| {
            serde_json::json!({
                "slug": r.slug,
                "title": r.title,
                "relevance": r.relevance,
                "snippet": r.snippet,
            })
        }).collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    if results.is_empty() {
        println!("No results for: {query}");
        return Ok(());
    }

    println!("Results for \"{query}\":\n");
    for (i, r) in results.iter().enumerate() {
        println!("{}. {} ({})", i + 1, r.title, r.slug);
        println!("   {}", r.snippet);
        println!();
    }

    Ok(())
}

/// List pages.
fn cmd_list(path: PathBuf, json: bool) -> Result<()> {
    let wiki = Wiki::load(&path)?;
    let ai = AiWiki::from_env();

    if json {
        let output: Vec<serde_json::Value> = wiki.pages.values().map(|p| {
            serde_json::json!({
                "slug": p.slug,
                "title": p.title,
                "tags": p.tags,
                "updated_at": p.updated_at,
                "links": p.links,
            })
        }).collect();
        println!("{}", serde_json::to_string_pretty(&output)?);
        return Ok(());
    }

    println!("Wiki: {}", wiki.config.title);
    if !wiki.config.author.is_empty() {
        println!("Author: {}", wiki.config.author);
    }
    println!("Pages: {}\n", wiki.pages.len());

    let mut entries: Vec<_> = wiki.pages.values().collect();
    entries.sort_by(|a, b| a.slug.cmp(&b.slug));

    for page in &entries {
        print!("  {}  {}", page.slug, page.title);
        if !page.tags.is_empty() {
            print!(" [{}]", page.tags.join(", "));
        }
        println!();
    }

    if ai.is_enabled() {
        println!("\nAI features available");
    }

    Ok(())
}

/// Dump all pages as JSON array (agent-friendly batch read).
fn cmd_dump(path: PathBuf) -> Result<()> {
    let wiki = Wiki::load(&path)?;

    let output: Vec<serde_json::Value> = wiki.pages.values().map(|p| {
        serde_json::json!({
            "slug": p.slug,
            "title": p.title,
            "body": p.body,
            "tags": p.tags,
            "links": p.links,
            "updated_at": p.updated_at,
        })
    }).collect();

    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}

/// AI: generate a page from a prompt.
fn cmd_generate(_path: PathBuf, prompt: &str) -> Result<()> {
    let ai = AiWiki::from_env();
    let content = ai.generate_page(prompt)?;
    println!("{content}");
    Ok(())
}
