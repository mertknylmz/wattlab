use super::{
    auth::MemberId,
    dto::{AgentResponse, LiveResponse},
    shutdown::Shutdown,
};
use crate::application::{command_service::CommandService, session_service::SessionService};
use axum::{
    extract::State,
    http::header::CACHE_CONTROL,
    response::{
        IntoResponse,
        sse::{Event, KeepAlive, Sse},
    },
};
use chrono::Utc;
use futures_util::stream::{self, Stream};
use serde::Serialize;
use std::{convert::Infallible, time::Duration};

const PERIOD: Duration = Duration::from_secs(1);

#[derive(Serialize)]
struct StreamSnapshot {
    live: LiveResponse,
    agent: AgentResponse,
}

/// Server-sent live view, one snapshot per second. Each event is a fresh read;
/// the stream never repeats an old snapshot to fill a gap.
pub(super) async fn live(
    MemberId(member): MemberId,
    State(sessions): State<SessionService>,
    State(commands): State<CommandService>,
    State(shutdown): State<Shutdown>,
) -> impl IntoResponse {
    let events = snapshots(member, sessions, commands, shutdown);
    (
        [(CACHE_CONTROL, "no-store")],
        Sse::new(events).keep_alive(KeepAlive::default()),
    )
}

fn snapshots(
    member: uuid::Uuid,
    sessions: SessionService,
    commands: CommandService,
    shutdown: Shutdown,
) -> impl Stream<Item = Result<Event, Infallible>> {
    stream::unfold(true, move |first| {
        let (sessions, commands, shutdown) = (sessions.clone(), commands.clone(), shutdown.clone());
        async move {
            if !first {
                tokio::select! {
                    () = tokio::time::sleep(PERIOD) => {},
                    () = shutdown.wait() => return None,
                }
            }
            let event = match sessions.live(member).await {
                Ok(live) => Event::default()
                    .event("snapshot")
                    .json_data(StreamSnapshot {
                        live: live.into(),
                        agent: commands.agent(Utc::now()).into(),
                    }),
                Err(_) => Ok(Event::default()
                    .event("unavailable")
                    .data(r#"{"error":"storage_unavailable"}"#)),
            };
            // Serialization of these DTOs cannot fail; fall back to a comment.
            let event = event.unwrap_or_else(|_| Event::default().comment("serialization"));
            Some((Ok(event), false))
        }
    })
}
