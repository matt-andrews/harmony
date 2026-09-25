# Harmony

A tiny, single-user time tracker for contract work. One button to start
and stop, sessions tagged with a project, projects grouped into ephemeral
"tasks" (one per pickup), running totals and pay, and a simple period report.

Rust (axum) backend serving an embedded Svelte 5 frontend. Persists to a
single JSON document, either in a local folder (a mounted volume in Docker)
or in Azure Blob Storage. Runs locally in Docker. No auth, dark theme,
nothing else.

## Concepts

| Thing | What it is |
|---|---|
| **Session** | A block of work with a start and end. Start / Stop. Only one can run at a time. Can be untagged. |
| **Project** | A reusable label: a client or gig. Has an hourly rate and an auto-assigned colour. |
| **Task** | One *pickup* of a project. When you pick up "contoso" again after turning it in, that's a new task. Sessions are filed under a task; the first session of a task carries a `NEW TASK` badge and cards read `task #3 · session 2 of 4`. |

Tagging a session with a project appends it to the project's current (latest)
task. Tick **New task** in the picker to start a fresh one. Tasks that end up
with no sessions (after retagging or deleting) are pruned automatically.

A task is **done** once you turn it in: press *Done* on its latest session
card. Starting a new task on the same project marks the previous one done
by itself. A done task is closed, so *Start* on that project opens the next
task without asking; *Reopen* takes it back. The completion time is the
task's last session end, which is what the payout report goes by.

Pay is `hours × the project's current rate`. Rates are not versioned, so
changing a rate re-prices that project's history, past payouts included.

The **Report** tab has a pay-cycle calendar: pick the date a cycle starts on
and *Week* / *2 Weeks* line up with it. It also shows the **payout** for the
period, i.e. the tasks whose pay becomes withdrawable in it, given the number
of days pay takes to finalize after a task is done (default 7). Both settings
are stored with the data, so they follow you between the desktop app and
Docker. The version in the bottom-right corner is the release tag; local
builds say `dev`.

## Running it

### 1. Pick a storage backend

One env var, `HARMONY_STORAGE`, chooses where the data lives. The backend is
inferred from the value; both backends use the same layout (`harmony.json`
at the root, daily copies under `backups/`).

| `HARMONY_STORAGE` | Backend |
|---|---|
| `file:<dir>` (or any bare path) | Local folder. `file:./data` for `cargo run`, `file:/data` in Docker (mounted from `./data`). |
| `sas:<url>` (or a bare `https://` URL) | Azure Blob container, via a container-level SAS URL. |

**Local folder** needs nothing else. Create `.env` from `.env.example` and go.

**Azure** talks to Blob Storage with a **container-level SAS URL**. The
official Rust SDK supports Entra ID tokens and SAS URLs only, so there is no
account-key / connection-string mode.

```powershell
az login
./scripts/azure-setup.ps1 -WriteEnv
```

That creates resource group `rg-harmony` (eastus2), a Standard_LRS StorageV2
account, a private `harmony` container, and a SAS valid until 2031, then
writes `.env`. Costs are a few cents a month.

To rotate or regenerate the SAS later:

```powershell
az storage container generate-sas --account-name <acct> --name harmony --permissions racwdl --expiry 2035-01-01 --https-only --account-key (az storage account keys list --account-name <acct> --query "[0].value" -o tsv) -o tsv
```

and put `HARMONY_STORAGE=sas:https://<acct>.blob.core.windows.net/harmony?<sas>` in `.env`.

### 2. Docker

```bash
docker compose up --build -d
```

Open <http://localhost:31415>. The compose file mounts `./data` at `/data`
inside the container, so `HARMONY_STORAGE=file:/data` in `.env` keeps the
data on the host at `./data/harmony.json`; with a `sas:` value the mount is
simply unused. The image is a multi-stage build (Node builds the frontend,
Rust embeds it, Debian slim runs it as a non-root user). CI passes the release
tag as the `PACKAGE_VERSION` build-arg, which becomes the version shown in
the UI; a plain `docker compose up --build` shows `dev`.

### 3. Local development

```bash
cp .env.example .env          # defaults to a local file backend
cargo run                     # http://127.0.0.1:8080
```

For frontend work with hot reload, in a second terminal:

```bash
npm --prefix web install
npm --prefix web run dev      # http://localhost:5173, proxies /api to :8080
```

Before running the Rust server standalone, build the frontend once so it has
something to serve: `npm --prefix web run build`. Debug builds read
`web/dist` from disk on every request; release builds embed it.

### 4. Desktop app

`harmony-desktop` is a native window (system WebView2 on Windows) that runs
the same server in-process on a random localhost port. No Docker needed.

