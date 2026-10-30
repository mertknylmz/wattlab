use super::{
    error::ApplicationError,
    ports::{history_repository::HistoryRepository, settings_repository::SettingsRepository},
};
use crate::domain::{
    history::{ActivityEntry, SeriesPoint, SessionRecord, SessionSummary, summarize},
    settings::Ftp,
};
use chrono::{DateTime, Utc};
use std::sync::Arc;
use uuid::Uuid;

pub const MAX_PAGE_SIZE: u32 = 100;
/// A year-long heatmap plus a little slack; not an unbounded export.
pub const MAX_ACTIVITY_DAYS: i64 = 400;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneReference {
    /// FTP captured when the session started.
    Session,
    /// Sessions recorded before FTP capture use the current setting.
    Current,
}

#[derive(Debug, Clone)]
pub struct SummarizedSession {
    pub record: SessionRecord,
    pub summary: SessionSummary,
    pub zone_reference: ZoneReference,
}

#[derive(Debug, Clone)]
pub struct SessionPage {
    pub sessions: Vec<SummarizedSession>,
    /// Cursor for the next page; absent when this page was not full.
    pub next_before: Option<Uuid>,
}

#[derive(Clone)]
pub struct HistoryService {
    history: Arc<dyn HistoryRepository>,
    settings: Arc<dyn SettingsRepository>,
}

impl HistoryService {
    pub fn new(history: Arc<dyn HistoryRepository>, settings: Arc<dyn SettingsRepository>) -> Self {
        Self { history, settings }
    }

    /// `limit` must already be within 1..=MAX_PAGE_SIZE.
    pub async fn page(
        &self,
        member: Uuid,
        before: Option<Uuid>,
        limit: u32,
    ) -> Result<SessionPage, ApplicationError> {
        let records = self.history.sessions_page(member, before, limit).await?;
        let mut series = self
            .history
            .series(records.iter().map(|r| r.session.id()).collect())
            .await?;
        let current = self.current_ftp(member, &records).await?;
        let next_before = (records.len() == limit as usize)
            .then(|| records.last().map(|r| r.session.id()))
            .flatten();
        let sessions = records
            .into_iter()
            .map(|record| {
                let points = series.remove(&record.session.id()).unwrap_or_default();
                summarized(record, &points, current)
            })
            .collect();
        Ok(SessionPage {
            sessions,
            next_before,
        })
    }

    pub async fn activity(
        &self,
        member: Uuid,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<ActivityEntry>, ApplicationError> {
        Ok(self.history.activity(member, from, to).await?)
    }

    pub async fn detail(
        &self,
        member: Uuid,
        id: Uuid,
    ) -> Result<(SummarizedSession, Vec<SeriesPoint>), ApplicationError> {
        let record = self
            .history
            .session_record(member, id)
            .await?
            .ok_or(ApplicationError::SessionNotFound)?;
        let points = self
            .history
            .series(vec![id])
            .await?
            .remove(&id)
            .unwrap_or_default();
        let current = self
            .current_ftp(member, std::slice::from_ref(&record))
            .await?;
        Ok((summarized(record, &points, current), points))
    }

    /// Reads settings only when some session lacks a captured FTP.
    async fn current_ftp(
        &self,
        member: Uuid,
        records: &[SessionRecord],
    ) -> Result<Option<Ftp>, ApplicationError> {
        if records.iter().all(|r| r.ftp_w.is_some()) {
            return Ok(None);
        }
        Ok(Some(self.settings.settings(member).await?.ftp))
    }
}

fn summarized(
    record: SessionRecord,
    points: &[SeriesPoint],
    current: Option<Ftp>,
) -> SummarizedSession {
    let captured = record.ftp_w.and_then(|w| Ftp::new(w.into()).ok());
    let (ftp, zone_reference) = match captured {
        Some(ftp) => (Some(ftp), ZoneReference::Session),
        None => (current, ZoneReference::Current),
    };
    SummarizedSession {
        summary: summarize(&record.session, points, ftp),
        record,
        zone_reference,
    }
}
