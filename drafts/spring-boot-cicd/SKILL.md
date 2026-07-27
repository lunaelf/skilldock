---
name: spring-boot-cicd
description: >-
  Set up containerized CI/CD for a Spring Boot / JVM service: a multi-stage
  Dockerfile, a GHCR image build via GitHub Actions, and a Docker Compose
  deploy over SSH to a single server (behind a reverse proxy, connecting to an
  external database). Use this whenever the user wants to containerize /
  Dockerize a Spring Boot or Java app, or "set up CI/CD" / a deploy pipeline
  for one — and when they mention a Dockerfile, GHCR / ghcr.io, GitHub Actions
  deployment, compose.prod.yaml, scp/ssh deploy, Traefik/nginx in front of a
  JVM app, or shipping a Spring Boot service to a server, even if they don't
  name every piece. The skill interviews for the environment facts, grounds
  version-specific details in current docs, generates the files, and verifies
  by actually building the image.
---

# Spring Boot CI/CD

Produce a working deployment pipeline for a Spring Boot / JVM service:

- `Dockerfile` + `.dockerignore` — multi-stage build, Spring Boot layered jar, non-root
- `compose.prod.yaml` — how the container runs in production (reverse proxy, DB, secrets)
- `.github/workflows/deploy.yml` — test → build → push to GHCR → deploy over SSH
- optionally an ADR recording the deployment architecture

## The core principle: interview → ground → generate → verify

Roughly 80% of this config is identical across projects. The other 20% — the
registry, the server, whether there's a reverse proxy, where the database
lives, the CPU arch, how the server authenticates to the registry — is exactly
the part that silently breaks a deploy if you guess wrong. **So don't paste
templates blind.** And the version-specific details drift: Spring Boot changed
its jar-extraction mechanism (`layertools` → `tools`), GitHub Action major
versions bump, base images move. A template that was correct last year fails
today.

Two disciplines make the difference between "looks right" and "works":

1. **Interview the user** for the environment facts before writing anything.
2. **Build the image and launch it** before declaring done. A `docker build`
   plus a launch smoke-test catches the bugs a human reviewer's eyes slide
   past — a wrong entrypoint, a missing native lib, a nonexistent action tag.

Work through the steps below in order.

## Step 1 — Interview for the environment facts

Ask these as you would in a design review — roughly one decision at a time,
each with your recommended default, resolving dependencies before dependents.
**Look up what you can** (the git remote for the image name, `pom.xml` for the
Java version, any existing `compose.yaml`, the app's port) rather than asking.
Put the genuine *decisions* to the user.

| Decision | Default (recommend unless told otherwise) | Notes / dependency |
| --- | --- | --- |
| Container registry | **GHCR** (`ghcr.io/<owner>/<repo>`, lowercase) | needs the GitHub repo to exist |
| Image visibility + how the **server** pulls | **private + ephemeral `GITHUB_TOKEN`** forwarded over SSH | public (no auth) or long-lived PAT are the alternatives |
| Trigger / release model | **tag `v*`** → build+push+deploy; `main` → test+build only | or auto-deploy on `main`, or manual |
| Test gate | **`./mvnw verify`** as a separate CI job that gates build+deploy | Testcontainers needs Docker — can't run in `docker build` |
| Server CPU arch | **amd64** | arm64 / multi-arch change the `platforms:` and need QEMU |
| Reverse proxy in front | **ask** — Traefik (labels) / nginx (host port) / none | decides whether compose publishes a host port |
| Database location | **ask** — remote (address in `.env`) / same host / sidecar | decides the container→DB networking |
| Server path, deploy user, SSH secrets | `SSH_HOST` / `SSH_USERNAME` / `SSH_PRIVATE_KEY` / `SSH_PORT` | plus the absolute project path on the server |

The two questions with real correctness impact are **reverse proxy** and
**database location** — they interact (see `references/compose-deploy.md`).
Surface the interdependencies rather than deciding silently:

- visibility → server-pull auth
- reverse proxy → whether a host port is published, and how the app is reached
- DB location → container networking (`localhost` inside a container is the
  container itself — see the DB_URL trap in `references/gotchas.md`)

## Step 2 — Ground the version-specific details in current docs

These drift and fail silently, so verify them for *this* project rather than
trusting memory or an old tutorial. (Prefer Context7 for library docs; use the
Docker guides at `docs.docker.com/guides/java` and `.../gha` for the shape.)

- **Spring Boot layered-jar extraction.** For Boot 3.2+/4 it is
  `java -Djarmode=tools ... extract --layers` with
  `ENTRYPOINT ["java","-jar","application.jar"]` — **not** the older
  `layertools` + `org.springframework.boot.loader.launch.JarLauncher`. The two
  are not interchangeable; each pairs with its own extraction layout. Confirm
  the form for the project's exact Boot version.
- **GitHub Action versions.** A tag that doesn't exist fails the whole run.
  Check the current *major* for each action instead of guessing:
  `gh api repos/<owner>/<action>/releases/latest --jq .tag_name` (e.g.
  `actions/checkout`, `actions/setup-java`, `docker/build-push-action`,
  `docker/metadata-action`, `docker/login-action`, `docker/setup-buildx-action`,
  `appleboy/ssh-action`, `appleboy/scp-action`).
- **Base image.** Match the project's Java version (`eclipse-temurin:<N>-jdk`
  build / `-jre` runtime). Default to a **glibc** image (temurin/liberica) over
  Alpine (musl): it's the safe choice, and it's *required* if any dependency
  ships a native library (e.g. `sqlite-jdbc`, `netty-tcnative`) or that lib
  fails to load at runtime. Pure-Java drivers — the Postgres/MySQL JDBC drivers
  — don't need it, so don't cite a native-lib reason a project doesn't have.

