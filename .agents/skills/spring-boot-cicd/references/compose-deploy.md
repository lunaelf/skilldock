# compose.prod.yaml — production runtime

Runs **only the app container**. Assemble it from a base plus one reverse-proxy
variant and one database variant, based on the Step 1 answers.

## Base

```yaml
services:
  app:
    # tag injected by the deploy job (git tag, e.g. v1.2.3); errors if unset
    image: ghcr.io/<owner>/<repo>:${IMAGE_TAG:?IMAGE_TAG is required}
    restart: unless-stopped
    # DB + API creds come from the server's production .env (never overwritten by deploy)
    env_file:
      - .env
    environment:
      SPRING_PROFILES_ACTIVE: prod        # force prod; wins over .env
```

Then add a network/ingress block and, if needed, a DB block.

## Reverse-proxy variant

**Traefik (Docker provider — labels).** App joins Traefik's external network and
carries router labels; no host port is published (Traefik is the only ingress,
TLS terminates there, the app stays plain-HTTP 8080). Needs four facts from the
user: network name, host domain, `websecure` entrypoint name, cert-resolver name.

```yaml
    networks:
      - <traefik-network>
    labels:
      traefik.enable: "true"
      traefik.docker.network: "<traefik-network>"
      traefik.http.routers.<name>.rule: "Host(`<domain>`)"
      traefik.http.routers.<name>.entrypoints: "websecure"
      traefik.http.routers.<name>.tls.certresolver: "<resolver>"
      traefik.http.services.<name>.loadbalancer.server.port: "8080"
      # let Traefik health-check the public actuator endpoint (no curl in the image)
      traefik.http.services.<name>.loadbalancer.healthcheck.path: "/actuator/health"
      traefik.http.services.<name>.loadbalancer.healthcheck.interval: "30s"

networks:
  <traefik-network>:
    external: true
```

**nginx / file-based proxy.** Publish to loopback only and point the proxy at it:

```yaml
    ports:
      - "127.0.0.1:8080:8080"
```

**No proxy (direct).** Publish the port on the host:

```yaml
    ports:
      - "8080:8080"
```

## Database variant

**Remote DB (address in `.env`).** Nothing special — the container reaches it
over normal outbound networking. `DB_URL` in the server `.env` holds the real
host/IP. Ensure the app's network has egress (a plain bridge does; a Traefik
ingress network usually does too — but not if it's `internal: true`).

**Same-host DB (e.g. a host-native Postgres).** Map the host gateway and point
`DB_URL` at it:

```yaml
    extra_hosts:
      - "host.docker.internal:host-gateway"   # DB_URL -> host.docker.internal:5432
```

**Sidecar DB (dev-style, not typical for prod-with-managed-DB).** Add a `db`
service + a healthcheck `depends_on`. Prefer a managed/host DB in production.

## Rules that keep this safe

- **Never publish a port when a reverse proxy fronts the app** — let the proxy
  be the only ingress; keep the app plain-HTTP internally.
- **`SPRING_PROFILES_ACTIVE: prod` in `environment:`**, not relying on `.env`, so
  the profile is guaranteed.
- **The deploy scp's this file but never the `.env`.** Secrets live only on the
  server; the app reads them via `env_file`.
- **Health-check without baking tools into the image**: prefer the reverse
  proxy's health-check of `/actuator/health` (public) over a container
  `healthcheck:` that would need `curl`/`wget` added to the JRE image. This
  assumes `spring-boot-starter-actuator` is present and health is exposed
  (`management.endpoints.web.exposure.include=health`). If the app has no
  actuator, **omit the health-check labels** — an active check against a 404
  makes the proxy drop the service from rotation.

Validate before shipping: `IMAGE_TAG=v0.0.0-test docker compose -f compose.prod.yaml config`.
