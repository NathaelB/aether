use std::str::FromStr;

use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, ToSchema)]
pub struct UserId(pub Uuid);

impl FromStr for UserId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::from_str(s).map(UserId)
    }
}
