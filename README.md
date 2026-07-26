<div align="center">
    <img width="600" height="300" alt="RPS (Rust Paste Server) Banner" src="https://github.com/user-attachments/assets/0999d7e7-9c4e-49c3-bfbd-1bd2512bc419" />
</div>

# RPS (Rust Paste Server)

![](https://img.shields.io/github/stars/tristanbudd/RPS.svg)
![](https://img.shields.io/github/watchers/tristanbudd/RPS.svg)
![](https://img.shields.io/github/license/tristanbudd/RPS.svg)

![](https://img.shields.io/github/issues-raw/tristanbudd/RPS.svg)
![](https://img.shields.io/github/issues-closed-raw/tristanbudd/RPS.svg)
![](https://img.shields.io/github/issues-pr-raw/tristanbudd/RPS.svg)
![](https://img.shields.io/github/issues-pr-closed-raw/tristanbudd/RPS.svg)

RPS (Rust Paste Server) - A lightweight, ultra-fast self-hosted pastebin server. Built with a high-performance Rust backend and a zero-framework, minimalist frontend UI.

---

## Project Description

RPS provides a sleek, zero-bloat platform for sharing text and code snippets. It is designed to run entirely self-contained with no external font or library CDN dependencies.

The server uses an asynchronous Axum/Tokio stack backed by PostgreSQL, ensuring extremely low resource usage and sub-millisecond response times. The client-side single page application (SPA) handles rendering, saving, duplication, and syntax highlighting dynamically.

Local config reference: [config.toml](config.toml).

---

## Features

### Completed

- **Minimalist UI**: Simple, fast aesthetic with responsive transitions, dynamic scroll indicators.
- **Password Protection & Content Encryption**: Optional client-password protection hashed with Bcrypt, and optional server-side AES-256-CBC content encryption using the client password before database storage.
- **Dynamic Syntax Highlighting**: Automatic detection and loading of Highlight.js libraries for code extensions, only downloaded when viewing a non-plaintext file.
- **SPA Path-Based Extensions**: Accessing `/{PASTE_ID}.rs` or `/{PASTE_ID}.js` directly loads the syntax-highlighted code.
- **Duplicate & Edit Flow**: Clone any existing paste into the editor context with a single click to make updates and save a new version.
- **Accidental Loss Prevention**: Prompts for confirmation when initiating a new paste if the current editor contains unsaved modifications.
- **IP Rate Limiting**: Embedded middleware tracking request frequencies per IP to prevent spamming and DoS attempts.
- **Optimized Caching & Compression**: Automatic Gzip/Brotli file compression via tower-http and cache-control headers on static assets.
- **Command Line Client**: Native CLI scripts for terminal saving (`rps` / `rps.ps1`) with pipeline stdin support and automatic file extension detection.
- **Admin Dashboard & Moderation Console**: Fully responsive monochromatic administration panel available at `/admin` to monitor server metrics (CPU, RAM, DB size), prune expired pastes manually, and delete stored pastes. Gated securely with GitHub OAuth session-based authentication.

### Planned Updates

- **User Accounts & Custom Expiries**: Enable registration to manage personal pastes and specify custom lifetime parameters per paste.

---

## Preview Images

### Code Editor Interface

<img width="1920" height="945" alt="Code Editor Interface" src="https://github.com/user-attachments/assets/ea528631-3db3-4fac-8927-09e6f6d362c1" />

### Code Viewer with Syntax Highlighting

<img width="1920" height="945" alt="Code Viewer with Syntax Highlighting" src="https://github.com/user-attachments/assets/cb7ed451-3d1e-4632-a0eb-acc967fa64d8" />

---

## Tech Stack

- **Backend:** Rust (Axum, Tokio, SQLx, Postgres)
- **Frontend:** HTML5, CSS3 (Vanilla), JavaScript (ES6+ Vanilla)
- **Database:** PostgreSQL
- **Containerization:** Docker, Docker Compose

---

## Installation & Setup

### 1. Clone the repository

```bash
git clone https://github.com/tristanbudd/RPS.git
cd RPS
```

### 2. Setup Configuration

1. **Environment Variables (.env)**:
   Copy the example environment file and configure secure database credentials:

   ```bash
   cp .env.example .env
   ```

   Open the `.env` file and set a custom username (`DB_USERNAME`) and a strong, randomly generated password (`DB_PASSWORD`).

2. **Application Configuration (`config.toml`)**:
   A configuration file is provided in `config.toml`. You can edit it to customize settings like the server host, port, maximum paste length limits, cleanup task intervals, and security policies:

   ```toml
   [server]
   host = "0.0.0.0"
   port = 8000

   [paste]
   default_expiry_days = 30
   max_length = 5000000

   [security]
   password_protection_enabled = true
   encryption_enabled = true
   ```

   > [!NOTE]
   > The server will prioritize environment variables (like `DATABASE_URL`, constructed from the `.env` file in the Docker Compose environment) over the database settings in `config.toml`.

3. **Database Port Security**:
   By default, the database port `5432` is **not** exposed to the public internet or host. If you need to connect to the database from the host machine for local development or administration, you can uncomment the loopback port binding in `docker-compose.yml`:
   ```yaml
   ports:
     - "127.0.0.1:5432:5432"
   ```

### 3. Deploy using Docker Compose

Build the application and start both the PostgreSQL database and the web server:

```bash
docker compose up -d --build
```

The server will be accessible locally at `http://localhost:18000`.

---

## Admin Dashboard Configuration

RPS features a built-in administrative panel accessible at `/admin`. This panel allows checking server resource usages (CPU, RAM), PostgreSQL database storage size (relative to the configured limit), and manually moderating or deleting pastes.

### 1. GitHub OAuth Registration

To access `/admin`, you must authenticate via GitHub. Register a new OAuth application on GitHub:

1. Go to your GitHub profile -> **Settings** -> **Developer settings** -> **OAuth Apps** -> **New OAuth App**.
2. Set the **Homepage URL** to your server's domain (e.g. `http://localhost:8000` or `https://rps.example.com`).
3. Set the **Authorization callback URL** strictly to:
   ```
   https://<your-domain>/api/admin/auth/callback
   ```
   _(For local testing, use `http://localhost:8000/api/admin/auth/callback`)_

### 2. Required Environment Variables

Add the following keys to your `.env` or injection variables (e.g. in your Coolify setup):

- `GITHUB_CLIENT_ID`: The Client ID generated by GitHub.
- `GITHUB_CLIENT_SECRET`: The Client Secret generated by GitHub.
- `GITHUB_ALLOWED_USERNAME`: The exact, case-insensitive GitHub username allowed access to the dashboard.
- `DATABASE_STORAGE_LIMIT_BYTES` _(Optional)_: Limit (in bytes) before new pastes are blocked. Defaults to `10737418240` (10 GB).

---

## Command Line Interface (CLI)

RPS includes platform-native CLI helper scripts to save text and code snippets directly from your terminal.

### 1. Installation

- **Linux / macOS (Bash/Zsh)**:
  Make the `rps` script executable and copy/symlink it to your local bin path:

  ```bash
  chmod +x rps
  sudo ln -s "$(pwd)/rps" /usr/local/bin/rps
  ```

- **Windows (PowerShell)**:
  Add the directory containing `rps.ps1` to your User PATH, or execute it directly:
  ```powershell
  .\rps.ps1 -FilePath .\file.txt
  ```

### 2. Configuration

By default, the CLI scripts target `http://localhost:8000`. You can configure a custom remote server URL using the `RPS_SERVER` environment variable:

```bash
# Linux/macOS
export RPS_SERVER="https://rps.tristanbudd.com"

# Windows PowerShell
$env:RPS_SERVER="https://rps.tristanbudd.com"
```

### 3. Usage Examples

- **Piping from standard input**:

  ```bash
  cat file.txt | rps
  echo "hello world" | rps -e txt
  ```

- **Passing a file directly** (automatically detects the file extension for syntax highlighting routing):

  ```bash
  rps main.rs   # Will output: https://rps.tristanbudd.com/abc12345.rs
  ```

- **Specifying server or extension options**:

  ```bash
  # Linux/macOS:
  rps -s http://localhost:8000 -e py script.txt

  # Windows:
  .\rps.ps1 -Server http://localhost:8000 -Ext py -FilePath script.txt
  ```

---

## Scripts

```bash
cargo build --release  # Build the production release binary locally
cargo test             # Run the test suite
cargo fmt --all        # Format the codebase according to style rules
cargo clippy           # Run the linter to analyze and improve code quality
```

---

## Development Notes

- **Static Asset Serving**: The directory `src/static` contains the SPA bundle and the local font assets.
- **Database Schema**: The database tables are automatically initialized and migrated by the application on startup (defined in `src/main.rs`).
- **SPA Fallback**: The server uses a custom fallback handler to serve `index.html` with a `200 OK` status for SPA routes, avoiding console errors when utilizing file extensions.

---

## Credits & License

This project bundles and hosts the following open-source assets locally:

1. **[Inter Font Family](https://rsms.me/inter/)**
   - **Creator**: Rasmus Andersson
   - **License**: [SIL Open Font License 1.1](https://scripts.sil.org/OFL)
   - **Usage**: Used as the primary user interface typeface.

2. **[Cascadia Code Font](https://github.com/microsoft/cascadia-code)**
   - **Creator**: Microsoft
   - **License**: [SIL Open Font License 1.1](https://scripts.sil.org/OFL)
   - **Usage**: Used as the monospaced font for code editing and rendering.

3. **[Highlight.js](https://highlightjs.org/)**
   - **License**: [BSD 3-Clause License](https://github.com/highlightjs/highlight.js/blob/main/LICENSE)
   - **Usage**: Handles client-side code syntax highlighting dynamically.
