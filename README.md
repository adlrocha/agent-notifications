# Agent Inbox

A CLI-first notification system that tracks tasks across multiple LLM/coding agents (Claude Web, Gemini Web, Claude Code, OpenCode, etc.) and provides a simple inbox-style dashboard to view tasks requiring attention.

## Features

- **Unified Task Tracking**: Track tasks across different AI agents in one place
- **Transparent Wrappers**: Auto-track CLI agents without changing your workflow
- **Parallel Tracking**: Monitor multiple instances of the same agent simultaneously
- **Simple CLI Interface**: View, manage, and monitor tasks from the command line
- **Automatic Cleanup**: Completed tasks auto-delete after 1 hour (configurable)
- **Background Monitoring**: Detect when tasks need attention (stdin, stalls)
- **SQLite Backend**: Fast, reliable, and concurrent-safe storage
- **Extensible**: Easy template for wrapping any CLI coding agent

## Installation

### Prerequisites

- Rust 1.70+ (for building)
- Linux (tested on Arch Linux)

### Build and Install

```bash
# Clone the repository
cd /path/to/agent-notifications

# Install
./install.sh
```

This will:
1. Build the release binary
2. Install `agent-inbox` to `/usr/local/bin/` (or `~/.local/bin/` if no sudo)
3. Create `~/.agent-tasks/` directory

### Phase 2: Set Up Wrappers (Automatic Tracking)

After installing the core CLI, set up wrappers to automatically track your coding agents:

```bash
# Set up wrappers for detected agents
./setup-wrappers.sh

# Reload your shell
source ~/.bashrc  # or ~/.zshrc

# Test it
claude --help  # Should now be tracked
agent-inbox list --all
```

The setup script will:
1. Detect which agents are installed (claude, opencode, etc.)
2. Install wrapper scripts to `~/.agent-tasks/wrappers/`
3. Add transparent aliases to your shell RC file
4. Create backups of original binaries

**Supported agents out of the box:**
- Claude Code (`claude`)
- OpenCode (`opencode`)

**Want to wrap other agents?** See [WRAPPING_AGENTS.md](WRAPPING_AGENTS.md) for a complete guide on wrapping Cursor, Aider, Windsurf, or any other CLI agent.

### Phase 3: Install Browser Extension (Optional - For Web LLMs)

To track Claude.ai and Gemini conversations:

```bash
# Install extension and native messaging host
./install-extension.sh

# Follow prompts to:
# 1. Load extension in browser (chrome://extensions)
# 2. Copy extension ID
# 3. Update native messaging manifest
# 4. Reload extension
```

See [EXTENSION.md](EXTENSION.md) for detailed installation guide and troubleshooting.

## Usage

### Basic Commands

```bash
# Show tasks needing attention (default)
agent-inbox

# List all tasks
agent-inbox list --all

# List tasks by status
agent-inbox list --status running
agent-inbox list --status needs_attention
agent-inbox list --status completed
agent-inbox list --status failed

# Show detailed task information
agent-inbox show <task-id>

# Clear a specific task
agent-inbox clear <task-id>

# Clear all completed and failed tasks
agent-inbox clear-all

# Watch tasks in real-time (refreshes every 2s)
agent-inbox watch

# Manual cleanup of old completed tasks
agent-inbox cleanup --retention-secs 3600
```

### Manual Task Reporting (Phase 1)

You can manually report task status using the `report` subcommand:

```bash
# Start a task
TASK_ID=$(uuidgen)
agent-inbox report start "$TASK_ID" "claude_code" "$PWD" "My task description" --pid $$ --ppid $PPID

# Mark task as needing attention
agent-inbox report needs-attention "$TASK_ID" "Waiting for user input"

# Complete a task
agent-inbox report complete "$TASK_ID" --exit-code 0

# Report task failure
agent-inbox report failed "$TASK_ID" 1
```

## Architecture

```
┌─────────────────────────────────┐
│     User CLI Commands           │
└───────────┬─────────────────────┘
            │
    ┌───────▼────────┐
    │  agent-inbox   │  (Rust binary)
    └───────┬────────┘
            │
    ┌───────▼────────┐
    │  SQLite DB     │  (~/.agent-tasks/tasks.db)
    └────────────────┘
```

### Database Schema

```sql
CREATE TABLE tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id TEXT UNIQUE NOT NULL,          -- UUID
    agent_type TEXT NOT NULL,               -- 'claude_web', 'gemini_web', 'claude_code', etc.
    title TEXT NOT NULL,                    -- First 100 chars of prompt
    status TEXT NOT NULL,                   -- 'running', 'completed', 'needs_attention', 'failed'
    created_at INTEGER NOT NULL,            -- Unix timestamp
    updated_at INTEGER NOT NULL,
    completed_at INTEGER,                   -- Timestamp when finished
    pid INTEGER,                            -- Process ID for CLI tools
    ppid INTEGER,                           -- Parent process ID
    monitor_pid INTEGER,                    -- Background monitor process ID
    attention_reason TEXT,                  -- Why it needs attention
    exit_code INTEGER,                      -- Exit code when completed/failed
    context TEXT,                           -- JSON: {url, project_path, session_id}
    metadata TEXT                           -- JSON: agent-specific data
);
```

### Task States

- **running**: Task in progress
- **completed**: Finished successfully (auto-cleared after 1 hour)
- **needs_attention**: Waiting for user input/approval
- **failed**: Errored out

## Development

### Run Tests

```bash
cargo test
```

All tests should pass:
- Unit tests for task model
- Unit tests for database operations
- Integration tests for CLI commands

### Build for Development

```bash
cargo build
./target/debug/agent-inbox --help
```

## Configuration

Default configuration (future):
- **Database**: `~/.agent-tasks/tasks.db`
- **Cleanup retention**: 3600 seconds (1 hour) for completed tasks
- **Wrappers directory**: `~/.agent-tasks/wrappers/`

## Contributing

This is a personal project, but suggestions and improvements are welcome!

## License

Apache 2.0

