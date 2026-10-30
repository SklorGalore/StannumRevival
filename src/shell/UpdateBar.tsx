import { AlertTriangle, ArrowDownCircle, CheckCircle2, ExternalLink, Loader2, X } from "lucide-react";
import { RELEASES_URL, useUpdate } from "../store/updateStore";

const mb = (bytes: number) => `${(bytes / 1048576).toFixed(1)} MB`;

/** First lines of the release notes, as plain text: enough to say what's new. Headings are left out. */
export function notesPreview(notes: string, lines = 3): string[] {
  return notes
    .split(/\r?\n/)
    .filter((l) => !/^\s*#/.test(l))
    .map((l) => l.replace(/^\s*[-*]\s+/, "").replace(/\*\*|__|`/g, "").trim())
    .filter(Boolean)
    .slice(0, lines);
}

async function openLink(url: string) {
  try {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } catch {
    window.open(url, "_blank", "noopener");
  }
}

/** Under the case bar: a newer Stannum, its download, or how a check went. */
export function UpdateBar() {
  const phase = useUpdate((s) => s.phase);
  const update = useUpdate((s) => s.update);
  const received = useUpdate((s) => s.received);
  const total = useUpdate((s) => s.total);
  const error = useUpdate((s) => s.error);
  const u = useUpdate.getState();
  if (phase === "idle") return null;

  const base = "shrink-0 flex items-start gap-2.5 border-b px-4 py-2 text-xs";
  const teal = `${base} border-teal-100 bg-teal-50 text-teal-900`;
  const close = (
    <button type="button" onClick={u.dismiss} className="shrink-0 rounded p-0.5 hover:bg-black/5" aria-label="Dismiss" title="Dismiss">
      <X size={13} />
    </button>
  );
  const button = "inline-flex items-center gap-1 rounded-md px-2 py-0.5 font-medium";

  if (phase === "checking")
    return (
      <div role="status" data-testid="update-bar" className={teal}>
        <Loader2 size={14} className="mt-0.5 shrink-0 animate-spin" aria-hidden="true" />
        <p className="flex-1">Checking for a newer Stannum…</p>
      </div>
    );

  if (phase === "current")
    return (
      <div role="status" data-testid="update-bar" className={teal}>
        <CheckCircle2 size={14} className="mt-0.5 shrink-0" />
        <p className="flex-1">Stannum {__APP_VERSION__} is the latest version.</p>
        {close}
      </div>
    );

  if (phase === "error")
    return (
      <div role="alert" data-testid="update-bar" className={`${base} border-amber-200 bg-amber-50 text-amber-900`}>
        <AlertTriangle size={14} className="mt-0.5 shrink-0" />
        <p className="min-w-0 flex-1 break-words">{error}</p>
        <button type="button" onClick={() => void openLink(RELEASES_URL)} className={`${button} shrink-0 border border-amber-300 bg-white hover:bg-amber-100`}>
          <ExternalLink size={12} /> Releases page
        </button>
        {close}
      </div>
    );

  if (!update) return null;

  if (phase === "downloading" || phase === "restarting") {
    const share = total ? Math.min(1, received / total) : null;
    return (
      <div role="status" data-testid="update-bar" className={teal}>
        <Loader2 size={14} className="mt-0.5 shrink-0 animate-spin" aria-hidden="true" />
        <div className="min-w-0 flex-1">
          <p className="font-semibold">
            {phase === "restarting" ? `Restarting into Stannum ${update.version}…` : `Downloading Stannum ${update.version}…`}
          </p>
          {phase === "downloading" && (
            <div className="mt-1 flex items-center gap-2">
              <div
                className="h-1.5 w-48 overflow-hidden rounded-full bg-teal-100"
                role="progressbar"
                aria-label="Download"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={share === null ? undefined : Math.round(share * 100)}
              >
                <div className={`h-full bg-teal-600 ${share === null ? "w-1/3 animate-pulse" : ""}`} style={share === null ? undefined : { width: `${share * 100}%` }} />
              </div>
              <span className="tabular-nums text-teal-800/80">{total ? `${mb(received)} of ${mb(total)}` : mb(received)}</span>
            </div>
          )}
        </div>
      </div>
    );
  }

  const preview = notesPreview(update.notes);
  return (
    <div role="status" data-testid="update-bar" className={teal}>
      <ArrowDownCircle size={14} className="mt-0.5 shrink-0" />
      <div className="min-w-0 flex-1">
        <p className="font-semibold">
          Stannum {update.version} is available. You have {update.current}.
        </p>
        {preview.map((line, i) => (
          <p key={i} className="mt-0.5 truncate text-teal-800/80">
            {line}
          </p>
        ))}
      </div>
      <span className="ml-auto flex shrink-0 items-center gap-1.5">
        <button
          type="button"
          onClick={() => void u.install()}
          className={`${button} bg-teal-700 text-white hover:bg-teal-800`}
          data-testid="install-update"
          title="Download the update, install it and restart Stannum"
        >
          <ArrowDownCircle size={12} /> Install and restart
        </button>
        <button
          type="button"
          onClick={() => void openLink(`${RELEASES_URL}/tag/v${update.version}`)}
          className={`${button} border border-teal-300 bg-white hover:bg-teal-100`}
          title="Open the release page"
        >
          <ExternalLink size={12} /> What's new
        </button>
        <button type="button" onClick={u.skip} className={`${button} hover:bg-teal-100`} title="Don't offer this version again when Stannum starts">
          Skip this version
        </button>
      </span>
      {close}
    </div>
  );
}
