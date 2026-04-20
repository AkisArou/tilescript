# Playground

Run the playground locally:

```sh
make playground
```

Build the static playground bundle for GitHub Pages:

```sh
make playground-build PLAYGROUND_PUBLIC_URL=/tilescript/
```

Local dev output is written to `apps/tilescript-playground/.dist-dev`.

GitHub Pages output is written to `apps/tilescript-playground/.dist`.

GitHub Pages deployment is configured in `.github/workflows/deploy-playground-pages.yml`.

In the GitHub repo settings, set `Settings > Pages > Source` to `GitHub Actions`.
