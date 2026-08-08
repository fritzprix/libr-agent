# LibrAgent user docs (VitePress)

Source of truth: [`docs/user/`](../docs/user/).

```bash
# from repo root
pnpm install
pnpm docs:dev      # http://localhost:5173/libr-agent/
pnpm docs:build
pnpm docs:preview
```

Production URL (GitHub Pages): https://fritzprix.github.io/libr-agent/

Deploy: [`.github/workflows/docs.yml`](../.github/workflows/docs.yml) on push to `main` when `docs/user/**` or `website/**` change.

## Enable GitHub Pages (once)

Repo **Settings → Pages → Build and deployment → Source: GitHub Actions**.
