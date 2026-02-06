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

1. Create a `servinel-compose.yaml` file (see `examples/servinel-compose.yaml`).

2. Start all services and open the dashboard:
```bash
servinel up --file examples/servinel-compose.yaml
```

3. View the dashboard separately:
```bash
servinel dash
```

4. Stop and remove services:
```bash
servinel down --file examples/servinel-compose.yaml
```

## CLI Commands

### General
- `servinel up` - Launch services and (optionally) the TUI.
  - `--file <path>`: Specify compose file.
  - `--profile <name>`: Start specific profile.
  - `--no-tui`: Start in background without dashboard.
- `servinel down` - Stop and remove apps/services.
  - `--file <path>`: Use compose file to identify app.
  - `--app <name>`: Specify app name directly.
- `servinel dash` - Open the TUI dashboard for running services.
- `servinel status` - Show status of services.
- `servinel logs <service>` - View or stream logs.
  - `--follow`: Stream logs.
  - `--tail <n>`: Show last N lines.
  - `--merged`: Merge logs from all instances (for profiles).
- `servinel profiles` - List available profiles.
- `servinel doctor` - Run diagnostic checks on the daemon.

### Service Management
- `servinel start <service>` - Start a specific service.
- `servinel stop <service>` - Stop a specific service.
- `servinel restart <service>` - Restart a specific service.

## TUI Controls

### Navigation
| Key | Action |
| :--- | :--- |
| `Tab` / `Shift+Tab` | Switch between Apps |
| `Left` / `Right` | Switch between Services |
| `q` | Quit Dashboard (leaves services running) |

### Log Scrolling
| Key | Action |
| :--- | :--- |
| `Up` / `Down` | Scroll Logs Vertically |
| `Shift` + `Up` / `Down` | Scroll Logs **Horizontally** |
| `PageUp` / `PageDown` | Scroll by Page |
| `Home` | Jump to Top |
| `End` | Jump to Bottom (enables autoscroll) |
| **Mouse Wheel** | Scroll Logs Vertically |
| **Shift** + **Mouse Wheel** | Scroll Logs **Horizontally** (if supported by terminal) |

### Actions
| Key | Action |
| :--- | :--- |
| `s` | Start selected service |
| `x` | Stop selected service |
| `r` | Restart selected service |

## Installation

```bash
cargo install servinel
```

## License

MIT