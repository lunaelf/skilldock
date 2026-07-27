# Gotchas

The traps that cost real debugging time. Skim before generating; revisit if the
build or launch misbehaves.

## Spring Boot jar layout & entrypoint

- **`tools`, not `layertools`.** Boot 3.2+/4 use `-Djarmode=tools ... extract
  --layers`. The old `layertools` jarmode is deprecated. They produce different
  layouts and are **not** interchangeable with each other's entrypoint.
- **`extract --layers` gives a thin jar + `lib/`, not exploded `BOOT-INF`.** The
  extracted `application.jar` has `Main-Class: <your app>` and
  `Class-Path: lib/...`; it runs on the plain system classloader. Launch it with
  `java -jar application.jar`. `org.springframework.boot.loader.launch.JarLauncher`
  will **fail** here — there's no `BOOT-INF`, and the `spring-boot-loader` layer
  is empty. (Tutorials that use `JarLauncher` are on the old `layertools` path.)
- **Symptom of a wrong entrypoint:** the container produces no Spring Boot banner
  at all and exits immediately. If you see the banner, the launch is correct.

## Base image / native libraries

- **glibc vs musl.** `eclipse-temurin` (glibc) runs native libs like
  `sqlite-jdbc` fine; Alpine (musl) needs the musl-specific native build or the
  lib fails to load at runtime. Default to temurin/liberica unless you've
  confirmed every native dep has a musl build.
- **Non-root user warning.** `useradd --system` with an explicit uid > 999 warns
  `uid is greater than SYS_UID_MAX`. Drop `--system`; a plain `--uid 10001
  --no-create-home --shell /usr/sbin/nologin` user is what you want.

## Database networking

- **`localhost` inside a container is the container itself.** The #1 first-deploy
  failure is a server `.env` with `DB_URL=...localhost:5432...` carried over from
  running the jar natively. For a same-host DB use `host.docker.internal:5432`
  (+ `extra_hosts: host.docker.internal:host-gateway`); for a remote DB use its
  real address. With a reverse proxy in play you can't use host networking, so
  `localhost` is never right for the DB.
- **Source IP after Docker NAT.** A container connecting out to the DB is seen by
  the DB as coming from the *host's* IP (or a docker subnet). The DB's `pg_hba` /
  firewall must allow that. Pigsty's default `intra` rule covers RFC1918, which
  includes docker subnets and typical LAN hosts.
- **PgBouncer + Flyway.** Flyway takes a session-level `pg_advisory_lock` during
  migration, which is unsafe under PgBouncer's default transaction pooling. Point
  `DB_URL` at Postgres directly (5432), not PgBouncer (6432).

## CI / registry

- **Pin action majors that exist.** A nonexistent tag fails the whole run. Verify
  with `gh api repos/<owner>/<action>/releases/latest --jq .tag_name`.
- **GHCR is private by default.** The deploy job needs `permissions: packages:
  read` for the forwarded `GITHUB_TOKEN` to pull. If the first pull 403s, link
  the package to the repo once in package settings.
- **Testcontainers can't run in `docker build`** (no Docker-in-Docker). Run
  `verify` as a separate CI job on the runner; build the image with `-DskipTests`.
- **compose interpolation vs `env_file`.** Pass `IMAGE_TAG` via the shell env
  (exported by `ssh-action`) for `${IMAGE_TAG}` substitution; keep the server
  `.env` for `env_file:` secrets only. Don't write `IMAGE_TAG` into `.env`.

## Verification tooling

- **Smoke-test primitive: `-Dspring.context.exit=onRefresh` via `JAVA_TOOL_OPTIONS`.**
  Run `docker run --rm -e JAVA_TOOL_OPTIONS="-Dspring.context.exit=onRefresh" <img>`.
  Spring boots the full context then exits rc 0, so a foreground run
  self-terminates whether the app is a long-running web server or not — no
  `timeout` (absent on macOS) or foreground `sleep` (blocked in some harnesses).
  It must be the **`-D` system property**: this early property is read from
  system properties only, so a `SPRING_CONTEXT_EXIT` env var *or* a
  `--spring.context.exit` program arg do NOT bind — the web server stays up and
  the run hangs (both verified). Also avoid `docker logs -f | grep -m1` as a
  substitute: `grep -m1` exits on the match but `docker logs -f` keeps following
  a quiet-but-alive container, so the pipeline hangs. The pass signal is the
  Spring Boot banner; **no** banner means the entrypoint/layout is wrong.
