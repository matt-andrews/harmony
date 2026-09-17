# Harmony

A tiny, single-user time tracker for contract work. One button to start
and stop, sessions tagged with a project, projects grouped into ephemeral
"tasks" (one per pickup), running totals and pay, and a simple period report.

Rust (axum) backend serving an embedded Svelte 5 frontend. Persists to a
single JSON document in Azure Blob Storage. Runs locally in Docker. No auth,
dark theme, nothing else.

## Concepts

| Thing | What it is |
|---|---|
| **Session** | A block of work with a start and end. Start / Stop. Only one can run at a time. Can be untagged. |
| **Project** | A reusable label: a client or gig. Has an hourly rate and an auto-assigned colour. |
| **Task** | One *pickup* of a project. When you pick up "contoso" again after turning it in, that's a new task. Sessions are filed under a task; the first session of a task carries a `NEW TASK` badge and cards read `task #3 · session 2 of 4`. |

Tagging a session with a project appends it to the project's current (latest)
task. Tick **New task** in the picker to start a fresh one. Tasks that end up
with no sessions (after retagging or deleting) are pruned automatically.

Pay is `hours × the project's current rate`. Rates are not versioned, so
changing a rate re-prices that project's history.

## Running it

### 1. Azure storage

Harmony talks to Blob Storage with a **container-level SAS URL**. The
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

Open <http://localhost:8080>. The image is a multi-stage build (Node builds
the frontend, Rust embeds it, Debian slim runs it).

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

## Configuration

| Variable | Meaning | Default |
|---|---|---|
| `HARMONY_STORAGE` | `sas:<container SAS URL>` or `file:<path>` | required |
| `HARMONY_BIND` | Listen address | `127.0.0.1:8080` (Docker sets `0.0.0.0:8080`) |
| `RUST_LOG` | Log filter | `harmony=info,tower_http=info` |

## Data

Everything lives in one document, `harmony.json`, in the container:

```json
{
  "version": 1,
  "projects": [{ "id": "…", "name": "contoso", "hourly_rate": 40.0, "color": "#f97316", "created_at": "…", "archived": false }],
  "tasks":    [{ "id": "…", "project_id": "…", "number": 1, "created_at": "…" }],
  "sessions": [{ "id": "…", "task_id": "…", "started_at": "…", "ended_at": "…", "note": "…" }]
}
```

The server loads it once at startup and writes it back on every change with
an `If-Match` ETag, so two instances pointed at the same container can't
silently clobber each other (the loser gets a 409 and should be restarted).
The first write of each UTC day also drops a copy at `backups/<date>.json`.

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
| GET | `/api/report?from=<iso>&to=<iso>` | — |

## Tests

```bash
cargo test
cargo clippy --all-targets -- -D warnings
npm --prefix web run check
```

## Layout

```
src/domain/    model, mutations (ops), read models (views), colour palette
src/storage/   Storage enum: blob (Azure SAS) and file backends
src/api/       axum handlers + AppState (mutate-copy-save-commit)
src/frontend.rs  embedded web/dist with SPA fallback
web/           Svelte 5 + Vite + TypeScript
scripts/       azure-setup.ps1
```
