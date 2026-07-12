import { useState } from "react";
import { Button, Card, Checkbox, Modal } from "./ui";

interface Props {
  onAccept: () => void;
}

/** First-run consent + legal note (spec §9): recording laws differ by
 *  jurisdiction; the user acknowledges once before the first recording
 *  feature is available. Consent is required, so the modal has no close
 *  affordance and can't be dismissed by overlay/Esc — only "Got it". */
export default function FirstRunNotice({ onAccept }: Props) {
  const [checked, setChecked] = useState(false);

  return (
    <Modal open onClose={undefined} title="Before you record" width={520} closeOnOverlay={false}>
      <p style={{ margin: "0 0 12px", fontSize: 14, lineHeight: 1.55, color: "var(--text-2)" }}>
        Fly on the Wall records your microphone <em>and</em> the audio of other meeting participants
        (system audio). Everything is stored{" "}
        <b style={{ fontWeight: 600, color: "var(--text)" }}>only on this computer</b> — nothing is
        uploaded unless you explicitly enable a cloud provider.
      </p>
      <Card tone="muted" pad="md" style={{ marginBottom: 14 }}>
        <p style={{ margin: "0 0 4px", fontWeight: 600, fontSize: 13, color: "var(--text)" }}>
          Recording laws vary by jurisdiction.
        </p>
        <p style={{ margin: 0, fontSize: 13, lineHeight: 1.5, color: "var(--text-2)" }}>
          Many places require the consent of <em>all</em> participants before recording a
          conversation. It is your responsibility to know the rules that apply to you and to tell
          people they are being recorded. Fly on the Wall shows a persistent on-screen indicator
          while recording.
        </p>
      </Card>
      <Checkbox
        checked={checked}
        onChange={(e) => setChecked(e.target.checked)}
        label="I understand that I am responsible for complying with the recording-consent laws that apply to my meetings."
        style={{ marginBottom: 16 }}
      />
      <div style={{ display: "flex", justifyContent: "flex-end" }}>
        <Button variant="primary" size="md" disabled={!checked} onClick={onAccept}>
          Got it
        </Button>
      </div>
    </Modal>
  );
}
