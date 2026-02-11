pub mod identity_instance;
pub mod identity_instance_upgrade;

use futures::try_join;

use crate::domain::OperatorError;

pub async fn run() -> Result<(), OperatorError> {
    try_join!(identity_instance::run(), identity_instance_upgrade::run())?;
    Ok(())
}
