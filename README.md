# jaringan-wiki

**AI-powered private wikis over the Jaringan protocol.**

A wiki server that turns a directory of markdown files into Jaringan pages — browseable with the [Jaringan Browser](https://github.com/thesimonharms/jaringan). Designed for both **human and AI agent** use via CLI and MCP.

## Quick Start

```bash
# Create a new wiki
jaringan-wiki init ~/my-wiki

# Serve it over Jaringan
jaringan-wiki serve ~/my-wiki

# Browse with the Jaringan Browser at jrg://wiki/
# (the wiki host is routed to your server; path / returns the index)
```

## CLI

| Command | Description |
|---------|-------------|
| `init <path>` | Initialise a new wiki directory with sample pages |
| `add <path> <slug> <markdown>` | Add/update a page from content (agent-friendly) |
| `get <path> <slug>` | Print raw markdown to stdout |
| `rm <path> <slug>` | Delete a page |
| `list <path>` | List all pages |
| `search <path> <query>` | Full-text search |
| `dump <path>` | Batch-read all pages as JSON |
| `add <path> <slug> --stdin` | Pipe content from stdin |
| `list --json` / `search --json` | Machine-parseable output |

### Agent workflow

```bash
# Build a knowledge base from AI output
jaringan-wiki add . research-notes '# Research Notes

Key findings from the experiment...'

# Read pages for context
jaringan-wiki get . research-notes

# Search across pages
jaringan-wiki search . "key findings" --json

# Batch-read everything
jaringan-wiki dump .
```

## MCP Server

For AI agents that speak the Model Context Protocol (Claude Code, Hermes, Cursor, etc.):

```bash
# Start the MCP server (stdio transport)
jaringan-wiki-mcp --wiki-path ~/my-wiki

# Configure in Claude Code:
# > /mcp add wiki -- npx jaringan-wiki-mcp --wiki-path ~/my-wiki

# Configure in Hermes Agent (~/.hermes/config.yaml):
# mcp_servers:
#   wiki:
#     command: jaringan-wiki-mcp
#     args: ["--wiki-path", "~/my-wiki"]
```

### MCP Tools

| Tool | Description |
|------|-------------|
| `wiki_read(slug)` | Read a page by slug — returns title, body, tags, links |
| `wiki_write(slug, content)` | Create or update a page with full markdown content |
| `wiki_search(query)` | Full-text search — returns matching pages with snippets |
| `wiki_list()` | List all pages with slugs, titles, tags, timestamps |

## How the URI works

`jrg://` is the Jaringan protocol scheme. The host part is **advisory** — the resolver
doesn't validate or resolve it via DNS. It only looks at the **path**. The same server
responds identically to `jrg://wiki/welcome`, `jrg://my-wiki/welcome`, or
`jrg://anything/welcome` — the host is just a label passed through to the resolver.

| URL | Resolves to |
|-----|-------------|
| `jrg://<any>/` | Wiki index (page listing) |
| `jrg://<any>/welcome` | Welcome page |
| `jrg://<any>/wiki/welcome` | Also works (`wiki/` prefix is optional) |

Running `jaringan-wiki serve` binds a JRG TCP server that handles all incoming requests regardless of hostname.

## Wiki Format

Pages are plain markdown files with optional YAML front matter:

```markdown
---
title: My Page
tags: [rust, networking]
---

# My Page

Content here. Link to other pages with [[page-slug]]
or [[page-slug|Display Text]].
```

The server converts `[[slug]]` links to `jrg://wiki/slug` links automatically.

## AI Features

Set one of these environment variables to enable AI:

| Env var | Provider |
|---------|----------|
| `OPENAI_API_KEY` | OpenAI |
| `ANTHROPIC_API_KEY` | Anthropic Claude |
| `XAI_API_KEY` | xAI Grok |
| `AI_MODEL` | Model override (default: `gpt-4o-mini`) |

> *AI methods are stubbed in v0.1 — full baochuan integration coming next.*

## Architecture

```
jaringan-wiki/
├── src/
│   ├── main.rs          # CLI (clap)
│   ├── mcp_server.rs    # MCP server (stdio transport)
│   ├── lib.rs           # Public API
│   ├── wiki.rs          # Content model + markdown parsing
│   ├── resolver.rs      # JRG PageResolver implementation
│   ├── search.rs        # Full-text search index
│   └── ai.rs            # AI integration (baochuan)
└── README.md
```

Two runnable binaries are built:
- `jaringan-wiki` — CLI for humans and agents
- `jaringan-wiki-mcp` — MCP stdio server for AI agent tool integration

## License

MIT