```bash
cargo run -p harmony-desktop                      # dev
cargo build --release -p harmony-desktop          # -> target/release/harmony-desktop.exe
```

- Storage comes from `HARMONY_STORAGE` (environment, or a `.env` next to the
  exe, or in the working directory). When unset it defaults to a local folder:
  `%LOCALAPPDATA%\Harmony` on Windows (`~/.local/share/Harmony` on Linux,
  `~/Library/Application Support/Harmony` on macOS).
- `harmony-desktop --url http://localhost:31415` skips the embedded server and
  just shows a running instance, e.g. the Docker one.
- **Compact mode**: the `⤡ Compact` button collapses the window into a small
  borderless strip that floats above other windows, showing the project, the
  session clock, the task total and Start/Stop. Drag it anywhere by its body;
  `⤢` brings the full window back. Tick *Float compact while a session runs*
  to collapse automatically on Start. The preference and the strip's position
  are kept in `desktop-settings.json` in the local data folder above, whatever
  `HARMONY_STORAGE` says (they belong to this machine, not to the data).
- Release builds hide the console and log to `harmony-desktop.log` in that
  data folder. If startup fails, the window shows the error and the log path.
- The UI shows the version from the `HARMONY_VERSION` environment variable
  *at compile time* (CI sets it from the tag); a local build shows `dev`.
- Windows 11 ships the WebView2 runtime; on older Windows install it from
  Microsoft. `scripts/make-icons.py` regenerates the icons if you change the mark.

## Configuration

| Variable | Meaning | Default |
|---|---|---|
| `HARMONY_STORAGE` | `file:<dir>` / bare path, or `sas:<container SAS URL>` / bare `https://` URL | required |
| `HARMONY_BIND` | Listen address | `127.0.0.1:8080` (Docker sets `0.0.0.0:8080`) |
| `RUST_LOG` | Log filter | `harmony=info,tower_http=info` |

## Data

Everything lives in one document, `harmony.json`, at the root of the folder
or container:

```json
{
  "version": 2,
  "projects": [{ "id": "…", "name": "contoso", "hourly_rate": 40.0, "color": "#f5a97f", "created_at": "…", "archived": false }],
  "tasks":    [{ "id": "…", "project_id": "…", "number": 1, "created_at": "…", "completed": true }],
  "sessions": [{ "id": "…", "task_id": "…", "started_at": "…", "ended_at": "…", "note": "…" }],
  "settings": { "cycle_start": "2026-09-24", "payout_delay_days": 7 }
}
```

The server loads it once at startup and writes it back on every change with
a version check (an `If-Match` ETag on Azure, the file's mtime and size
locally), so two instances pointed at the same store, or a hand edit while
the app runs, can't be silently clobbered: the write fails with a 409 and
the app should be restarted to reload. The first write of each UTC day also
drops a copy at `backups/<date>.json`.

## API

All responses are JSON. Mutations return the full state so the UI can
replace what it has.

| Method | Path | Body |
|---|---|---|
| GET | `/api/state` | — |
| POST | `/api/projects` | `{name, hourly_rate?}` |
| PATCH | `/api/projects/{id}` | `{name?, hourly_rate?, color?, archived?}` |
| GET | `/api/projects/{id}/summary` | — |
| POST | `/api/sessions/start` | `{project_id?, new_task?}` |
| POST | `/api/sessions/stop` | — |
| PATCH | `/api/sessions/{id}` | `{project_id?: uuid\|null, new_task?, task_id?, started_at?, ended_at?: iso\|null, note?}` |
| DELETE | `/api/sessions/{id}` | — |
| POST | `/api/tasks/{id}/complete` | — |
| POST | `/api/tasks/{id}/reopen` | — |
| PATCH | `/api/settings` | `{cycle_start?: "YYYY-MM-DD"\|null, payout_delay_days?}` |
| GET | `/api/report?from=<iso>&to=<iso>[&completed_from=<iso>&completed_to=<iso>]` | — |

The report's optional `completed_*` window selects the tasks whose pay is
withdrawable in `[from, to)`; the UI sends the period shifted back by the
payout delay. Tasks done after the window but before `to` come back as
"carried" to the next payout.

## Tests

```bash
cargo test
cargo clippy --all-targets -- -D warnings
npm --prefix web run check
```

## Layout

```
src/domain/    model, mutations (ops), read models (views), colour palette
src/storage/   Storage enum: file (local dir / volume) and blob (Azure SAS) backends
src/api/       axum handlers + AppState (mutate-copy-save-commit)
src/frontend.rs  embedded web/dist with SPA fallback
web/           Svelte 5 + Vite + TypeScript
scripts/       azure-setup.ps1
```
