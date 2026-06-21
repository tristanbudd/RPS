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
- **Dynamic Syntax Highlighting**: Automatic detection and loading of Highlight.js libraries for code extensions, only downloaded when viewing a non-plaintext file.
- **SPA Path-Based Extensions**: Accessing `/{PASTE_ID}.rs` or `/{PASTE_ID}.js` directly loads the syntax-highlighted code.
- **Duplicate & Edit Flow**: Clone any existing paste into the editor context with a single click to make updates and save a new version.
- **Accidental Loss Prevention**: Prompts for confirmation when initiating a new paste if the current editor contains unsaved modifications.
- **IP Rate Limiting**: Embedded middleware tracking request frequencies per IP to prevent spamming and DoS attempts.
- **Optimized Caching & Compression**: Automatic Gzip/Brotli file compression via tower-http and cache-control headers on static assets.

### Planned Updates

- **Command Line Client**: A minimalist CLI helper (curl-based or native) to allow saving directly from the terminal.
- **Password Protection**: Optional encryption or password validation before loading sensitive snippets.
- **Admin Dashboard**: A panel to monitor active pastes, storage limits, and server metrics.

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
   A configuration file is provided in `config.toml`. You can edit it to customize settings like the server host, port, maximum paste length limits, and cleanup task intervals:
   ```toml
   [server]
   host = "0.0.0.0"
   port = 8000

   [paste]
   default_expiry_days = 30
   max_length = 5000000
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
