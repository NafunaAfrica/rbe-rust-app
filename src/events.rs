//! Server-sent events for live shop-cache invalidation. This replaces the
//! reference app's Supabase Realtime subscription: when a webhook bumps the
//! version, every connected browser reloads the shop grid.

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use std::convert::Infallible;
use tokio_stream::Stream;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::state::AppState;

pub async fn events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.events().subscribe();
    let stream = BroadcastStream::new(rx).map(|msg| {
        let data = match msg {
            Ok(ev) => format!("{{\"version\":{},\"source\":\"{}\"}}", ev.version, ev.source),
            Err(_) => "{}".to_string(),
        };
        Ok::<Event, Infallible>(Event::default().event("cache").data(data))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}
