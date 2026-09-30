import { createReadStream, existsSync, statSync } from "node:fs";
import { Readable } from "node:stream";
import { join } from "node:path";
import { createFileRoute } from "@tanstack/react-router";

const REPO = "hijoelkim/daylight";
const FILE_NAME = "Daylight-Setup.exe";

const attachment = {
  "content-type": "application/octet-stream",
  "content-disposition": `attachment; filename="${FILE_NAME}"`,
  "cache-control": "no-store",
};

export const Route = createFileRoute("/api/installer")({
  server: {
    handlers: {
      GET: async () => {
        const released = await fromGithub();
        if (released) return released;
        const local = fromDisk();
        if (local) return local;
        return new Response(
          "<!doctype html><meta charset=utf-8><title>Daylight</title><p>The installer is not up yet. Tag a v* release on the desktop repo.</p>",
          { status: 503, headers: { "content-type": "text/html; charset=utf-8", "cache-control": "no-store" } },
        );
      },
    },
  },
});

async function fromGithub(): Promise<Response | null> {
  const headers: Record<string, string> = {
    Accept: "application/vnd.github+json",
    "User-Agent": "daylight",
  };
  if (process.env.GITHUB_TOKEN) headers.Authorization = `Bearer ${process.env.GITHUB_TOKEN}`;
  try {
    const listed = await fetch(`https://api.github.com/repos/${REPO}/releases/latest`, { headers });
    if (!listed.ok) return null;
    const body = (await listed.json()) as { assets?: Array<{ name?: string; browser_download_url?: string }> };
    const url = body.assets?.find((asset) => asset.name === FILE_NAME)?.browser_download_url;
    if (!url) return null;
    const file = await fetch(url, { headers: { ...headers, Accept: "application/octet-stream" } });
    if (!file.ok || !file.body) return null;
    return new Response(file.body, { headers: attachment });
  } catch {
    return null;
  }
}

function fromDisk(): Response | null {
  const path = join(process.cwd(), "public", "downloads", FILE_NAME);
  if (!existsSync(path)) return null;
  const size = statSync(path).size;
  const stream = Readable.toWeb(createReadStream(path)) as ReadableStream;
  return new Response(stream, { headers: { ...attachment, "content-length": String(size) } });
}
