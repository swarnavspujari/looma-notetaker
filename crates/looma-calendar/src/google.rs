//! Google Calendar provider (installed-app OAuth, calendar.readonly scope).

use std::sync::Arc;

use chrono::{DateTime, Utc};
use looma_secrets::SecretStore;

use crate::oauth::{self, OAuthConfig, TokenSet};
use crate::{CalendarError, CalendarEvent, CalendarProvider, Result};

pub type OpenUrl = Arc<dyn Fn(String) + Send + Sync>;

pub struct GoogleCalendarProvider {
    pub client_id: String,
    pub client_secret: String,
    pub secrets: Arc<dyn SecretStore>,
    pub open_url: OpenUrl,
}

impl GoogleCalendarProvider {
    fn oauth_config(&self) -> OAuthConfig {
        OAuthConfig {
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            client_id: self.client_id.clone(),
            client_secret: Some(self.client_secret.clone()),
            scopes: "https://www.googleapis.com/auth/calendar.readonly".into(),
            // offline + consent → Google actually returns a refresh token
            extra_auth_params: vec![("access_type", "offline"), ("prompt", "consent")],
        }
    }

    async fn access_token(&self) -> Result<String> {
        let stored = self
            .secrets
            .get(looma_secrets::keys::GOOGLE_OAUTH_TOKEN)
            .map_err(|e| CalendarError::Auth(e.to_string()))?
            .ok_or(CalendarError::NotConnected)?;
        let tokens: TokenSet = serde_json::from_str(&stored)
            .map_err(|e| CalendarError::Auth(format!("stored token unreadable: {e}")))?;
        if !tokens.is_expired() {
            return Ok(tokens.access_token);
        }
        let refresh_token = tokens
            .refresh_token
            .as_deref()
            .ok_or(CalendarError::NotConnected)?;
        let renewed = oauth::refresh(&self.oauth_config(), refresh_token).await?;
        self.secrets
            .set(
                looma_secrets::keys::GOOGLE_OAUTH_TOKEN,
                &serde_json::to_string(&renewed).unwrap_or_default(),
            )
            .map_err(|e| CalendarError::Auth(e.to_string()))?;
        Ok(renewed.access_token)
    }
}

#[async_trait::async_trait]
impl CalendarProvider for GoogleCalendarProvider {
    fn id(&self) -> &'static str {
        "google"
    }

    fn display_name(&self) -> &'static str {
        "Google Calendar"
    }

    async fn is_connected(&self) -> bool {
        self.secrets
            .get(looma_secrets::keys::GOOGLE_OAUTH_TOKEN)
            .ok()
            .flatten()
            .is_some()
    }

    async fn connect(&self) -> Result<()> {
        let tokens = oauth::interactive_auth(&self.oauth_config(), self.open_url.as_ref()).await?;
        self.secrets
            .set(
                looma_secrets::keys::GOOGLE_OAUTH_TOKEN,
                &serde_json::to_string(&tokens).unwrap_or_default(),
            )
            .map_err(|e| CalendarError::Auth(e.to_string()))
    }

    async fn disconnect(&self) -> Result<()> {
        self.secrets
            .delete(looma_secrets::keys::GOOGLE_OAUTH_TOKEN)
            .map_err(|e| CalendarError::Auth(e.to_string()))
    }

    async fn upcoming(&self, from: DateTime<Utc>, to: DateTime<Utc>) -> Result<Vec<CalendarEvent>> {
        let token = self.access_token().await?;
        let url = format!(
            "https://www.googleapis.com/calendar/v3/calendars/primary/events?timeMin={}&timeMax={}&singleEvents=true&orderBy=startTime&maxResults=25",
            urlencoding::encode(&from.to_rfc3339()),
            urlencoding::encode(&to.to_rfc3339()),
        );
        let client = reqwest::Client::new();
        let resp = client
            .get(url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| CalendarError::Network(e.to_string()))?;
        let status = resp.status();
        let text = resp
            .text()
            .await
            .map_err(|e| CalendarError::Network(e.to_string()))?;
        if status.as_u16() == 401 {
            return Err(CalendarError::NotConnected);
        }
        if !status.is_success() {
            return Err(CalendarError::Provider(format!(
                "{status}: {}",
                text.chars().take(300).collect::<String>()
            )));
        }
        parse_google_events(&text)
    }
}

pub fn parse_google_events(json: &str) -> Result<Vec<CalendarEvent>> {
    let v: serde_json::Value = serde_json::from_str(json)
        .map_err(|e| CalendarError::Provider(format!("bad events JSON: {e}")))?;
    let mut events = Vec::new();
    for item in v.get("items").and_then(|i| i.as_array()).unwrap_or(&vec![]) {
        let parse_time = |field: &str| -> Option<DateTime<Utc>> {
            let t = item.get(field)?;
            if let Some(dt) = t.get("dateTime").and_then(|d| d.as_str()) {
                DateTime::parse_from_rfc3339(dt)
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            } else {
                // all-day events: date only, midnight UTC
                let d = t.get("date")?.as_str()?;
                DateTime::parse_from_rfc3339(&format!("{d}T00:00:00Z"))
                    .ok()
                    .map(|d| d.with_timezone(&Utc))
            }
        };
        let (Some(start), Some(end)) = (parse_time("start"), parse_time("end")) else {
            continue;
        };
        let attendees = item
            .get("attendees")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a.get("email").and_then(|e| e.as_str()))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let join_url = item
            .get("hangoutLink")
            .and_then(|h| h.as_str())
            .map(str::to_string)
            .or_else(|| {
                item.pointer("/conferenceData/entryPoints")?
                    .as_array()?
                    .iter()
                    .find(|e| e.get("entryPointType").and_then(|t| t.as_str()) == Some("video"))?
                    .get("uri")?
                    .as_str()
                    .map(str::to_string)
            });
        events.push(CalendarEvent {
            id: item
                .get("id")
                .and_then(|i| i.as_str())
                .unwrap_or_default()
                .to_string(),
            provider: "google".into(),
            title: item
                .get("summary")
                .and_then(|s| s.as_str())
                .unwrap_or("(untitled)")
                .to_string(),
            start,
            end,
            attendees,
            join_url,
        });
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_events_with_attendees_and_meet_link() {
        let json = r#"{"items": [{
            "id": "evt1",
            "summary": "Budget sync",
            "start": {"dateTime": "2026-07-01T15:00:00Z"},
            "end": {"dateTime": "2026-07-01T15:30:00Z"},
            "attendees": [{"email": "a@x.com"}, {"email": "b@x.com"}],
            "hangoutLink": "https://meet.google.com/abc"
        }, {
            "id": "evt2",
            "summary": "Vacation",
            "start": {"date": "2026-07-02"},
            "end": {"date": "2026-07-03"}
        }]}"#;
        let events = parse_google_events(json).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].title, "Budget sync");
        assert_eq!(events[0].attendees.len(), 2);
        assert_eq!(
            events[0].join_url.as_deref(),
            Some("https://meet.google.com/abc")
        );
        assert_eq!(events[1].join_url, None);
    }
}
