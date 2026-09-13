//! A channel to the broker that comes back after the broker goes away.
//!
//! lapin does not reconnect. A channel whose connection was closed -- an idle
//! connection the broker dropped overnight, a broker that restarted -- stays in
//! `Error` for the life of the process, and every use of it fails with
//! `invalid channel state: Error`.
//!
//! For a data plane that means going deaf without going down: work is claimed
//! from the control plane, fails to publish, and the only sign is a warning
//! every fifteen seconds. A deployment created in that window is left `failed`
//! with nothing in the cluster and nothing saying why.

use std::sync::Arc;

use lapin::{Channel, Connection};
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::{DEFAULT_BUDGET, connect_with_retry};

/// A channel, and whether it had to be made.
///
/// The distinction is the caller's business: a fresh channel carries none of
/// the topology the old one had, so exchanges and queues have to be declared
/// on it again. Returning that as a type rather than a boolean means a caller
/// cannot forget which one they are holding.
pub enum Live {
    /// The channel from last time, still usable.
    Existing(Channel),
    /// Newly made. Declare the topology on it before using it.
    Fresh(Channel),
}

impl Live {
    pub fn channel(&self) -> &Channel {
        match self {
            Self::Existing(channel) | Self::Fresh(channel) => channel,
        }
    }

    pub fn into_channel(self) -> Channel {
        match self {
            Self::Existing(channel) | Self::Fresh(channel) => channel,
        }
    }
}

/// Holds a connection to the broker and hands out a channel that works.
#[derive(Clone)]
pub struct Link {
    url: String,
    // Behind a mutex because two callers arriving at a dead channel must not
    // both rebuild it: the second would drop the first's connection, and the
    // channel the first is about to publish on dies with it.
    live: Arc<Mutex<Option<(Connection, Channel)>>>,
}

impl Link {
    pub fn to(url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            live: Arc::new(Mutex::new(None)),
        }
    }

    /// A channel that was usable a moment ago.
    ///
    /// Re-establishes when there is nothing, or when what there is has stopped
    /// working. Both halves are checked: a channel reports itself connected
    /// while the connection under it is gone, and the failure then only shows
    /// up at the next publish.
    pub async fn channel(&self) -> Result<Live, lapin::Error> {
        let mut live = self.live.lock().await;

        if let Some((connection, channel)) = live.as_ref()
            && connection.status().connected()
            && channel.status().connected()
        {
            return Ok(Live::Existing(channel.clone()));
        }

        if live.is_some() {
            warn!("the broker connection is gone: opening another");
        }

        // Dropped before connecting, not after: holding a dead connection
        // while building its replacement is how a process ends up with two.
        *live = None;

        let connection = connect_with_retry(&self.url, DEFAULT_BUDGET).await?;
        let channel = connection.create_channel().await?;

        info!("a channel to the broker is open");
        *live = Some((connection, channel.clone()));

        Ok(Live::Fresh(channel))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A link that has never connected holds nothing, so the first caller
    /// makes the channel rather than finding a dead one.
    #[tokio::test]
    async fn a_link_starts_with_nothing_open() {
        let link = Link::to("amqp://127.0.0.1:1/%2f");

        assert!(link.live.lock().await.is_none());
    }

    /// Bounded by the connect budget rather than hanging: a broker that is
    /// genuinely gone must surface, not look like a slow tick.
    #[tokio::test(start_paused = true)]
    async fn a_broker_that_never_answers_is_reported() {
        let link = Link::to("amqp://127.0.0.1:1/%2f");

        assert!(link.channel().await.is_err());
    }
}
