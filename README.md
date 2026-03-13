# BuzzClaw

> Fast, small autonomous AI agent infrastructure — deployable anywhere.

BuzzClaw is a self-hosted agent runtime built in Rust. Trait-driven architecture, built-in persistent memory, multi-platform bots (CLI, Telegram, Discord, Nostr), and a workspace system inspired by the best open-source agents. Deploy it on a VPS, a Raspberry Pi, or your laptop — it runs wherever Rust runs.

---

## Features

- **Multiple LLM providers** — OpenAI, Anthropic (Claude), Google Gemini, Kimi (Moonshot), DeepSeek, Buzzster
- **Multi-platform** — interactive CLI chat, Telegram bot, Discord bot, Nostr bot, all from one binary
- **Persistent sessions** — conversation history survives restarts on every platform
- **Workspace memory** — markdown-based identity and memory files (`SOUL.md`, `USER.md`, `MEMORY.md`, `AGENTS.md`, `HEARTBEAT.md`, daily logs)
- **SQLite memory** — key-value semantic memory store for long-term facts
- **Canvas** — persistent markdown artifact documents the agent can create and edit
- **Named agents** — per-channel agents with their own model, provider, and system prompt
- **Skills** — `.md` instruction files loadable per-agent as extra system prompt context
- **Subagents** — the main agent can delegate subtasks to specialized subagents
- **Cron jobs** — scheduled agent tasks without manual triggers
- **Daemon** — systemd user service for always-on deployment
- **Onboard wizard** — interactive first-run setup
- **Single binary** — no runtime dependencies, no Docker required

---

## Requirements

