import { useState } from "react";
import ReactMarkdown, { type Components } from "react-markdown";
import remarkGfm from "remark-gfm";
import type { Note, NoteBlock } from "../types";
import { api } from "../api";
import { Btn } from "./ui";

interface Props {
  note: Note;
  onNoteChanged: (note: Note) => void;
  /** Zoom-in: select an AI block's source transcript segments. */
  onZoom: (segmentIds: string[]) => void;
}

/* Markdown → design-language mapping shared by every block: headings become
   uppercase clay mini-headings, bullets become 5px coral dots. */
const MD_COMPONENTS: Components = {
  h1: ({ children }) => (
    <h1 className="mb-2 mt-4 text-[11px] font-semibold uppercase tracking-[0.08em] text-clay">
      {children}
    </h1>
  ),
  h2: ({ children }) => (
    <h2 className="mb-2 mt-4 text-[11px] font-semibold uppercase tracking-[0.08em] text-clay">
      {children}
    </h2>
  ),
  h3: ({ children }) => (
    <h3 className="mb-2 mt-4 text-[11px] font-semibold uppercase tracking-[0.08em] text-clay">
      {children}
    </h3>
  ),
  p: ({ children }) => <p className="my-1 text-[15px] leading-[1.65] text-ink">{children}</p>,
  ul: ({ children }) => <ul className="my-1 list-none p-0">{children}</ul>,
  ol: ({ children }) => <ol className="my-1 list-none p-0">{children}</ol>,
  li: ({ children }) => (
    <li className="my-1 flex gap-[11px]">
      <span className="relative top-[9px] h-[5px] w-[5px] flex-none rounded-full bg-coral" />
      <span className="min-w-0 flex-1">{children}</span>
    </li>
  ),
  strong: ({ children }) => <strong className="font-semibold text-ink">{children}</strong>,
  a: ({ href, title, children }) => (
    <a href={href} title={title} className="text-clay underline">
      {children}
    </a>
  ),
  code: ({ children }) => (
    <code className="rounded bg-peach-2 px-1 font-mono text-[12.5px]">{children}</code>
  ),
};

/** The enhanced document: provenance-colored blocks. Your text is plain;
 *  AI text is tinted and cites its transcript sources (click to zoom).
 *  Editing an AI block reclaims it as your text. */
export default function EnhancedDoc({ note, onNoteChanged, onZoom }: Props) {
  const [editing, setEditing] = useState<{ id: string; markdown: string } | null>(null);
  const [saving, setSaving] = useState(false);

  const saveEdit = async () => {
    if (!editing) return;
    setSaving(true);
    try {
      const updated = await api.editNoteBlock(note.id, editing.id, editing.markdown);
      onNoteChanged(updated);
      setEditing(null);
    } catch (e) {
      console.error(e);
    } finally {
      setSaving(false);
    }
  };

  /* Provenance styling: user words stay plain ink (transparent rule keeps the
     text column aligned); AI blocks are peach-tinted with a coral left rule.
     The AI card's -mx/px pair keeps its text on the same column. */
  const blockClass = (b: NoteBlock) =>
    b.origin.kind === "user"
      ? "border-l-2 border-transparent pl-3 text-ink"
      : "-mx-1 rounded-lg border-l-2 border-coral/60 bg-peach-2 px-4 py-2 text-ink";

  return (
    <div className="flex-1 overflow-y-auto bg-surface px-6 py-4">
      {note.blocks.map((b) => (
        <div key={b.id} className="group mb-3">
          {editing?.id === b.id ? (
            <div>
              <textarea
                autoFocus
                value={editing.markdown}
                onChange={(e) => setEditing({ id: b.id, markdown: e.target.value })}
                onKeyDown={(e) => {
                  if (e.key === "Escape") setEditing(null);
                  if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) void saveEdit();
                }}
                rows={Math.max(3, editing.markdown.split("\n").length + 1)}
                className="w-full rounded-lg border border-line bg-cream p-3 text-[14px] text-ink outline-none focus:border-coral"
              />
              <div className="mt-1.5 flex items-center gap-2">
                <Btn variant="primary" size="xs" onClick={() => void saveEdit()} disabled={saving}>
                  {saving ? "Saving…" : "Save (Ctrl+Enter)"}
                </Btn>
                <Btn variant="ghost" size="xs" onClick={() => setEditing(null)}>
                  Cancel
                </Btn>
                {b.origin.kind === "ai" && (
                  <span className="text-[11px] text-mute">editing makes this your text</span>
                )}
              </div>
            </div>
          ) : (
            <div className={blockClass(b)}>
              <ReactMarkdown remarkPlugins={[remarkGfm]} components={MD_COMPONENTS}>
                {b.markdown}
              </ReactMarkdown>
              <div className="mt-1 hidden items-center gap-2 text-[11px] text-mute group-hover:flex">
                <button
                  className="cursor-pointer font-semibold hover:text-ink"
                  onClick={() => setEditing({ id: b.id, markdown: b.markdown })}
                >
                  Edit
                </button>
                {b.origin.kind === "ai" ? (
                  b.origin.source_segment_ids.length > 0 ? (
                    <button
                      className="inline-flex h-[17px] min-w-[17px] cursor-pointer items-center justify-center rounded-[5px] bg-peach px-1 font-mono text-[10px] font-semibold text-clay hover:brightness-105"
                      title="Show the transcript this came from"
                      onClick={() => b.origin.kind === "ai" && onZoom(b.origin.source_segment_ids)}
                    >
                      {b.origin.source_segment_ids.length} source
                      {b.origin.source_segment_ids.length > 1 ? "s" : ""}
                    </button>
                  ) : (
                    <span className="text-clay/70">AI (no source)</span>
                  )
                ) : (
                  <span>your text</span>
                )}
              </div>
            </div>
          )}
        </div>
      ))}
    </div>
  );
}
