import { useState } from "react";

interface Props {
  onAccept: () => void;
}

/** First-run consent + legal note (spec §9): recording laws differ by
 *  jurisdiction; the user acknowledges once before the first recording
 *  feature is available. */
export default function FirstRunNotice({ onAccept }: Props) {
  const [checked, setChecked] = useState(false);

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/70">
      <div className="w-[520px] rounded-lg border border-zinc-700 bg-zinc-900 p-6 text-sm text-zinc-200 shadow-xl">
        <h2 className="mb-2 text-lg font-semibold">Before you record</h2>
        <p className="mb-3 text-zinc-300">
          Looma records your microphone <em>and</em> the audio of other meeting participants (system
          audio). Everything is stored{" "}
          <span className="font-medium text-zinc-100">only on this computer</span> — nothing is
          uploaded unless you explicitly enable a cloud provider.
        </p>
        <div className="mb-3 rounded-md border border-amber-900/60 bg-amber-950/30 p-3 text-xs text-amber-200/90">
          <p className="mb-1 font-medium">Recording laws vary by jurisdiction.</p>
          <p>
            Many places require the consent of <em>all</em> participants before recording a
            conversation. It is your responsibility to know the rules that apply to you and to tell
            people they are being recorded. Looma shows a persistent on-screen indicator while
            recording.
          </p>
        </div>
        <label className="mb-4 flex items-start gap-2 text-xs text-zinc-300">
          <input
            type="checkbox"
            className="mt-0.5"
            checked={checked}
            onChange={(e) => setChecked(e.target.checked)}
          />
          I understand that I am responsible for complying with the recording-consent laws that
          apply to my meetings.
        </label>
        <div className="flex justify-end">
          <button
            disabled={!checked}
            onClick={onAccept}
            className="rounded-md bg-indigo-600 px-4 py-1.5 font-medium text-white hover:bg-indigo-500 disabled:opacity-40"
          >
            Got it
          </button>
        </div>
      </div>
    </div>
  );
}
