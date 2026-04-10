/**
 * WhisperDesk auto-updater endpoint.
 *
 * Serves Tauri-compatible update manifests by proxying the latest
 * published GitHub Release for the correct platform and architecture.
 *
 * Request path: /:target/:arch/:current_version
 *   target  — "darwin" | "windows"
 *   arch    — "aarch64" | "x86_64"
 *   current_version — e.g. "1.0.0" (used for logging; Tauri compares locally)
 *
 * Deploy: wrangler deploy
 * Local dev: wrangler dev
 */

interface Env {
  GITHUB_REPO: string;
  // Optional: add GITHUB_TOKEN as a Worker secret for higher API rate limits
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
      return {
        bundle: /nsis.*\.zip$/,
        sig: /nsis.*\.zip\.sig$/,
      };
    default:
      throw new Error(`Unsupported target: ${target}`);
  }
}

/** Normalize arch variants: Tauri sends "x86_64", NSIS bundles use "x64". */
function archVariants(arch: string): string[] {
  if (arch === 'x86_64') return ['x86_64', 'x64'];
  if (arch === 'x64') return ['x64', 'x86_64'];
  return [arch];
}

function filterByArch(assets: GitHubAsset[], arch: string): GitHubAsset[] {
  const variants = archVariants(arch);
  return assets.filter(a => variants.some(v => a.name.includes(v)));
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const parts = url.pathname.split('/').filter(Boolean);

    if (parts.length < 2) {
      return new Response(
        JSON.stringify({ error: 'Usage: /:target/:arch/:current_version' }),
        { status: 400, headers: { 'Content-Type': 'application/json' } }
      );
    }

    const [target, arch] = parts;

    const headers: Record<string, string> = {
      'User-Agent': 'WhisperDesk-Updater/1.0',
      Accept: 'application/vnd.github.v3+json',
    };
    if (env.GITHUB_TOKEN) {
      headers['Authorization'] = `Bearer ${env.GITHUB_TOKEN}`;
    }

    const ghResponse = await fetch(
      `https://api.github.com/repos/${env.GITHUB_REPO}/releases/latest`,
      { headers }
    );

    if (!ghResponse.ok) {
      const body = await ghResponse.text();
      return new Response(
        JSON.stringify({ error: 'Failed to fetch GitHub release', detail: body }),
        { status: 502, headers: { 'Content-Type': 'application/json' } }
      );
    }

    const release: GitHubRelease = await ghResponse.json();
    const version = release.tag_name.replace(/^v/, '');

    let patterns: ReturnType<typeof getAssetPatterns>;
    try {
      patterns = getAssetPatterns(target);
    } catch {
      return new Response(
        JSON.stringify({ error: `Unsupported target: ${target}` }),
        { status: 404, headers: { 'Content-Type': 'application/json' } }
      );
    }

    const archAssets = filterByArch(release.assets, arch);
    const bundleAsset = archAssets.find(a => patterns.bundle.test(a.name));
    const sigAsset = archAssets.find(a => patterns.sig.test(a.name));

    if (!bundleAsset || !sigAsset) {
      return new Response(
        JSON.stringify({
          error: `No matching assets for ${target}/${arch}`,
          available: release.assets.map(a => a.name),
        }),
        { status: 404, headers: { 'Content-Type': 'application/json' } }
      );
    }

    // Fetch the signature text (small file, ~100 bytes)
    const sigResponse = await fetch(sigAsset.browser_download_url, {
      headers: { 'User-Agent': 'WhisperDesk-Updater/1.0' },
      redirect: 'follow',
    });
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

    return new Response(JSON.stringify(updatePayload), {
      headers: {
        'Content-Type': 'application/json',
        'Cache-Control': 'public, max-age=300',
        'Access-Control-Allow-Origin': '*',
      },
    });
  },
};
