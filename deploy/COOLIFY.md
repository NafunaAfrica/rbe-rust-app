# Deploying RBE → https://tanya.nafuna.africa

Same infra as BrandZa: **GitHub Actions builds the image → GHCR → Coolify pulls
it** on the server, behind **Cloudflare**. The server never compiles Rust.

Do the steps in order.

---

## 1. Push the repo to GitHub  ✅ done

Repo `NafunaAfrica/rbe-rust-app` is created and `main` is pushed, so the
`.github/workflows/deploy.yml` workflow is already running. It builds + pushes:

```
ghcr.io/nafunaafrica/rbe-rust-app:latest
ghcr.io/nafunaafrica/rbe-rust-app:<commit-sha>
```

Watch it under the repo's **Actions** tab. First build is ~5–10 min (cold cache).

---

## 2. Make the GHCR image pullable

By default the GHCR package is **private**. Either:

- **Make it public** (fine for a store): GitHub → the org's *Packages* →
  `rbe-rust-app` → *Package settings* → *Change visibility* → Public. Verify:
  ```bash
  token=$(curl -s "https://ghcr.io/token?scope=repository:nafunaafrica/rbe-rust-app:pull&service=ghcr.io" | jq -r .token)
  curl -s -o /dev/null -w '%{http_code}\n' -H "Authorization: Bearer $token" \
    https://ghcr.io/v2/nafunaafrica/rbe-rust-app/manifests/latest      # 200 = public & exists
  ```
- **Or keep it private** and give Coolify a registry credential: a GitHub PAT
  with `read:packages`, added in Coolify under the server's Docker registries.

---

## 3. Point DNS first (so TLS can issue)

In **Cloudflare** for `nafuna.africa`, add a record for `tanya`:

- Type **A** → your Coolify server's IP (the one BrandZa uses), **Proxied** (orange cloud).
- (Cloudflare terminates TLS at the edge; Coolify also gets a cert. Point DNS
  before adding the domain in Coolify or cert issuance can fail.)

---

## 4. Create the app in Coolify

Coolify dashboard → your project (or **+ New** → Project) → **+ New Resource** →
**Docker Image**.

- **Image:** `ghcr.io/nafunaafrica/rbe-rust-app:latest`
- **Ports exposed:** `8080`
- **Domain:** `https://tanya.nafuna.africa`
- **Health check:** path `/health`, port `8080` (the image already has `curl`
  and a `/health` route, so the in-container probe passes).

### Environment variables (Coolify → the app → *Environment Variables*)

Set these — **secrets live here, never in git**. Copy values from your local
`.env`, but generate a fresh `RBE_JWT_SECRET` and set a real admin password:

```
RBE_BIND_ADDR=0.0.0.0:8080
RBE_DATA_DIR=/data/surreal
RBE_PUBLIC_BASE_URL=https://tanya.nafuna.africa
RBE_JWT_SECRET=<generate: openssl rand -hex 32>
RBE_ADMIN_EMAIL=<your admin email>
RBE_ADMIN_PASSWORD=<a strong password>
SHOPIFY_STORE_DOMAIN=f0epji-nd.myshopify.com
SHOPIFY_STOREFRONT_TOKEN=f57a26bebd7fd009ce29dd16b8b79096
SHOPIFY_API_VERSION=2025-07
SHOPIFY_WEBHOOK_SECRET=<your shpss_… app secret>
PRINTIFY_API_TOKEN=<your Printify token>
# optional, enables the agent:
ANTHROPIC_API_KEY=
RBE_AGENT_MODEL=claude-sonnet-5
```

> `RBE_PUBLIC_BASE_URL` must be the real public URL — Printify's servers fetch
> product images from it during a sync, so `localhost` can't work (this is why
> the sync only works once deployed).

---

## 5. Add persistent storage (or SurrealDB is wiped on every redeploy)

Coolify → the app → *Storages* → add:

- **Type:** `persistent`  ← must be exactly this (not `volume`/`bind`)
- **Name:** `rbe-data`
- **Mount path:** `/data`

**Redeploy after adding storage** so it attaches. Coolify reuses the volume
across redeploys (a later deploy log with no "Volume … Creating" line = your
persistence proof).

---

## 6. Deploy

Click **Deploy**. When it finishes:

```bash
curl -s https://tanya.nafuna.africa/health          # -> ok
```

Then open **https://tanya.nafuna.africa** — the shop shows the 11 designs from
SurrealDB. Log in at `/auth`, then `/admin/printify` to sync products to Shopify
(now that the public URL works, image upload succeeds).

---

## 7. Auto-redeploy on future pushes (optional)

Coolify → the app → *Webhooks* → copy the **Deploy Webhook** URL, then in GitHub
→ repo *Settings → Secrets and variables → Actions*:

- Secret `COOLIFY_WEBHOOK_URL` = the webhook URL
- Secret `COOLIFY_TOKEN` = a Coolify API token

Now every push to `main` builds a fresh image and pings Coolify to pull it.
If `COOLIFY_WEBHOOK_URL` is missing, the workflow will still build and push the
image, but it will skip the live redeploy step.

---

## Post-deploy checklist

- [ ] `GET /health` → `ok`
- [ ] Store loads at the domain over HTTPS
- [ ] Admin login works; `/admin/printify` lists shops (RBE / Shopify-linked)
- [ ] Run a Printify sync → products appear in Shopify → checkout activates
- [ ] Point Shopify + Printify **webhooks** at `/api/webhooks/{shopify,printify}`
      and set `SHOPIFY_WEBHOOK_SECRET` / `PRINTIFY_WEBHOOK_SECRET`
- [ ] **Rotate** the Shopify + Printify tokens that were shared in chat
