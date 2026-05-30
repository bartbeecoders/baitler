# Baitler as an MCP server

Baitler exposes its features — ideas, HTML documents, files/folders, content
conversion/export, and LLM chat — as tools over the
[Model Context Protocol (MCP)](https://modelcontextprotocol.io). Any MCP‑compatible
client (Claude Code, the Hermes agent, Cursor, Claude Desktop, custom agents, …)
can connect and drive Baitler with natural language.

This guide covers what the server is, how to run it, and how to install it in:

- [Claude Code](#claude-code)
- [Hermes agent](#hermes-agent)
- [Other MCP‑compatible tools](#other-mcp-compatible-tools)

---

## How it works

Baitler ships MCP **two ways**, so it fits every client:

| Transport | Endpoint / command | Use it when |
|-----------|--------------------|-------------|
| **Streamable HTTP** (recommended) | `POST http://<host>:<port>/mcp` | Your client supports HTTP/“streamable‑http” MCP servers (Claude Code, Cursor, most modern clients). One server process, shared database, many clients. |
| **stdio bridge** | the `baitler-mcp` binary | Your client only launches **stdio** servers (a command it spawns). The bridge forwards stdio JSON‑RPC to a running server’s `/mcp`. |

Both expose the **same tools** and talk to the **same database**, because the
HTTP endpoint is served *inside* the Baitler API process and reuses its
repositories. The stdio bridge is a thin forwarder — it does **not** open the
database itself (so it never conflicts with the API over the RocksDB file lock).

```
┌─────────────┐   HTTP (recommended)   ┌────────────────────────┐
│  MCP client │ ─────────────────────▶ │  Baitler API           │
│ (Claude…)   │   POST /mcp            │  ┌──────────────────┐  │
└─────────────┘                        │  │ /mcp  (in‑proc)  │  │
       │                               │  │ ideas/docs/files │  │
       │ stdio                         │  │ SurrealDB (rocks)│  │
       ▼                               │  └──────────────────┘  │
┌─────────────┐   HTTP                 │                        │
│ baitler-mcp │ ─────────────────────▶ │  POST /mcp             │
│  (bridge)   │                        └────────────────────────┘
└─────────────┘
```

### Protocol details

- JSON‑RPC 2.0 over **Streamable HTTP**, JSON‑response variant: a `POST` with a
  request returns a single JSON response; a `POST` carrying only notifications
  returns `202 Accepted` with no body. Batched arrays are supported.
- `GET /mcp` and `DELETE /mcp` return `405` — the server does not open a
  standalone server‑to‑client SSE stream; clients use request/response.
- Latest protocol revision advertised: **2025‑06‑18** (the server echoes the
  client’s requested version, so older clients interoperate too).

---

## 1. Build and run the server

Prerequisites: the Rust toolchain (see `rust-toolchain.toml`). From the repo root:

```bash
# Build the API and the stdio bridge.
cargo build --release --manifest-path backend/Cargo.toml

# Run the API (serves REST + /mcp). Data persists to ./data/surreal.db.
cargo run --release --manifest-path backend/Cargo.toml --bin baitler-api
# → Baitler API listening on 0.0.0.0:8080
```

The binaries land at:

- `backend/target/release/baitler-api` — the server (REST + MCP at `/mcp`)
- `backend/target/release/baitler-mcp` — the stdio bridge

Verify MCP is up:

```bash
curl -s -X POST http://127.0.0.1:8080/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}'
# → {"jsonrpc":"2.0","id":1,"result":{"serverInfo":{"name":"baitler",...}}}

curl -s -X POST http://127.0.0.1:8080/mcp \
  -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}'
```

### Configuration

| Env var | Default | Meaning |
|---------|---------|---------|
| `PORT` | `8080` | API port (the MCP endpoint is `http://<host>:<PORT>/mcp`). |
| `BIND_HOST` | `0.0.0.0` | Bind address. Use `127.0.0.1` to keep MCP local‑only. |
| `MCP_ENABLED` | `true` | Set `false` to remove the `/mcp` endpoint entirely. |
| `MCP_AUTH_TOKEN` | *(unset)* | When set, every `/mcp` request must send `Authorization: Bearer <token>`. |

> **Security:** with no `MCP_AUTH_TOKEN`, `/mcp` is unauthenticated — fine when
> bound to `127.0.0.1`. **Before exposing Baitler beyond localhost, set a strong
> `MCP_AUTH_TOKEN`** (and ideally terminate TLS in front of it). The token is
> compared in constant time and never logged.

---

## Claude Code

Claude Code supports MCP servers over **HTTP** (recommended here) or **stdio**.

### Option A — HTTP transport (recommended)

With the server running on `http://127.0.0.1:8080`:

```bash
# No auth token configured on the server:
claude mcp add --transport http baitler http://127.0.0.1:8080/mcp

# With an MCP_AUTH_TOKEN configured on the server:
claude mcp add --transport http baitler http://127.0.0.1:8080/mcp \
  --header "Authorization: Bearer YOUR_TOKEN"
```

Then, inside Claude Code:

```
/mcp                     # shows "baitler" connected and its tools
```

### Option B — stdio bridge

Point the bridge at the running server. Use the absolute path to the built
`baitler-mcp` binary:

```bash
claude mcp add baitler \
  --env BAITLER_API_URL=http://127.0.0.1:8080 \
  -- /ABSOLUTE/PATH/TO/backend/target/release/baitler-mcp

# If the server requires a token:
claude mcp add baitler \
  --env BAITLER_API_URL=http://127.0.0.1:8080 \
  --env BAITLER_MCP_TOKEN=YOUR_TOKEN \
  -- /ABSOLUTE/PATH/TO/backend/target/release/baitler-mcp
```

### Project‑scoped config (`.mcp.json`)

To share the configuration with everyone working in the repo, commit a
`.mcp.json` at the project root:

```json
{
  "mcpServers": {
    "baitler": {
      "type": "http",
      "url": "http://127.0.0.1:8080/mcp"
    }
  }
}
```

…or the stdio form:

```json
{
  "mcpServers": {
    "baitler": {
      "command": "/ABSOLUTE/PATH/TO/backend/target/release/baitler-mcp",
      "args": [],
      "env": { "BAITLER_API_URL": "http://127.0.0.1:8080" }
    }
  }
}
```

Manage with: `claude mcp list`, `claude mcp get baitler`, `claude mcp remove baitler`.

---

## Hermes agent

The Hermes agent is MCP‑compatible and consumes the **standard MCP server
schema** — the same `mcpServers` object Claude Desktop/Cursor/Claude Code use.
Add Baitler to Hermes’ MCP configuration with either transport.

> The exact location of Hermes’ MCP config file depends on your Hermes
> version/install (commonly a JSON settings file or an `mcpServers` block in its
> agent config). Drop one of the snippets below into that `mcpServers` map and
> restart Hermes. Consult your Hermes docs for the precise path.

**HTTP transport (recommended):**

```json
{
  "mcpServers": {
    "baitler": {
      "type": "http",
      "url": "http://127.0.0.1:8080/mcp",
      "headers": {
        "Authorization": "Bearer YOUR_TOKEN"
      }
    }
  }
}
```

Omit the `headers` block if the server has no `MCP_AUTH_TOKEN`.

**stdio transport** (if your Hermes build only launches stdio servers):

```json
{
  "mcpServers": {
    "baitler": {
      "command": "/ABSOLUTE/PATH/TO/backend/target/release/baitler-mcp",
      "args": [],
      "env": {
        "BAITLER_API_URL": "http://127.0.0.1:8080",
        "BAITLER_MCP_TOKEN": "YOUR_TOKEN"
      }
    }
  }
}
```

Drop `BAITLER_MCP_TOKEN` if the server is unauthenticated. After saving, restart
Hermes and confirm the `baitler` tools appear in its tool list.

---

## Other MCP‑compatible tools

Almost every MCP client accepts one of the two registration shapes below. Use
the host application’s “add MCP server” UI or its JSON config file.

### HTTP servers (Cursor, Claude Desktop, custom clients, …)

```json
{
  "mcpServers": {
    "baitler": {
      "type": "http",
      "url": "http://127.0.0.1:8080/mcp",
      "headers": { "Authorization": "Bearer YOUR_TOKEN" }
    }
  }
}
```

Some clients call the field `"transport": "streamable-http"` or `"http"` instead
of `"type": "http"`, and a few historically used `"sse"`. Baitler implements
Streamable HTTP; prefer the `http`/`streamable-http` option.

### stdio servers (clients that only spawn a command)

```json
{
  "mcpServers": {
    "baitler": {
      "command": "/ABSOLUTE/PATH/TO/backend/target/release/baitler-mcp",
      "args": ["http://127.0.0.1:8080"],
      "env": { "BAITLER_MCP_TOKEN": "YOUR_TOKEN" }
    }
  }
}
```

The bridge resolves its target URL from (in order): the first CLI argument,
`BAITLER_MCP_URL`, `BAITLER_API_URL`, then `http://127.0.0.1:8080`. A bare base
URL gets `/mcp` appended automatically.

### Clients that only do stdio but you want HTTP

Use the community [`mcp-remote`](https://www.npmjs.com/package/mcp-remote)
adapter, or just use Baitler’s own `baitler-mcp` bridge (above) — it does the
same job without Node.

---

## Tool reference

`tools/list` returns the live, authoritative schema. Current tools:

| Tool | Description |
|------|-------------|
| `health` | Service + database readiness. |
| `ideas_list` | List ideas; filter by `status`, `tag`, `q`, with `limit`/`offset`. |
| `ideas_get` | Get one idea + its linked ideas. |
| `ideas_create` | Create an idea (Markdown body, tags, status). |
| `ideas_update` | Update an idea’s fields. |
| `ideas_delete` | Delete an idea. |
| `ideas_link` / `ideas_unlink` | Link/unlink two ideas (symmetric). |
| `ideas_tags` | List all distinct idea tags. |
| `documents_list` | List HTML documents. |
| `documents_get` | Get one document (HTML body). |
| `documents_create` | Create a document (HTML sanitized server‑side). |
| `documents_update` | Update a document. |
| `documents_delete` | Delete a document. |
| `documents_export` | Export a document to `html`/`markdown`/`pdf`/`docx`. |
| `files_list` | List a folder’s contents, or search files (`q`). |
| `files_get` | File metadata. |
| `files_read` | Read a file’s bytes (Base64, size‑limited). |
| `files_write` | Create a file from inline `content_base64`/`content_text`. |
| `files_delete` | Delete a file. |
| `folders_create` | Create a folder (optionally nested). |
| `ai_providers` | List LLM providers/models and which are configured. |
| `ai_chat` | Run a (non‑streaming) chat completion via a provider. |
| `export` | Convert arbitrary `html`/`markdown` content to `html`/`markdown`/`pdf`/`docx`. |

Notes:

- Binary results (`pdf`, `docx`, file reads) are **Base64‑encoded** in the JSON
  result (`content_base64`, plus `content_type` and `filename`). For text
  targets, a decoded `text` field is included too.
- `pdf` export needs headless Chrome (`CHROME_BIN`); `docx` needs Pandoc
  (`PANDOC_BIN`). Without them, those exports return a clear “not available”
  error.
- `ai_chat` with the built‑in `mock` provider works offline; real providers
  (`openai`, `anthropic`, `openrouter`) need a per‑owner API key configured via
  the REST API and outbound network access.
- All tools are scoped to the single dev owner today; when authentication lands,
  the same tools will resolve the real owner from the session with no client
  changes.

---

## Troubleshooting

| Symptom | Cause / fix |
|---------|-------------|
| Client can’t connect over HTTP | Is the server running? `curl http://127.0.0.1:8080/health`. Check `PORT`/`BIND_HOST`. The URL must end in `/mcp`. |
| `401 Unauthorized` | Server has `MCP_AUTH_TOKEN` set; send `Authorization: Bearer <token>` (HTTP) or `BAITLER_MCP_TOKEN` (bridge). |
| `405` on the endpoint | You issued a `GET`. MCP uses `POST /mcp`; this is expected for `GET`. |
| Bridge prints “could not reach Baitler MCP endpoint” | The API isn’t running or `BAITLER_API_URL` is wrong. Start `baitler-api` first. |
| `tools/list` is empty / client shows no tools | Ensure `MCP_ENABLED` isn’t `false`. Re‑run the client’s MCP refresh. |
| “database … lock” on startup | Don’t run a second process against the same `rocksdb://` path. The bridge does **not** touch the DB — only one `baitler-api` opens it. |

---

## Quick manual test

```bash
# Start the server (localhost only):
BIND_HOST=127.0.0.1 PORT=8080 \
  cargo run --release --manifest-path backend/Cargo.toml --bin baitler-api &

# Drive it through the stdio bridge (one JSON-RPC message per line):
printf '%s\n%s\n%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}' \
  '{"jsonrpc":"2.0","method":"notifications/initialized"}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"ideas_create","arguments":{"title":"Hello from MCP","body":"# hi"}}}' \
  | BAITLER_API_URL=http://127.0.0.1:8080 backend/target/release/baitler-mcp
```

You’ll see the `initialize` result, no line for the notification, and the
created idea returned from `ideas_create`.
