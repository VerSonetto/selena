# Selena

[![CI](https://github.com/VerSonetto/selena/actions/workflows/ci.yml/badge.svg)](https://github.com/VerSonetto/selena/actions/workflows/ci.yml)

Selena is a local-first API confluence proxy and control room for three LLM protocols:

- OpenAI Chat Completions
- OpenAI Responses
- Anthropic Messages

Every protocol can be an **ingress** and an **upstream provider**. Same-protocol traffic is passed through and observed; cross-protocol traffic is converted through a canonical IR with explicit, fail-fast capability checks. There is no fixed Chat Completions egress.

The control room discovers each provider's model catalog from its `/models` endpoint, caches it for 120 seconds, and exposes a dedicated model pool page plus per-provider model lists.

## Three surfaces, one core

- `selena` — full-screen terminal UI (ratatui)
- `selena web` — web control room served by the local axum server
- `selena desktop` — Tauri v2 desktop shell with a custom title bar

Web and desktop share the same React build. The desktop WebView loads the same loopback server, so SSE and API behavior are identical.

## Commands

```bash
selena                 # terminal control room
selena web             # web control room
selena desktop         # Tauri desktop app
selena serve           # headless gateway
selena status
selena doctor
selena config
```

Subcommands are case-insensitive: `SELENA WEB`, `Selena Desktop`, `SELENA STATUS`.

On Unix installers create `Selena` and `SELENA` symlinks so the executable name itself is case-insensitive too.

## Protocol matrix

| ingress \ provider | Chat | Responses | Messages |
|---|---|---|---|
| Chat | passthrough | convert | convert |
| Responses | convert | passthrough | convert |
| Messages | convert | convert | passthrough |

Same-protocol requests preserve native SSE and native-only features. Cross-protocol native-only features (`previous_response_id`, active hosted/server tool definitions, audio, `n>1`) return a structured `unsupported_feature` error instead of being silently faked. Anthropic thinking blocks, signatures, and historical server-tool blocks are opaque and provider-bound, so they are dropped with a conversion warning when crossing protocols; Selena never fabricates or replays them.

See [docs/architecture.md](docs/architecture.md), [docs/protocol-research.md](docs/protocol-research.md), [docs/api.md](docs/api.md), and [docs/design-system.md](docs/design-system.md).

## Development

Requirements: Rust stable, Node 22+, pnpm, and WebView2 on Windows.

```bash
cargo test --workspace
cd web && pnpm install && pnpm build
```

Run the backend and Vite dev server:

```bash
cargo build -p selena-cli
./scripts/dev.ps1
```

The default data home is `~/.selena` (override with `SELENA_HOME`). The server binds `127.0.0.1:13697` by default. Set a custom port with `selena web --port <port>` or persist it in Settings → 服务与路由.

## Clone

```bash
git clone https://github.com/VerSonetto/selena.git
cd selena
```

## Releases

- Push to `main` updates the [nightly prerelease](https://github.com/VerSonetto/selena/releases/tag/nightly).
- Push a version tag such as `v0.1.0` to publish a stable release.

Both include the Windows NSIS installer and `SHA256SUMS.txt`. The release workflow is Windows-only by design; PR CI runs the fast Rust/web checks on Ubuntu.

## Windows installer

```powershell
./scripts/build-desktop.ps1
```

This builds the console CLI, stages it as a Tauri resource, and produces an NSIS installer under `target/release/bundle/nsis/`. The installer copies `selena.exe` beside `selena-desktop.exe` and adds the install directory to the current user's PATH.

## Design

The shared UI is **Ivory Studio**: warm paper, editorial typography, coral annotations, teal live states, and a Dusk companion. Theme mode supports `system`, `ivory`, and `dusk`; `system` follows the OS appearance live. The desktop title bar is intentionally logo-free so it does not compete with the sidebar wordmark, and it is fixed above the scroll container.

## License

MIT. See [LICENSE](LICENSE).
