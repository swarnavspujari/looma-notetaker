//! Transcript persistence: structured JSON in SQLite (+ FTS) and portable
//! markdown/JSON mirrors under `transcripts/`.

use looma_core::Transcript;
use rusqlite::OptionalExtension;

use crate::{Result, Storage, StorageError};

impl Storage {
    /// Upsert a meeting's transcript; keeps FTS and on-disk mirrors in sync.
    pub fn save_transcript(&self, transcript: &Transcript) -> Result<()> {
        let json = serde_json::to_string(transcript)?;
        self.conn.execute(
            "INSERT INTO transcripts (meeting_id, json) VALUES (?1, ?2)
             ON CONFLICT(meeting_id) DO UPDATE SET json = excluded.json",
            (&transcript.meeting_id, &json),
        )?;
        self.sync_transcript_derived(transcript)?;
        Ok(())
    }

    pub fn get_transcript(&self, meeting_id: &str) -> Result<Option<Transcript>> {
        let json: Option<String> = self
            .conn
            .query_row(
                "SELECT json FROM transcripts WHERE meeting_id = ?1",
                [meeting_id],
                |r| r.get(0),
            )
            .optional()?;
        match json {
            Some(j) => Ok(Some(serde_json::from_str(&j)?)),
            None => Ok(None),
        }
    }

    /// Rename a speaker's display label (stable keys never change).
    pub fn relabel_speaker(
        &self,
        meeting_id: &str,
        speaker_key: &str,
        label: &str,
    ) -> Result<Transcript> {
        let mut transcript = self
            .get_transcript(meeting_id)?
            .ok_or_else(|| StorageError::NotFound(format!("transcript for {meeting_id}")))?;
        match transcript
            .speakers
            .iter_mut()
            .find(|s| s.key == speaker_key)
        {
            Some(s) => s.label = label.to_string(),
            None => transcript.speakers.push(looma_core::Speaker {
                key: speaker_key.to_string(),
                label: label.to_string(),
            }),
        }
        self.save_transcript(&transcript)?;
        Ok(transcript)
    }

    fn sync_transcript_derived(&self, t: &Transcript) -> Result<()> {
        // FTS body: labeled lines, so a search for a speaker name hits too.
        let body: String = t
            .segments
            .iter()
            .map(|s| format!("{}: {}", t.label_for(&s.speaker_key), s.text))
            .collect::<Vec<_>>()
            .join("\n");
        self.conn.execute(
            "DELETE FROM transcripts_fts WHERE meeting_id = ?1",
            [&t.meeting_id],
        )?;
        self.conn.execute(
            "INSERT INTO transcripts_fts (meeting_id, body) VALUES (?1, ?2)",
            (&t.meeting_id, &body),
        )?;

        let dir = self.data_dir.join("transcripts");
        std::fs::write(dir.join(format!("{}.md", t.meeting_id)), t.to_markdown())?;
        std::fs::write(
            dir.join(format!("{}.json", t.meeting_id)),
            serde_json::to_string_pretty(t)?,
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::test_storage;
    use looma_core::{Speaker, Transcript, TranscriptSegment, Word};

    fn sample_transcript(meeting_id: &str) -> Transcript {
        Transcript {
            meeting_id: meeting_id.into(),
            language: Some("en".into()),
            engine: "whisper.cpp".into(),
            segments: vec![TranscriptSegment {
                id: "seg-1".into(),
                speaker_key: "spk_0".into(),
                start_ms: 0,
                end_ms: 1500,
                text: "the budget is approved".into(),
                words: vec![Word {
                    text: "budget".into(),
                    start_ms: 200,
                    end_ms: 600,
                }],
            }],
            speakers: vec![Speaker {
                key: "spk_0".into(),
                label: "Speaker 1".into(),
            }],
        }
    }

    #[test]
    fn transcript_roundtrip_files_and_search() {
        let (dir, s) = test_storage();
        let note = s.create_note("Budget mtg", None).unwrap();
        let meeting = s.create_meeting("Budget mtg", &note.id, &[]).unwrap();
        let t = sample_transcript(&meeting.id);
        s.save_transcript(&t).unwrap();

        let loaded = s.get_transcript(&meeting.id).unwrap().unwrap();
        assert_eq!(loaded.segments[0].text, "the budget is approved");

        // markdown + json mirrors exist
        assert!(dir
            .path()
            .join("transcripts")
            .join(format!("{}.md", meeting.id))
            .exists());
        assert!(dir
            .path()
            .join("transcripts")
            .join(format!("{}.json", meeting.id))
            .exists());

        // transcript content is searchable and resolves to the note
        let hits = s.search("budget", 10).unwrap();
        assert!(hits
            .iter()
            .any(|h| h.kind == crate::SearchHitKind::Transcript && h.note_id == note.id));
    }

    #[test]
    fn relabel_updates_label_and_markdown() {
        let (dir, s) = test_storage();
        let note = s.create_note("m", None).unwrap();
        let meeting = s.create_meeting("m", &note.id, &[]).unwrap();
        s.save_transcript(&sample_transcript(&meeting.id)).unwrap();

        let updated = s.relabel_speaker(&meeting.id, "spk_0", "Dana").unwrap();
        assert_eq!(updated.speakers[0].label, "Dana");
        let md = std::fs::read_to_string(
            dir.path()
                .join("transcripts")
                .join(format!("{}.md", meeting.id)),
        )
        .unwrap();
        assert!(md.contains("**Dana:**"));
    }
}
