export function DownloadButton() {
  return (
    <a
      href="/api/installer"
      className="inline-flex min-h-11 items-center justify-center border border-accent bg-bg-elevated px-5 font-display text-xl tracking-wide text-fg transition-colors duration-200 hover:bg-bg-subtle focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
    >
      Download for Windows
    </a>
  );
}
