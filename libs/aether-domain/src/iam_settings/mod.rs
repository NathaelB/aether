use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::iam_settings::branding::Branding;

pub mod branding;
pub mod ports;
pub mod service;

pub const IAM_SETTINGS_ACTION_TYPE: &str = "deployment.iam_settings";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct IamSettings {
    pub branding: Option<Branding>,
}
