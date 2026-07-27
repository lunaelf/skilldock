# GitHub Actions — build to GHCR + deploy over SSH

Three jobs: `test` gates everything, `build` pushes to GHCR only on a `v*` tag,
`deploy` scp's the compose file and restarts over SSH. **Pin every action to a
major that currently exists** — verify with
`gh api repos/<owner>/<action>/releases/latest --jq .tag_name` before writing
(the versions below are examples and drift over time).

```yaml
name: Build and Deploy

# main push: test + build only (validate the Dockerfile).
# v* tag: test → build → push to GHCR → deploy over SSH.
on:
  push:
    branches: [main]
    tags: ['v*']

env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}   # owner/repo, already lowercase

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v7
      - uses: actions/setup-java@v5
        with:
          distribution: temurin
          java-version: '21'
          cache: maven
      - name: Run unit + integration tests
        run: ./mvnw -B -ntp verify        # full build + tests (ubuntu-latest has Docker for Testcontainers, if the project uses it)

  build:
    needs: test
    runs-on: ubuntu-latest
    permissions:
      contents: read
      packages: write                      # so GITHUB_TOKEN can push to GHCR
    steps:
      - uses: actions/checkout@v7
      - uses: docker/setup-buildx-action@v4
      - name: Log in to GHCR
        uses: docker/login-action@v4
        with:
          registry: ${{ env.REGISTRY }}
          username: ${{ github.actor }}
          password: ${{ secrets.GITHUB_TOKEN }}
      - name: Image metadata
        id: meta
        uses: docker/metadata-action@v6
        with:
          images: ${{ env.REGISTRY }}/${{ env.IMAGE_NAME }}
          # event=tag yields the git tag verbatim (e.g. v1.2.3) so deploy can
          # pull exactly ${{ github.ref_name }}.
          tags: |
            type=ref,event=tag
            type=ref,event=branch
      - name: Build and push
        uses: docker/build-push-action@v7
        with:
          context: .
          platforms: linux/amd64
          push: ${{ startsWith(github.ref, 'refs/tags/v') }}   # main only validates
          tags: ${{ steps.meta.outputs.tags }}
          labels: ${{ steps.meta.outputs.labels }}
          cache-from: type=gha
          cache-to: type=gha,mode=max

  deploy:
    needs: build
    runs-on: ubuntu-latest
    if: startsWith(github.ref, 'refs/tags/v')
    permissions:
      contents: read
      packages: read                       # so the forwarded token can pull
    steps:
      - uses: actions/checkout@v7
      - name: Copy compose.prod.yaml to server
        uses: appleboy/scp-action@v1.0.0
        with:
          host: ${{ secrets.SSH_HOST }}
          username: ${{ secrets.SSH_USERNAME }}
          key: ${{ secrets.SSH_PRIVATE_KEY }}
          port: ${{ secrets.SSH_PORT }}
          source: compose.prod.yaml
          target: /home/<user>/<project>
      - name: Pull image and restart on server
        uses: appleboy/ssh-action@v1.2.5
        env:
          GHCR_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          GHCR_USER: ${{ github.actor }}
          IMAGE_TAG: ${{ github.ref_name }}
          REGISTRY: ${{ env.REGISTRY }}
        with:
          host: ${{ secrets.SSH_HOST }}
          username: ${{ secrets.SSH_USERNAME }}
          key: ${{ secrets.SSH_PRIVATE_KEY }}
          port: ${{ secrets.SSH_PORT }}
          envs: GHCR_TOKEN,GHCR_USER,IMAGE_TAG,REGISTRY
          script: |
            set -euo pipefail
            cd /home/<user>/<project>
            echo "$GHCR_TOKEN" | docker login "$REGISTRY" -u "$GHCR_USER" --password-stdin
            docker compose -f compose.prod.yaml pull
            docker compose -f compose.prod.yaml up -d
            docker logout "$REGISTRY"
            docker image prune -f
```

## Why these choices

- **`GITHUB_TOKEN`, not a PAT, for both push and pull.** The build job pushes
  with the built-in token (`packages: write`). The deploy job forwards that same
  ephemeral token to the server via `ssh-action`'s `envs`, logs in, pulls, logs
  out — no long-lived credential to store or rotate. It works because the token
  can read its own repo's package.
- **Tag = git tag.** `IMAGE_TAG = github.ref_name` (`v1.2.3`) is the immutable
  tag `metadata-action` pushed, and `compose.prod.yaml` interpolates it. Rollback
  = re-run an older tag. Interpolation reads the shell env exported by
  `ssh-action`, so the server `.env` stays secrets-only.
- **`push:` gated on the tag ref**, so pushing to `main` builds (validates the
  Dockerfile) without publishing. `deploy` is likewise `if:`-gated to tags.
- **Testcontainers can't run in `docker build`** (no Docker-in-Docker), which is
  why `verify` is a separate job on the runner and the image builds `-DskipTests`.

## First-deploy notes

- GHCR packages are **private by default**. If the first server pull 403s, link
  the package to the repo (package settings → Manage Actions access) once.
- The server needs Docker + Compose v2, the deploy user in the `docker` group,
  and the project directory to already contain the production `.env`.
