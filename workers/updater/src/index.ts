/**
 * WhisperDesk auto-updater endpoint.
 *
 * Serves Tauri-compatible update manifests by proxying the latest
 * published GitHub Release for the correct platform and architecture.
 *
 * Request path: /:target/:arch/:current_version
 *   target  — "darwin" | "windows"
 *   arch    — "aarch64" | "x86_64"
 *   current_version — e.g. "1.0.0"
 *
 * Deploy: wrangler deploy
 * Local dev: wrangler dev
 *
 * GITHUB_REPO is configured at deploy time rather than from request input, so
 * clients cannot redirect the Worker to an arbitrary upstream host.
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
    case "darwin":
      return {
        bundle: /\.app\.tar\.gz$/,
        sig: /\.app\.tar\.gz\.sig$/,
      };
    case "windows":
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
  if (arch === "x86_64") return ["x86_64", "x64"];
  if (arch === "x64") return ["x64", "x86_64"];
  // aarch64 on Windows: not yet in release matrix, but handled correctly when added
  return [arch];
}

function filterByArch(assets: GitHubAsset[], arch: string): GitHubAsset[] {
  const variants = archVariants(arch);
  return assets.filter((a) => variants.some((v) => a.name.includes(v)));
}

interface SemVer {
  major: number;
  minor: number;
  patch: number;
  prerelease: string[];
}

/** Parse the SemVer forms accepted by Tauri, including an optional leading v. */
function parseSemVer(value: string): SemVer | null {
  const match = value
    .trim()
    .match(/^[vV]?(\d+)\.(\d+)\.(\d+)(?:-([0-9A-Za-z.-]+))?(?:\+[0-9A-Za-z.-]+)?$/);
  if (!match) return null;

  const major = Number(match[1]);
  const minor = Number(match[2]);
  const patch = Number(match[3]);
  if (![major, minor, patch].every(Number.isSafeInteger)) return null;

  return {
    major,
    minor,
    patch,
    prerelease: match[4]?.split(".") ?? [],
  };
}

/** Compare two valid SemVer strings. Returns null when either value is invalid. */
function compareSemVer(left: string, right: string): number | null {
  const a = parseSemVer(left);
  const b = parseSemVer(right);
  if (!a || !b) return null;

  for (const key of ["major", "minor", "patch"] as const) {
    if (a[key] !== b[key]) return a[key] < b[key] ? -1 : 1;
  }

  if (a.prerelease.length === 0 || b.prerelease.length === 0) {
    if (a.prerelease.length === b.prerelease.length) return 0;
    return a.prerelease.length === 0 ? 1 : -1;
  }

  const length = Math.max(a.prerelease.length, b.prerelease.length);
  for (let i = 0; i < length; i += 1) {
    const aPart = a.prerelease[i];
    const bPart = b.prerelease[i];
    if (aPart === undefined || bPart === undefined) {
      return aPart === undefined ? -1 : 1;
    }
    if (aPart === bPart) continue;

    const aNumeric = /^\d+$/.test(aPart);
    const bNumeric = /^\d+$/.test(bPart);
    if (aNumeric && bNumeric) {
      const aNumber = Number(aPart);
      const bNumber = Number(bPart);
      if (aNumber !== bNumber) return aNumber < bNumber ? -1 : 1;
    } else if (aNumeric !== bNumeric) {
      return aNumeric ? -1 : 1;
    } else {
      return aPart < bPart ? -1 : 1;
    }
  }

  return 0;
}

function noContentResponse(): Response {
  return new Response(null, {
    status: 204,
    headers: { "Access-Control-Allow-Origin": "*" },
  });
}

function jsonResponse(body: unknown, status: number, extra?: Record<string, string>): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: {
      "Content-Type": "application/json",
      "Access-Control-Allow-Origin": "*",
      ...extra,
    },
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const url = new URL(request.url);
    const parts = url.pathname.split("/").filter(Boolean);

    if (parts.length < 3) {
      return jsonResponse({ error: "Usage: /:target/:arch/:current_version" }, 400);
    }

    const [target, arch, currentVersion] = parts;
    let patterns: ReturnType<typeof getAssetPatterns>;
    try {
      patterns = getAssetPatterns(target);
    } catch {
      return jsonResponse({ error: `Unsupported target: ${target}` }, 404);
    }

    const ghHeaders: Record<string, string> = {
      "User-Agent": "WhisperDesk-Updater/1.0",
      Accept: "application/vnd.github.v3+json",
    };
    if (env.GITHUB_TOKEN) {
      ghHeaders["Authorization"] = `Bearer ${env.GITHUB_TOKEN}`;
    }

    const ghResponse = await fetch(
      `https://api.github.com/repos/${env.GITHUB_REPO}/releases/latest`,
      { headers: ghHeaders }
    );

    // 404 from GitHub means no published release exists yet (drafts are excluded)
    if (ghResponse.status === 404) {
      return noContentResponse();
    }

    if (!ghResponse.ok) {
      // Do NOT forward GitHub error bodies — they may leak token scopes or rate-limit info
      return jsonResponse({ error: "Failed to fetch release info" }, 502);
    }

    const release: GitHubRelease = await ghResponse.json();
    const version = release.tag_name.replace(/^v/, "");

    // A dynamic updater endpoint must return 204 when this client is already
    // current (or newer, as can happen with development builds). Check this
    // before assets so an older release with incomplete bundles does not turn
    // a no-op update check into a client-visible error.
    const versionOrder = compareSemVer(currentVersion, version);
    if (versionOrder !== null && versionOrder >= 0) {
      return noContentResponse();
    }

    const archAssets = filterByArch(release.assets, arch);
    const bundleAsset = archAssets.find((a) => patterns.bundle.test(a.name));
    const expectedSignatureName = bundleAsset ? `${bundleAsset.name}.sig` : null;
    const sigAsset = expectedSignatureName
      ? release.assets.find((a) => a.name === expectedSignatureName && patterns.sig.test(a.name))
      : undefined;

    if (!bundleAsset || !sigAsset) {
      // No compatible update is available to this client. Keep the endpoint's
      // response compliant (204) while retaining an operational signal in the
      // Worker logs so incomplete releases can be fixed server-side.
      console.error("No compatible updater asset pair", {
        target,
        arch,
        version,
        bundle: bundleAsset?.name ?? null,
        expectedSignature: expectedSignatureName,
      });
      return noContentResponse();
    }

    // Fetch the signature text (small file, ~100 bytes)
    const sigResponse = await fetch(sigAsset.browser_download_url, {
      headers: { "User-Agent": "WhisperDesk-Updater/1.0" },
      redirect: "follow",
    });
    if (!sigResponse.ok) {
      return jsonResponse({ error: `Failed to fetch update signature for ${target}/${arch}` }, 502);
    }
    const signature = (await sigResponse.text()).trim();

    const updatePayload = {
      version,
      notes: release.body ?? "",
      pub_date: release.published_at,
      platforms: {
        [`${target}-${arch}`]: {
          signature,
          url: bundleAsset.browser_download_url,
        },
      },
    };

    return jsonResponse(updatePayload, 200, {
      "Cache-Control": "public, max-age=300",
    });
  },
};
