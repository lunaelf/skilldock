# Dockerfile — Spring Boot multi-stage build

A hermetic multi-stage build: the image compiles from source, so `docker build`
reproduces it anywhere. Tests do **not** run here (Testcontainers needs a Docker
daemon that `docker build` doesn't provide) — they run as a separate CI job, and
this build passes `-DskipTests`.

Comments below are English for portability; when you drop this into a project,
match its convention (e.g. Chinese comments if its `CLAUDE.md` requires them).
Replace `21` with the project's Java version (read `pom.xml` / `java.version`).

```dockerfile
# syntax=docker/dockerfile:1

# ---- deps: resolve Maven deps as a cacheable layer ----
# Copy only wrapper + pom first so this layer is reused whenever deps are unchanged.
FROM eclipse-temurin:21-jdk AS deps
WORKDIR /build
COPY .mvn/ .mvn/
COPY mvnw pom.xml ./
RUN ./mvnw -B -ntp dependency:go-offline -DskipTests

# ---- package: compile + build the executable jar (tests ran separately in CI) ----
FROM deps AS package
COPY src/ src/
RUN ./mvnw -B -ntp clean package -DskipTests

# ---- extract: split the jar into layers with the Spring Boot 4 tools jarmode ----
# Rename to application.jar first so the runtime entrypoint is version-independent.
FROM package AS extract
RUN cp target/*.jar application.jar \
 && java -Djarmode=tools -jar application.jar extract --layers --destination extracted

# ---- final: minimal JRE runtime, non-root ----
# temurin (glibc), NOT alpine (musl) — the safe default. Required only if a dep
# ships a native lib (e.g. sqlite-jdbc); pure-Java drivers (postgres JDBC) don't.
FROM eclipse-temurin:21-jre AS final
WORKDIR /application

# Non-root user. Do NOT use --system with an explicit uid > 999 (it warns about
# SYS_UID_MAX); a plain uid/gid 10001 with no home + nologin shell is the intent.
RUN groupadd --gid 10001 appgroup \
 && useradd --uid 10001 --gid appgroup --no-create-home --home-dir /application --shell /usr/sbin/nologin appuser

# Copy layers stable→volatile so only the application layer rebuilds on code change.
COPY --from=extract /build/extracted/dependencies/ ./
COPY --from=extract /build/extracted/spring-boot-loader/ ./
COPY --from=extract /build/extracted/snapshot-dependencies/ ./
COPY --from=extract /build/extracted/application/ ./

USER appuser
EXPOSE 8080
# tools-jarmode layout is a thin application.jar + lib/; launch with plain java -jar
# (its manifest Main-Class is your app and Class-Path points at ./lib). This is
# NOT the old exploded-BOOT-INF + JarLauncher layout.
ENTRYPOINT ["java", "-jar", "application.jar"]
```

## Why these choices

- **Hermetic (build inside the Dockerfile)** rather than copying a CI-built jar:
  `docker build` reproduces the image standalone. The extra compile after the
  test job is cheap because the `deps` layer caches via buildx GHA cache and
  `go-offline` isolates dependency download into its own layer.
- **`tools` jarmode `extract --layers`** produces a *thin* `application.jar`
  (Main-Class = your app, `Class-Path: lib/...`) plus a `lib/` directory of
  dependency jars — a standard JAR run on the normal system classloader, and
  AOT-cache/CDS friendly. The `spring-boot-loader` layer is typically empty in
  this layout; keep the COPY anyway to match Spring's reference and stay
  forward-compatible.
- **Non-root** (`uid 10001`) limits blast radius. `/tmp` stays writable for
  native-lib extraction (e.g. sqlite-jdbc) and any temp download.
- **Container timezone stays UTC** (temurin default). Only set `TZ` if the app
  relies on the JVM default zone; if it parses timestamps with explicit zones,
  leave it (changing it can reintroduce a timezone bug).

## .dockerignore

Keep the build context small and never leak secrets or the local `target/`:

```
# build output (also keeps it out of the layer cache key)
target/

# vcs + ci
.git/
.gitattributes
.gitignore
.github/

# ide
.idea/
*.iml
*.iws
*.ipr

# secrets + local env — never in the image or context
.env
.env.example

# agent/scratch dirs
.agents/
.claude/
.scratch/

# docs (not needed at runtime)
docs/
*.md

# docker/compose itself
Dockerfile
.dockerignore
compose.yaml
compose.prod.yaml
```

Do **not** exclude `.mvn/` or `mvnw` — the build needs the wrapper.
