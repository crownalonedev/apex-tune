# Apex Tune

A reversible Windows 11 gaming optimizer: Tauri 2 + Rust backend, React 18 frontend.

> Work in progress. The design and the full tweak catalog are in [docs/PLAN.md](docs/PLAN.md).
> This README gets its full version (every tweak and how to revert it, known limitations) in the final stage.

## Requirements (Windows 11 PC)

- [Rust](https://rustup.rs) stable, MSVC toolchain (`x86_64-pc-windows-msvc`)
- Visual Studio 2022 Build Tools with the "Desktop development with C++" workload
- Node.js 20+ and [pnpm](https://pnpm.io)
- WebView2 runtime (preinstalled on Windows 11)

## Run in development

Apex Tune's manifest requires administrator rights, so Windows refuses to start it
from a normal terminal (error 740). Open **Terminal (Admin)** first, then:

```powershell
pnpm install
pnpm tauri dev
```

## Checks

```powershell
pnpm test                 # Vitest
pnpm typecheck            # TypeScript
cd src-tauri
cargo test --workspace    # Rust unit tests (do not need admin)
cargo clippy --workspace --all-targets -- -D warnings
```

The frontend also runs in a plain browser with `pnpm dev` (http://localhost:1420).
There it shows labelled preview data and never touches the system.

## Where things are stored

`%ProgramData%\ApexTune\` holds `backups\`, `logs\` and `profiles\`. The folder is
restricted so only Administrators and SYSTEM can write to it, because backups
decide what the elevated app writes back during a revert.
