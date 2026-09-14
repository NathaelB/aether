//! Naming the first operator, so an installation is not one nobody can run.
//!
//! Same shape as the wrapping key and the archive bucket beside it: called on
//! every start, does nothing when there is nothing to do, and says on the
//! first line of the logs when an installation has nobody able to operate it.
//!
//! The write itself lives in aether-core, which owns persistence. This reads
//! the configuration and reports what happened.

use std::sync::Arc;

use aether_core::FirstOperator;
use tracing::{error, info, warn};

use crate::{args::Args, state::AppState};

pub async fn ensure_first_operator(args: Arc<Args>, state: AppState) {
    let subject = args.platform.bootstrap_operator.trim();

    if subject.is_empty() {
        // Not an error. An installation past its first day has operators in
        // the database and no reason to keep naming one in its configuration.
        info!("no bootstrap operator is configured");
        return;
    }

    match state.service.ensure_first_operator(subject).await {
        Ok(FirstOperator::AlreadyGranted) => {
            info!(subject, "the bootstrap operator already holds rights")
        }
        Ok(FirstOperator::Granted) => {
            info!(
                subject,
                "granted the bootstrap operator every platform right"
            )
        }
        Err(error) => error!(%error, subject, "could not grant the bootstrap operator"),
    }
}

/// Says so, loudly, when nothing here can be operated.
///
/// A different question from the one above: that asks whether the configured
/// subject is in place, this asks whether anybody at all is. An installation
/// with neither answers 403 to every platform screen, and the reason is worth
/// finding in the logs rather than in the database.
pub async fn warn_if_nobody_operates(state: AppState) {
    match state.service.count_operators().await {
        Ok(0) => warn!(
            "nobody holds a platform right: every platform screen will refuse. \
             Set AETHER_BOOTSTRAP_OPERATOR to a subject and restart."
        ),
        Ok(count) => info!(count, "operators are in place"),
        Err(error) => error!(%error, "could not read who operates this installation"),
    }
}
