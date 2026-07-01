//! Meetings: the bridge between a note, its recording, and (from M3) its
//! transcript. Recording paths are stored relative to the data dir.

use chrono::Utc;
use looma_core::{Meeting, RecordingRef};
use rusqlite::OptionalExtension;

use crate::folders::parse_ts;
use crate::{Result, Storage, StorageError};

impl Storage {
    /// Create a meeting attached to a note and point the note back at it.
    pub fn create_meeting(
        &self,
        title: &str,
        note_id: &str,
        attendees: &[String],
    ) -> Result<Meeting> {
        let meeting = Meeting {
            id: looma_core::new_id(),
            title: title.to_string(),
            note_id: note_id.to_string(),
            attendees: attendees.to_vec(),
            started_at: Utc::now(),
            ended_at: None,
            recording: None,
        };
        self.conn.execute(
            "INSERT INTO meetings (id, title, note_id, attendees_json, started_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                &meeting.id,
                &meeting.title,
                &meeting.note_id,
                serde_json::to_string(&meeting.attendees)?,
                meeting.started_at.to_rfc3339(),
            ),
        )?;
        self.conn.execute(
            "UPDATE notes SET meeting_id = ?1 WHERE id = ?2",
            (&meeting.id, note_id),
        )?;
        Ok(meeting)
    }

    /// Mark a meeting finished and store its recording.
    pub fn end_meeting(&self, id: &str, recording: &RecordingRef) -> Result<Meeting> {
        let n = self.conn.execute(
            "UPDATE meetings SET ended_at = ?1, recording_json = ?2 WHERE id = ?3",
            (
                Utc::now().to_rfc3339(),
                serde_json::to_string(recording)?,
                id,
            ),
        )?;
        if n == 0 {
            return Err(StorageError::NotFound(format!("meeting {id}")));
        }
        self.get_meeting(id)
    }

    pub fn get_meeting(&self, id: &str) -> Result<Meeting> {
        self.conn
            .query_row(
                "SELECT id, title, note_id, attendees_json, started_at, ended_at, recording_json
                 FROM meetings WHERE id = ?1",
                [id],
                row_to_meeting,
            )
            .optional()?
            .ok_or_else(|| StorageError::NotFound(format!("meeting {id}")))
    }

    pub fn get_meeting_for_note(&self, note_id: &str) -> Result<Option<Meeting>> {
        Ok(self
            .conn
            .query_row(
                "SELECT id, title, note_id, attendees_json, started_at, ended_at, recording_json
                 FROM meetings WHERE note_id = ?1 ORDER BY started_at DESC LIMIT 1",
                [note_id],
                row_to_meeting,
            )
            .optional()?)
    }
}

fn row_to_meeting(r: &rusqlite::Row<'_>) -> rusqlite::Result<Meeting> {
    let attendees_json: String = r.get(3)?;
    let recording_json: Option<String> = r.get(6)?;
    Ok(Meeting {
        id: r.get(0)?,
        title: r.get(1)?,
        note_id: r.get(2)?,
        attendees: serde_json::from_str(&attendees_json).unwrap_or_default(),
        started_at: parse_ts(r.get::<_, String>(4)?),
        ended_at: r.get::<_, Option<String>>(5)?.map(parse_ts),
        recording: recording_json.and_then(|j| serde_json::from_str(&j).ok()),
    })
}

#[cfg(test)]
mod tests {
    use crate::test_storage;
    use looma_core::RecordingRef;

    #[test]
    fn meeting_lifecycle() {
        let (_dir, s) = test_storage();
        let note = s.create_note("Weekly sync", None).unwrap();
        let meeting = s
            .create_meeting("Weekly sync", &note.id, &["dana@example.com".into()])
            .unwrap();

        // note points back at the meeting
        assert_eq!(
            s.get_note(&note.id).unwrap().meeting_id,
            Some(meeting.id.clone())
        );
        assert!(s.get_meeting(&meeting.id).unwrap().ended_at.is_none());

        let rec = RecordingRef {
            mic_path: Some("recordings/x/recording.mic.wav".into()),
            system_path: Some("recordings/x/recording.system.wav".into()),
            mixed_path: Some("recordings/x/recording.mixed.wav".into()),
            duration_ms: 61_000,
        };
        let ended = s.end_meeting(&meeting.id, &rec).unwrap();
        assert!(ended.ended_at.is_some());
        assert_eq!(ended.recording.unwrap().duration_ms, 61_000);

        let by_note = s.get_meeting_for_note(&note.id).unwrap().unwrap();
        assert_eq!(by_note.id, meeting.id);
    }
}
