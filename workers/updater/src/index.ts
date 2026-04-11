/**
 * WhisperDesk auto-updater endpoint.
 *
 * Serves Tauri-compatible update manifests by proxying the latest
 * published GitHub Release for the correct platform and architecture.
 *
 * Request path: /:target/:arch/:current_version
 *   target  — "darwin" | "windows"
 *   arch    — "aarch64" | "x86_64"
 *   current_version — e.g. "1.0.0" (Tauri compares locally; ignored here)
 *
 * Deploy: wrangler deploy
 * Local dev: wrangler dev
 *
 * SECURITY NOTE: Pin tauri-apps/tauri-action in release.yml to a full commit
 * SHA before production use. The GITHUB_REPO var is set at deploy time and is
 * not user-controlled, so SSRF risk is operational rather than exploitable.
 */

interface Env {
  GITHUB_REPO: string;
  /** Optional: add as a Worker Secret for 5 000 req/hr vs 60 req/hr unauthenticated. */
  GITHUB_TOKEN?: string;
}

interface GitHubAsset {
  name: string;
  browser_download_url: string;
}

interface GitHubRelease {
  tag_name: string;
  published_at: string;
  body: string;
  assets: GitHubAsset[];
}

function getAssetPatterns(target: string): { bundle: RegExp; sig: RegExp } {
  switch (target) {
    case 'darwin':
      return {
        bundle: /\.app\.tar\.gz$/,
        sig: /\.app\.tar\.gz\.sig$/,
      };
    case 'windows':
      // Match the NSIS installer .exe — tauri-action uploads the .exe directly;
      // we re-sign it in CI and upload the .exe.sig alongside it.
      return {
        bundle: /WhisperDesk.*-setup\.exe$/i,
        sig: /WhisperDesk.*-setup\.exe\.sig$/i,
      };
    default:
      throw new Error(`Unsupported target: ${target}`);
  }
}

/** Normalize arch variants: Tauri sends "x86_64"; NSIS bundles use "x64". */
function archVariants(arch: string): string[] {
  if (arch === 'x86_64') return ['x86_64', 'x64'];
  if (arch === 'x64') return ['x64', 'x86_64'];
  // aarch64 on Windows: not yet in release matrix, but handled correctly when added
  return [arch];
}

function filterByArch(assets: GitHubAsset[], arch: string): GitHubAsset[] {
  const variants = archVariants(arch);
  return assets.filter(a => variants.some(v => a.name.includes(v)));
}

function jsonResponse(body: unknown, status: number, extra?: Record<string, string>): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      'Content-Type': 'application/json',
      'Access-Control-Allow-Origin': '*',
      ...extra,
    },
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const parts = url.pathname.split('/').filter(Boolean);

    if (parts.length < 2) {
      return jsonResponse({ error: 'Usage: /:target/:arch/:current_version' }, 400);
    }

    const [target, arch] = parts;

    const ghHeaders: Record<string, string> = {
      'User-Agent': 'WhisperDesk-Updater/1.0',
      Accept: 'application/vnd.github.v3+json',
    };
    if (env.GITHUB_TOKEN) {
      ghHeaders['Authorization'] = `Bearer ${env.GITHUB_TOKEN}`;
    }

    const ghResponse = await fetch(
      `https://api.github.com/repos/${env.GITHUB_REPO}/releases/latest`,
      { headers: ghHeaders }
    );

    // 404 from GitHub means no published release exists yet (drafts are excluded)
    if (ghResponse.status === 404) {
      return new Response(null, {
        status: 204,
        headers: { 'Access-Control-Allow-Origin': '*' },
      });
    }

    if (!ghResponse.ok) {
      // Do NOT forward GitHub error bodies — they may leak token scopes or rate-limit info
      return jsonResponse({ error: 'Failed to fetch release info' }, 502);
    }

    const release: GitHubRelease = await ghResponse.json();
    const version = release.tag_name.replace(/^v/, '');

    let patterns: ReturnType<typeof getAssetPatterns>;
    try {
      patterns = getAssetPatterns(target);
    } catch {
      return jsonResponse({ error: `Unsupported target: ${target}` }, 404);
    }

    const archAssets = filterByArch(release.assets, arch);
    const bundleAsset = archAssets.find(a => patterns.bundle.test(a.name));
    // Sig files may not include the arch in their name (e.g. macOS .app.tar.gz.sig),
    // so search all assets for the sig pattern rather than only arch-filtered ones.
    const sigAsset = release.assets.find(a => patterns.sig.test(a.name));

    if (!bundleAsset || !sigAsset) {
      return jsonResponse(
        { error: `No matching assets for ${target}/${arch}` },
        404
      );
    }

    // Fetch the signature text (small file, ~100 bytes)
    const sigResponse = await fetch(sigAsset.browser_download_url, {
      headers: { 'User-Agent': 'WhisperDesk-Updater/1.0' },
      redirect: 'follow',
    });
    if (!sigResponse.ok) {
      return jsonResponse(
        { error: `Failed to fetch update signature for ${target}/${arch}` },
        502
      );
    }
    const signature = (await sigResponse.text()).trim();

    const updatePayload = {
      version,
      notes: release.body ?? '',
      pub_date: release.published_at,
      platforms: {
        [`${target}-${arch}`]: {
          signature,
          url: bundleAsset.browser_download_url,
        },
      },
    };

    return jsonResponse(updatePayload, 200, {
      'Cache-Control': 'public, max-age=300',
    });
  },
};
