# Servinel

A service manager and supervisor for applications defined in YAML compose files. Provides background process management, real-time monitoring, and a tabbed TUI dashboard.

## Features

- Background process management via daemon
- Real-time service monitoring and metrics
- Tabbed TUI dashboard for visualization
- YAML-based compose file configuration
- Profile support for batch operations
- Cross-platform (Linux/macOS)

## Quick Start

1. Create a `servinel-compose.yaml` file (see `examples/servinel-compose.yaml`):

2. Start all services:
```bash
servinel up
```

3. View the dashboard:
```bash
servinel dash
```

## Commands

- `up` - Launch all services
- `start/stop/restart` - Manage individual services or profiles
- `status` - Show service status
- `logs` - View service logs
- `profiles` - List available profiles
- `dash` - Open TUI dashboard
- `doctor` - Diagnostic tools

## Installation

```bash
cargo install servinel
```

## License

MIT