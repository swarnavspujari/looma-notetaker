import { useEffect, useState } from "react";
import { api } from "../api";
import type { AsrSettings, ModelProgress } from "../types";

interface Props {
  modelProgress: ModelProgress | null;
  onClose: () => void;
}

const TIERS = [
  { id: "light", label: "Light", desc: "Whisper small (Q5) — ~2 GB RAM, fine on old laptops" },
  { id: "balanced", label: "Balanced", desc: "large-v3-turbo (Q5) — near-large accuracy on CPU" },
  { id: "best", label: "Best", desc: "large-v3-turbo (Q5), large-v3 with max-quality toggle" },
  { id: "cloud", label: "Cloud (Groq)", desc: "For weak devices — audio LEAVES this machine" },
];

function gb(bytes: number): string {
  return bytes >= 1_000_000_000
    ? `${(bytes / 1_000_000_000).toFixed(1)} GB`
    : `${Math.round(bytes / 1_000_000)} MB`;
}

export default function SettingsModal({ modelProgress, onClose }: Props) {
  const [settings, setSettings] = useState<AsrSettings | null>(null);
  const [tier, setTier] = useState("balanced");
  const [modelId, setModelId] = useState<string>("");
  const [useGroq, setUseGroq] = useState(false);
  const [maxQuality, setMaxQuality] = useState(false);
  const [autoTranscribe, setAutoTranscribe] = useState(true);
  const [groqKey, setGroqKey] = useState("");
  const [saving, setSaving] = useState(false);
  const [downloading, setDownloading] = useState<string | null>(null);

  const load = () =>
    api.getAsrSettings().then((s) => {
      setSettings(s);
      setTier(s.tier);
      setModelId(s.model_id ?? "");
      setUseGroq(s.use_groq);
      setMaxQuality(s.max_quality);
      setAutoTranscribe(s.auto_transcribe);
    });

  useEffect(() => {
    load().catch(console.error);
  }, []);

  const save = async () => {
    setSaving(true);
    try {
      await api.setAsrSettings({
        tier,
        model_id: modelId || null,
        use_groq: useGroq,
        max_quality: maxQuality,
        auto_transcribe: autoTranscribe,
        groq_key: groqKey ? groqKey : null,
      });
      onClose();
    } finally {
      setSaving(false);
    }
  };

  const download = async (id: string) => {
    setDownloading(id);
    try {
      await api.downloadModel(id);
      await load();
    } catch (e) {
      console.error(e);
    } finally {
      setDownloading(null);
    }
  };

  const asrModels = settings?.models.filter((m) => m.id.startsWith("ggml-")) ?? [];
  const otherModels = settings?.models.filter((m) => !m.id.startsWith("ggml-")) ?? [];

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        className="max-h-[85vh] w-[560px] overflow-y-auto rounded-lg border border-zinc-700 bg-zinc-900 p-5 text-sm text-zinc-200 shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="mb-3 flex items-center justify-between">
          <h2 className="text-lg font-semibold">Transcription settings</h2>
          <button className="text-zinc-500 hover:text-zinc-200" onClick={onClose}>
            ✕
          </button>
        </div>

        {settings && (
          <div className="mb-4 rounded-md bg-zinc-800/70 px-3 py-2 text-xs text-zinc-400">
            This machine: {settings.hw.cpu_cores} cores · {settings.hw.ram_gb} GB RAM
            {settings.hw.gpu_name
              ? ` · ${settings.hw.gpu_name} (${Math.round((settings.hw.vram_mb ?? 0) / 1024)} GB VRAM)`
              : " · no NVIDIA GPU"}
            {" — recommended tier: "}
            <span className="font-medium text-zinc-200 capitalize">
              {settings.hw.recommended_tier}
            </span>
          </div>
        )}

        <div className="mb-4">
          <div className="mb-1 font-medium">Hardware tier</div>
          {TIERS.map((t) => (
            <label key={t.id} className="mb-1 flex cursor-pointer items-start gap-2">
              <input
                type="radio"
                checked={tier === t.id}
                onChange={() => setTier(t.id)}
                className="mt-1"
              />
              <span>
                <span className="font-medium">{t.label}</span>
                <span className="ml-2 text-xs text-zinc-500">{t.desc}</span>
              </span>
            </label>
          ))}
        </div>

        <div className="mb-4 flex items-center gap-3">
          <div className="font-medium">Model</div>
          <select
            value={modelId}
            onChange={(e) => setModelId(e.target.value)}
            className="rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-xs"
          >
            <option value="">Auto (per tier)</option>
            {asrModels.map((m) => (
              <option key={m.id} value={m.id}>
                {m.display}
              </option>
            ))}
          </select>
        </div>

        <label className="mb-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={maxQuality}
            onChange={(e) => setMaxQuality(e.target.checked)}
          />
          Maximum quality (full large-v3 — slower, ~1 GB download)
        </label>
        <label className="mb-2 flex items-center gap-2">
          <input
            type="checkbox"
            checked={autoTranscribe}
            onChange={(e) => setAutoTranscribe(e.target.checked)}
          />
          Transcribe automatically when a recording stops
        </label>

        <div className="mb-4 mt-3 rounded-md border border-amber-900/60 bg-amber-950/30 p-3">
          <label className="flex items-center gap-2 font-medium">
            <input
              type="checkbox"
              checked={useGroq}
              onChange={(e) => setUseGroq(e.target.checked)}
            />
            Use Groq for transcription (cloud fallback)
          </label>
          <p className="mt-1 text-xs text-amber-200/80">
            When enabled, meeting audio is uploaded to Groq for transcription. Who-said-what
            (diarization) still runs locally. Bring your own key — free tier available.
          </p>
          <input
            type="password"
            placeholder={settings?.has_groq_key ? "Groq API key saved — enter to replace" : "gsk_…"}
            value={groqKey}
            onChange={(e) => setGroqKey(e.target.value)}
            className="mt-2 w-full rounded border border-zinc-700 bg-zinc-800 px-2 py-1 text-xs outline-none focus:border-indigo-500"
          />
        </div>

        <div className="mb-4">
          <div className="mb-1 font-medium">Models & engines on disk</div>
          {[...asrModels, ...otherModels].map((m) => (
            <div key={m.id} className="flex items-center justify-between py-1 text-xs">
              <span className="truncate text-zinc-400">{m.display}</span>
              <span className="ml-2 flex shrink-0 items-center gap-2">
                <span className="text-zinc-600">{gb(m.bytes)}</span>
                {m.installed ? (
                  <span className="text-emerald-400">installed</span>
                ) : downloading === m.id ? (
                  <span className="text-indigo-300">
                    {modelProgress && modelProgress.id === m.id && modelProgress.total > 0
                      ? `${Math.round((modelProgress.downloaded / modelProgress.total) * 100)}%`
                      : "downloading…"}
                  </span>
                ) : (
                  <button
                    className="rounded border border-zinc-700 px-2 py-0.5 text-zinc-300 hover:bg-zinc-800"
                    onClick={() => void download(m.id)}
                  >
                    download
                  </button>
                )}
              </span>
            </div>
          ))}
        </div>

        <div className="flex justify-end gap-2">
          <button
            className="rounded border border-zinc-700 px-3 py-1.5 text-zinc-300 hover:bg-zinc-800"
            onClick={onClose}
          >
            Cancel
          </button>
          <button
            className="rounded bg-indigo-600 px-3 py-1.5 font-medium text-white hover:bg-indigo-500 disabled:opacity-50"
            disabled={saving}
            onClick={() => void save()}
          >
            {saving ? "Saving…" : "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