## Step 3 — Generate the files

Read the reference for each artifact and adapt it to the answers from Step 1:

- **`references/dockerfile.md`** — the multi-stage `Dockerfile` and
  `.dockerignore`.
- **`references/github-actions.md`** — the `deploy.yml` workflow.
- **`references/compose-deploy.md`** — `compose.prod.yaml`, with the
  reverse-proxy and database-location variants.

Match the surrounding project's conventions when you write: if its `CLAUDE.md`
asks for Chinese comments, write the Dockerfile/compose comments in Chinese;
keep log/echo lines English. Don't invent a `server.port` — read it, default 8080.

## Step 4 — Verify (do not skip — this is where bugs surface)

Static checks first, then actually run it:

```bash
# 1. YAML + compose parse and interpolate
python3 -c "import yaml; yaml.safe_load(open('.github/workflows/deploy.yml'))"
IMAGE_TAG=v0.0.0-test docker compose -f compose.prod.yaml config

# 2. Build the real image (compiles the app, exercises the jarmode extraction)
docker build --target final -t <artifact>:verify .

# 3. Launch smoke-test. spring.context.exit=onRefresh (as a -D system property,
#    via JAVA_TOOL_OPTIONS since the entrypoint is a fixed java -jar) boots the
#    full context then exits rc 0, so the run self-terminates instead of a web
#    server staying up. The Spring Boot banner proves the JVM + entrypoint +
#    classpath + non-root user all work.
docker run --rm -e JAVA_TOOL_OPTIONS="-Dspring.context.exit=onRefresh" <artifact>:verify
docker rmi <artifact>:verify
```

It must be the **`-D` system property** form above. This early property is read
from system properties only, so a `SPRING_CONTEXT_EXIT` env var *or* a
`--spring.context.exit` program arg do **not** bind — the web server stays up and
the run hangs (both verified). The `-D` form self-exits (rc 0), so a foreground
`docker run --rm` returns on its own — no `timeout` (absent on macOS) or
foreground `sleep` (blocked in some harnesses). If there's **no** Spring Boot
banner at all, the entrypoint or layout is wrong — see `references/gotchas.md`.

## Step 5 — Document and hand off

- **Offer an ADR** (`docs/adr/`) for the deployment architecture *if* it clears
  the bar: hard-ish to reverse, surprising without context (why
  `host.docker.internal`? why no published port? why tag-triggered?), and the
  result of real trade-offs. Skip it otherwise.
- **List the out-of-band prerequisites** the user must do — these aren't code
  and the pipeline can't do them:
  - create/push the GitHub repo (for Actions + the GHCR image name)
  - set the `SSH_*` repo secrets
  - DNS for the domain; the external Docker network exists; deploy user in the
    `docker` group
  - **the DB_URL the container will use** — a remote address, or
    `host.docker.internal` for a same-host DB. Call this out loudly: `localhost`
    in the server's `.env` points the container at *itself* and is the single
    most common first-deploy failure.

## Gotchas

`references/gotchas.md` collects the traps that cost real debugging time —
read it before generating, and again if the build or launch misbehaves.