- [Rust](https://rustup.rs/) 1.87 or later
- An API key from at least one supported provider
- (Optional) A Telegram bot token from [@BotFather](https://t.me/BotFather)
- (Optional) A Discord bot token from the [Discord Developer Portal](https://discord.com/developers/applications)
- (Optional) A Nostr keypair (`nsec1...`) for the Nostr bot

---

## Installation

### Build from source

```bash
git clone https://github.com/youruser/buzzclaw
cd buzzclaw
cargo build --release
```

The binary will be at `./target/release/buzzclaw`.

### Install system-wide (optional)

```bash
sudo cp target/release/buzzclaw /usr/local/bin/
```

---

## Quick start

Run the interactive setup wizard:

```bash
buzzclaw onboard
```

This will guide you through:
1. Choosing a provider (OpenAI, Anthropic, Gemini, Buzzster)
2. Setting your API key
3. Selecting a model
4. Configuring Telegram, Discord, and/or Nostr bots (optional)
5. Initializing the workspace files

Then start chatting:

```bash
buzzclaw chat
```

---

## CLI reference

```
buzzclaw [OPTIONS] <COMMAND>

Options:
  -p, --provider <PROVIDER>        Override provider (openai, anthropic, gemini, buzzster)
  -m, --model <MODEL>              Override model
  -t, --temperature <TEMPERATURE>  Override temperature (0.0 – 2.0)
      --debug                      Enable debug logging
```

### Commands

| Command | Description |
|---------|-------------|
| `run "<message>"` | Send a single message and print the response |
| `chat` | Start an interactive chat session |
| `serve` | Start all configured bots + cron runner |
| `telegram` | Start Telegram bot only |
| `discord` | Start Discord bot only |
| `nostr` | Start Nostr bot only |
| `onboard` | Interactive setup wizard |
| `config` | Interactive configuration editor |
| `config show` | Print current configuration |
| `config set <key> <value>` | Set a single config value |
| `agents` | Manage named agents |
| `skills` | Manage skill files |
| `sessions` | Manage persistent conversation sessions |
| `canvas` | Manage canvas documents |
| `cron` | Manage scheduled tasks |
| `memory` | Manage the SQLite memory store |
| `workspace` | Manage workspace markdown files |
| `daemon` | Manage the systemd service |
| `uninstall` | Remove all BuzzClaw data and the daemon |

---

## Configuration

All data lives in `~/.buzzclaw/`:

```
~/.buzzclaw/
├── config.toml          # Main configuration
├── memory.db            # SQLite memory store
├── workspace/           # Markdown identity & memory files
│   ├── AGENTS.md        # Agent operating rules
│   ├── SOUL.md          # Agent identity and personality
│   ├── USER.md          # User profile
│   ├── MEMORY.md        # Long-term memory (auto-updated by agent)
│   ├── HEARTBEAT.md     # Proactive task checklist
│   └── memory/          # Daily log files (YYYY-MM-DD.md)
├── sessions/            # Persistent conversation histories
├── canvas/              # Canvas document artifacts
├── skills/              # Skill .md files
└── cron/                # Cron job definitions
```

### Environment variables

```bash
OPENAI_API_KEY=...
ANTHROPIC_API_KEY=...
GEMINI_API_KEY=...
GOOGLE_API_KEY=...        # alias for Gemini
MOONSHOT_API_KEY=...      # Kimi (Moonshot)
KIMI_API_KEY=...          # alias for Kimi
DEEPSEEK_API_KEY=...
BUZZSTER_API_KEY=...
TELEGRAM_TOKEN=...
DISCORD_TOKEN=...
NOSTR_PRIVATE_KEY=...     # nsec1... or hex
RUSTCLAW_PROVIDER=...     # override provider
RUSTCLAW_MODEL=...        # override model
```

### Interactive config editor

```bash
buzzclaw config
```

### Set individual values

```bash
buzzclaw config set provider anthropic
buzzclaw config set model claude-sonnet-4-6
buzzclaw config set telegram_token <token>
buzzclaw config set discord_token <token>
buzzclaw config set nostr_private_key nsec1...
buzzclaw config set nostr_relays wss://relay.damus.io,wss://nos.lol
buzzclaw config set nostr_allowed_pubkeys npub1...
```

---

## Providers & models

| Provider | Key env var | Example models |
|----------|-------------|----------------|
| `openai` | `OPENAI_API_KEY` | `gpt-4o`, `gpt-4o-mini`, `o1` |
| `anthropic` | `ANTHROPIC_API_KEY` | `claude-opus-4-6`, `claude-sonnet-4-6`, `claude-haiku-4-5` |
| `gemini` | `GEMINI_API_KEY` | `gemini-2.0-flash`, `gemini-1.5-pro` |
| `kimi` | `MOONSHOT_API_KEY` | `kimi-k2-turbo-preview` |
| `deepseek` | `DEEPSEEK_API_KEY` | `deepseek-chat`, `deepseek-reasoner` |
| `buzzster` | `BUZZSTER_API_KEY` | (Buzzster-specific) |

---

## Sessions

Persist conversation history between runs:

```bash
# Named session — history saved to ~/.buzzclaw/sessions/work.json
buzzclaw chat --session work

# Single run in a session context
buzzclaw run "summarize what we discussed" --session work
```

Manage sessions:

```bash
buzzclaw sessions list
buzzclaw sessions show work
buzzclaw sessions delete work
```

Telegram, Discord, and Nostr sessions are **automatically persistent** — each user/channel gets its own session file (`telegram_<id>.json`, `discord_<id>.json`, `nostr_<pubkey>.json`). Use `/clear` inside the chat to reset.

---

## Workspace

The workspace is a set of markdown files the agent reads on every session to remember who it is and who you are.

```bash
# Initialize default workspace files
buzzclaw workspace init

# Edit your identity
buzzclaw workspace edit SOUL.md

# Edit your user profile
buzzclaw workspace edit USER.md

# Print what the agent sees as system context
buzzclaw workspace context

# View daily memory logs
buzzclaw workspace logs
```

The agent can write to these files itself using the `workspace_write` tool.

---

## Canvas

Canvas documents are persistent markdown artifacts — drafts, reports, code, plans — that the agent can create and update across sessions.

```bash
buzzclaw canvas list
buzzclaw canvas show my-report
buzzclaw canvas edit my-report
buzzclaw canvas delete my-report
```

The agent has access to `canvas_read`, `canvas_write`, and `canvas_list` tools automatically.

---

## Named agents

Create agents with custom configurations:

```bash
# Create an agent
buzzclaw agents add researcher \
  --system-prompt "You are a research assistant. Be thorough and cite sources." \
  --model gpt-4o \
  --provider openai

# Bind it to a Telegram chat
buzzclaw agents bind researcher telegram:123456789

# Or set a default for all Telegram chats
buzzclaw agents bind researcher telegram:default

# Same for Discord
buzzclaw agents bind researcher discord:987654321
```

Each Telegram user, Discord channel, and Nostr pubkey can have its own agent with its own configuration.

---

## Skills

Skills are `.md` instruction files that extend an agent's capabilities:

```bash
# Create a blank skill
buzzclaw skills new coding

# Edit it
buzzclaw workspace edit  # or $EDITOR ~/.buzzclaw/skills/coding.md

# Assign to an agent
buzzclaw skills assign coding researcher

# List all skills
buzzclaw skills list
```

---

## Subagents

The main agent can delegate subtasks to specialized subagents via the `spawn_agent` tool:

```
spawn_agent(
  prompt: "Research the latest papers on RAG and summarize them",
  agent: "researcher",      // optional: use a named agent's config
  system_prompt: "...",     // optional: inline system prompt
  model: "gpt-4o-mini"      // optional: cheaper model for simple tasks
)
```

Subagents run with full tool access (web, files, memory, canvas, workspace). They cannot spawn further subagents (depth = 1).

---

## Cron jobs

Schedule recurring agent tasks:

```bash
# Every 30 minutes
buzzclaw cron add check-news \
  --prompt "Search for the latest AI news and save a summary to canvas/news.md" \
  --schedule "every 30m"

# Daily at 9am
buzzclaw cron add morning-brief \
  --prompt "Review my MEMORY.md and today's log, then prepare a daily brief" \
  --schedule "daily 09:00"

# List, enable, disable, run immediately
buzzclaw cron list
buzzclaw cron disable check-news
buzzclaw cron run morning-brief
```

Cron jobs run automatically when BuzzClaw starts (`serve`, `telegram`, `discord`, or `nostr` commands). Results are logged to the workspace daily log.

---

## Telegram bot

1. Create a bot with [@BotFather](https://t.me/BotFather) → get a token
2. Find your chat ID by messaging [@userinfobot](https://t.me/userinfobot)

```bash
buzzclaw config set telegram_token <your-token>
buzzclaw config set telegram_allowed_ids <your-chat-id>

# Start the bot
buzzclaw telegram
```

In-chat commands:
- `/clear` — reset conversation history

---

## Discord bot

1. Create an application at [discord.com/developers](https://discord.com/developers/applications)
2. Add a Bot, copy the token
3. Enable **Message Content Intent** under Bot → Privileged Gateway Intents
4. Invite the bot to your server with `bot` + `applications.commands` scopes

```bash
buzzclaw config set discord_token <your-token>

# Start the bot
buzzclaw discord
```

The bot responds to:
- Direct messages
- `@mentions` in server channels

In-chat commands:
- `/clear` — reset conversation history for that channel

---

## Nostr bot

The Nostr bot listens for **NIP-04 encrypted direct messages** (kind 4) sent to its pubkey and replies with the same encryption.

1. Generate a Nostr keypair with any Nostr client or tool
2. Configure the bot:

```bash
buzzclaw config set nostr_private_key nsec1...

# Optional: restrict access to specific pubkeys
buzzclaw config set nostr_allowed_pubkeys npub1...,npub1...

# Optional: override default relays
buzzclaw config set nostr_relays wss://relay.damus.io,wss://nos.lol

# Start the bot
buzzclaw nostr
```

Default relays (used when none are configured):
- `wss://relay.0xchat.com`
- `wss://relay.damus.io`
- `wss://nos.lol`
- `wss://relay.nostr.band`

In-chat commands:
- `/clear` — reset conversation history for that pubkey

You can bind named agents to specific Nostr pubkeys:

```bash
buzzclaw agents bind researcher nostr:<pubkey-hex>
buzzclaw agents bind researcher nostr:default
```

---

## Run all services at once

```bash
buzzclaw serve
```

This starts all configured bots (Telegram + Discord + Nostr) and the cron runner simultaneously.

---

## Daemon (systemd)

Install BuzzClaw as a systemd user service that starts on login and restarts on failure:

```bash
# Install and enable
buzzclaw daemon install
buzzclaw daemon start

# Manage
buzzclaw daemon status
buzzclaw daemon logs
buzzclaw daemon restart
buzzclaw daemon stop

# Remove
buzzclaw daemon uninstall
```

The service runs `buzzclaw serve`, so it starts all configured bots automatically.

---

## Full uninstall

```bash
# Removes ~/.buzzclaw and the systemd service
buzzclaw uninstall
```

---

## Tools available to the agent

| Tool | Description |
|------|-------------|
| `shell` | Run shell commands |
| `file_read` | Read any file |
| `file_write` | Write to any file |
| `web_fetch` | Fetch a URL and extract text |
| `memory_recall` | Search SQLite memory |
| `memory_store` | Save to SQLite memory |
| `workspace_read` | Read a workspace markdown file |
| `workspace_write` | Write/append to a workspace markdown file |
| `canvas_read` | Read a canvas document |
| `canvas_write` | Write/append to a canvas document |
| `canvas_list` | List available canvas documents |
| `spawn_agent` | Delegate a task to a subagent |

---

## License

MIT
