# @bigfishnpm/matrixcode

AI coding assistant CLI with multi-model support, intelligent context compression, and cross-session memory.

## Installation

```bash
npm install -g @bigfishnpm/matrixcode
```

The package automatically downloads the pre-built binary for your platform:
- Windows (x64)
- macOS (x64, arm64)
- Linux (x64, arm64)

## Usage

### Interactive Terminal Mode

```bash
matrixcode
```

Start an interactive terminal UI with:
- Streaming response rendering
- Markdown formatting
- Code syntax highlighting
- Tool call visualization

### One-shot Query

```bash
matrixcode "Analyze this project structure"

# JSON output mode (for scripting)
matrixcode --mode service "Explain this function"
```

### Daemon Mode

For VS Code extension integration:

```bash
matrixcode --mode daemon
```

Send JSON requests via stdin:
```bash
echo '{"type":"chat","content":"test"}' | matrixcode --mode daemon
```

### Session Management

```bash
# List previous sessions
matrixcode --list-sessions

# Resume a session
matrixcode --resume
```

## Configuration

Create `~/.matrix/config.json`:

```json
{
  "provider": "anthropic",
  "apiKey": "your-api-key",
  "model": "claude-sonnet-4-20250514",
  "maxTokens": 16384,
  "think": true
}
```

Or use environment variables:

```bash
export PROVIDER=anthropic
export API_KEY=your-key
export MODEL=claude-sonnet-4-20250514
```

## Features

### 🤖 Multi-Model Support
- Anthropic Claude (Sonnet, Opus, Haiku)
- OpenAI GPT (GPT-4, GPT-3.5)
- Flexible model configuration for different tasks

### 🧠 Cross-Session Memory
- SQLite persistent storage
- Automatic memory extraction from conversations
- Keyword-triggered retrieval
- User preferences, project context, learning records

### 🗜️ Intelligent Context Compression
- Multi-phase compression strategy
- Dependency analysis and importance scoring
- Tool call result compression
- Session summarization

### 🔧 Rich Tool System
- File operations: read, write, edit, glob, grep
- Code execution: bash (sandboxed)
- Code intelligence: codegraph (tree-sitter based)
- Web capabilities: webfetch, websearch
- Task management: todo_write, task

### 📋 Workflow Engine
- YAML-defined workflows
- Multiple node types: AI, tool, condition, validate
- Failure strategies: retry, skip, fail, fallback

## CLI Options

```
matrixcode [OPTIONS] [MESSAGE]

Options:
  --mode <MODE>           Run mode: terminal, tui, service, json, daemon
  --resume, -r            Interactively select session to resume
  --session <ID>          Resume from specific session
  --list-sessions         List previous sessions
  --config <PATH>         Configuration file path
  --help                  Show help
  --version               Show version
```

## Links

- [GitHub](https://github.com/bigfish1913/matrixcode)
- [Documentation](https://github.com/bigfish1913/matrixcode#readme)
- [VS Code Extension](https://marketplace.visualstudio.com/items?itemName=bigfish1913.matrixcode)

## License

MIT