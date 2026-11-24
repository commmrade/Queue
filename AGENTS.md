# Agent guide

## Project

This repository contains a small Russian-language student presentation queue app for ДВФУ. It is a three-service Docker Compose application:

- `frontend/`: Vue 3 single-page app, built with Vite and served by Nginx.
- `backend/`: Rust 2021 REST API using Axum, SQLx, and MariaDB.
- `mariadb`: persistent database in the Compose volume `mariadb_data`.

Nginx serves the frontend and proxies `/api/` to the backend. The API listens on port 3000 inside the Compose network. Only the frontend port (8080 by default) and MariaDB's loopback-only development port (3307 by default) are published.

## Useful files

- `frontend/web/App.vue`: UI, auth forms, queue listing/detail, and user interactions.
- `frontend/web/style.css`: all application styles.
- `frontend/web/main.js`: Vue app bootstrap.
- `frontend/vite.config.js`: local dev server and API proxy configuration.
- `frontend/nginx.conf`: production static hosting and API proxy.
- `backend/src/main.rs`: API startup and port binding.
- `backend/src/handlers/mod.rs`: route table.
- `backend/src/handlers/auth.rs`: registration, login, current-user, and logout endpoints.
- `backend/src/handlers/queues.rs`: queue and entry endpoints, authorization checks, and guest identity handling.
- `backend/src/auth.rs`: JWT, cookies, authenticated-user lookup, and username validation.
- `backend/src/models.rs`: database/API data structures.
- `backend/src/db.rs`: queue loading from MariaDB.
- `backend/src/state.rs`: environment configuration, database pool, and automatic migrations.
- `backend/migrations/`: ordered SQLx schema migrations. Add a new migration for schema changes; do not rewrite already-applied migrations for deployed databases.
- `docker-compose.yml`, `.env.example`: service wiring and configuration template.
- `README.md`: deployment, local development, admin setup, and API overview.

## Domain and behavior

- Queues are publicly listed and can be joined by guests or authenticated users. Each entry contains the participant-provided name.
- Only accounts with role `admin` can create, edit, close, delete queues, or remove other participants. Registration creates role `user`; admins are assigned manually in MariaDB. The backend checks the role from the database for each authenticated request.
- An entry can be removed by its creator using the secret returned when joining. The browser stores that secret in local storage; the database stores only its SHA-256 hash. Admins can remove any entry.
- Duplicate queue entries are guarded by database unique constraints for account identity, visitor identity, and queue/name. Visitor IDs are kept in a long-lived HttpOnly cookie. Clearing the cookie or using another browser creates a new visitor identity.
- Auth uses usernames, not email addresses. Usernames are lowercased and trimmed, must be 3–32 ASCII letters, digits, or underscores, and are unique. Passwords must be 8–128 bytes and are stored as Argon2 hashes.
- Auth returns a public user object (`id`, `username`, `role`) and sets an HttpOnly JWT cookie. JWTs last seven days; logout records the token ID in `revoked_tokens` until expiry and clears the cookie. `QUEUE_COOKIE_SECURE=true` adds the Secure cookie attribute for HTTPS deployments.
- Existing installations running migration `0004_use_usernames.sql` receive generated usernames in the form `user_<account-id-without-hyphens>`. That migration drops the old email column. Keep this compatibility behavior in mind when changing account schema.

## API routes

- `POST /api/auth/register`, `POST /api/auth/login`: JSON `{ "username": "...", "password": "..." }`.
- `GET /api/auth/me`: current authenticated public user.
- `POST /api/auth/logout`: revoke current JWT and clear auth cookie.
- `GET /api/queues`: list queues and entries; also initializes the visitor cookie when absent.
- `POST /api/queues`: admin-only queue creation.
- `PUT /api/queues/:id`, `DELETE /api/queues/:id`: admin-only queue management.
- `POST /api/queues/:id/join`: join an open queue; returns the updated queue and a one-time leave token.
- `DELETE /api/queues/:id/entries/:entry_id`: leave using the `x-queue-leave-token` header.
- `DELETE /api/queues/:id/manage/entries/:entry_id`: admin-only participant removal.
- `GET /health`: returns 204 when MariaDB is reachable, otherwise 503.

API errors are returned as JSON strings by the shared error type; the frontend also tolerates JSON objects with an `error` field.

## Development

The canonical local setup is described in `README.md`. In brief, configure `.env`, start and wait for MariaDB with `docker compose up -d --wait mariadb`, then run the backend from the repository root with `cargo run --manifest-path backend/Cargo.toml` after exporting `DATABASE_URL` and the required secrets. Run the frontend with `cd frontend && npm ci && npm run dev`; Vite proxies `/api` to `http://localhost:3000` unless `VITE_API_PROXY_TARGET` is set.

Production-style build and startup use `docker compose up -d --build`. The backend runs embedded SQLx migrations automatically during startup and retries transient MariaDB connection errors for up to 30 seconds while the database starts. Keep secrets out of source control; use `.env.example` only as a template. `QUEUE_JWT_SECRET` must be at least 32 bytes, and database credentials must match the Compose environment.

## Change guidelines

- Keep the Russian user-facing copy consistent with the existing UI and README.
- Keep frontend request shapes, backend deserialization structs, SQL queries, and public response models in sync.
- Preserve authorization checks on the backend; frontend visibility of admin controls is only presentation.
- Preserve the database uniqueness and foreign-key constraints that enforce queue integrity.
- For schema changes, add the next numbered migration and update any relevant README instructions. Migrations run on every backend startup but each migration is applied only once by SQLx.
- This project currently has no dedicated test suite. When asked to validate a change, use focused checks such as `cargo fmt --check`, `cargo check --manifest-path backend/Cargo.toml`, or `npm run build` as appropriate. Do not assume a database is available for integration checks.
