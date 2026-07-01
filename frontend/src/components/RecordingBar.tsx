import type { RecordingStatus } from "../types";

interface Props {
  status: RecordingStatus;
  noteTitle: string | null;
  onPause: () => void;
  onResume: () => void;
  onStop: () => void;
  onOpenNote: () => void;
}

export function fmtElapsed(ms: number): string {
  const s = Math.floor(ms / 1000);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = s % 60;
  return h > 0
    ? `${h}:${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`
    : `${m}:${String(sec).padStart(2, "0")}`;
}

/** Always-visible consent/recording indicator (spec §9): while a capture is
 *  live this bar is pinned above everything, on every view. */
export default function RecordingBar({
  status,
  noteTitle,
  onPause,
  onResume,
  onStop,
  onOpenNote,
}: Props) {
  if (!status.active) return null;
  const paused = status.state === "paused";

  return (
    <div className="flex items-center gap-3 border-b border-red-900/50 bg-red-950/60 px-4 py-1.5 text-sm">
      <span className="relative flex h-2.5 w-2.5">
        {!paused && (
          <span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-red-400 opacity-75" />
        )}
        <span
          className={`relative inline-flex h-2.5 w-2.5 rounded-full ${
            paused ? "bg-amber-400" : "bg-red-500"
          }`}
        />
      </span>
      <span className="font-medium text-red-200">{paused ? "Recording paused" : "Recording"}</span>
      <span className="tabular-nums text-red-300">{fmtElapsed(status.elapsed_ms)}</span>
      {noteTitle && (
        <button
          className="truncate text-red-300/80 underline-offset-2 hover:underline"
          onClick={onOpenNote}
        >
          {noteTitle}
        </button>
      )}
      <div className="ml-auto flex items-center gap-2">
        {paused ? (
          <button
            onClick={onResume}
            className="rounded border border-red-800 px-2 py-0.5 text-xs text-red-200 hover:bg-red-900/50"
          >
            ▶ Resume
          </button>
        ) : (
          <button
            onClick={onPause}
            className="rounded border border-red-800 px-2 py-0.5 text-xs text-red-200 hover:bg-red-900/50"
          >
            ⏸ Pause
          </button>
        )}
        <button
          onClick={onStop}
          className="rounded bg-red-600 px-2.5 py-0.5 text-xs font-medium text-white hover:bg-red-500"
        >
          ■ Stop
        </button>
      </div>
    </div>
  );
}
