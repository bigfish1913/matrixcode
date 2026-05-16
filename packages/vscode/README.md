# MatrixCode VSCode Extension

A VSCode extension that integrates MatrixCode CLI as a sidebar AI assistant.

## Features

- **Sidebar Chat Interface**: Chat with MatrixCode directly in VSCode sidebar
- **Code Actions**: Right-click on code to explain, fix, refactor, or generate tests
- **Auto Context**: Automatically includes current file and selection as context
- **Streaming Responses**: Real-time streaming of AI responses
- **Session Management**: Create new sessions, view history
- **Configurable**: Full settings integration with VSCode

## Installation

### Prerequisites

1. Install MatrixCode CLI from `../cli/`:
   ```bash
   cd ../cli
   cargo build --release
   # or
   npm install -g matrixcode
   ```

2. Configure API key in `~/.matrix/config.json` or environment variables.

### Install Extension

From source:
```bash
npm install
npm run compile
```

Then in VSCode: Debug > Start Debugging (F5)

## Development

```bash
# Install dependencies
npm install

# Build
npm run compile

# Watch mode
npm run watch

# Package
npm run package
```

## Configuration

Open VSCode settings and search for "MatrixCode":

| Setting | Description | Default |
|---------|-------------|---------|
| `matrixcode.cliPath` | Path to CLI binary | `matrixcode` |
| `matrixcode.provider` | LLM provider | `anthropic` |
| `matrixcode.model` | Model name | `claude-sonnet-4-20250514` |
| `matrixcode.autoContext` | Auto file context | `true` |

## Architecture

```
packages/vscode/
├── src/
│   ├── extension.ts       # Entry point
│   ├── chatView.ts        # Sidebar webview
│   ├── matrixcodeClient.ts # CLI communication
│   └── configManager.ts   # Settings
├── package.json           # Extension manifest
└── dist/                  # Compiled output
```

## Related

- [MatrixCode CLI](../cli/) - The CLI backend
- [Documentation](../../docs/) - Full documentation

## License

MIT License