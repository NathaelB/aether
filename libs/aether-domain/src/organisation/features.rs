use std::fmt;
use std::future::Future;

use aether_auth::Identity;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::{
    CoreError,
    organisation::{OrganisationId, value_objects::Plan},
};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum IamFeature {
    SsoConnectors,
    Mfa,
    DirectoryFederation,
    CustomDomain,
    Branding,
    Analytics,
    Compliance,
    DelegatedAdmin,
}

const EVERY_TIER: &[Plan] = &[Plan::Free, Plan::Starter, Plan::Business, Plan::Enterprise];

impl IamFeature {
    pub const ALL: [Self; 8] = [
        Self::SsoConnectors,
        Self::Mfa,
        Self::DirectoryFederation,
        Self::CustomDomain,
        Self::Branding,
        Self::Analytics,
        Self::Compliance,
        Self::DelegatedAdmin,
    ];

    /// The tiers that open this feature.
    ///
    /// Restricting a feature is editing its arm. Written cheapest first, so
    /// the tier walk and a reader agree on which tier unlocks it.
    pub fn open_to(self) -> &'static [Plan] {
        match self {
            Self::SsoConnectors => EVERY_TIER,
            Self::Mfa => EVERY_TIER,
            Self::DirectoryFederation => EVERY_TIER,
            Self::CustomDomain => EVERY_TIER,
            Self::Branding => EVERY_TIER,
            Self::Analytics => EVERY_TIER,
            Self::Compliance => EVERY_TIER,
            Self::DelegatedAdmin => EVERY_TIER,
        }
    }
}

impl fmt::Display for IamFeature {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SsoConnectors => write!(f, "sso_connectors"),
            Self::Mfa => write!(f, "mfa"),
            Self::DirectoryFederation => write!(f, "directory_federation"),
            Self::CustomDomain => write!(f, "custom_domain"),
            Self::Branding => write!(f, "branding"),
            Self::Analytics => write!(f, "analytics"),
            Self::Compliance => write!(f, "compliance"),
            Self::DelegatedAdmin => write!(f, "delegated_admin"),
        }
    }
}

impl Plan {
    /// Cheapest first.
    pub const TIERS: [Self; 4] = [Self::Free, Self::Starter, Self::Business, Self::Enterprise];

    pub fn opens(&self, feature: IamFeature) -> bool {
        feature.open_to().contains(self)
    }
}

/// The cheapest tier that opens a feature, for a refusal that says what to do.
pub fn cheapest_plan_opening(feature: IamFeature) -> Option<Plan> {
    Plan::TIERS.into_iter().find(|plan| plan.opens(feature))
}

/// One feature, seen from an organisation on a given tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct IamFeatureAvailability {
    pub feature: IamFeature,

    pub open: bool,

    /// The cheapest tier that opens it, when this one does not. Null when open.
    pub opened_by: Option<Plan>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, ToSchema)]
pub struct PlanFeatures {
    pub plan: Plan,
    pub features: Vec<IamFeatureAvailability>,
}

/// The whole set, answered for one tier.
pub fn features_for(plan: Plan) -> PlanFeatures {
    let features = IamFeature::ALL
        .into_iter()
        .map(|feature| {
            let open = plan.opens(feature);

            IamFeatureAvailability {
                feature,
                open,
                opened_by: if open {
                    None
                } else {
                    cheapest_plan_opening(feature)
                },
            }
        })
        .collect();

    PlanFeatures { plan, features }
}

pub trait FeatureService: Send + Sync {
    /// Gated on being able to see instances, like the offers: the answer
    /// discloses which tier the organisation is on.
    fn list_features(
        &self,
        identity: Identity,
        organisation_id: OrganisationId,
    ) -> impl Future<Output = Result<PlanFeatures, CoreError>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_feature_is_listed() {
        for feature in IamFeature::ALL {
            match feature {
                IamFeature::SsoConnectors
                | IamFeature::Mfa
                | IamFeature::DirectoryFederation
                | IamFeature::CustomDomain
                | IamFeature::Branding
                | IamFeature::Analytics
                | IamFeature::Compliance
                | IamFeature::DelegatedAdmin => {}
            }
        }

        let mut sorted = IamFeature::ALL;
        sorted.sort();
        sorted
            .windows(2)
            .for_each(|pair| assert_ne!(pair[0], pair[1]));
        assert_eq!(IamFeature::ALL.len(), 8);
    }

    #[test]
    fn every_plan_opens_every_feature_today() {
        for plan in Plan::TIERS {
            for feature in IamFeature::ALL {
                assert!(plan.opens(feature), "{plan} does not open {feature}");
            }
        }
    }

    #[test]
    fn the_cheapest_plan_opening_a_feature_is_free_today() {
        for feature in IamFeature::ALL {
            assert_eq!(cheapest_plan_opening(feature), Some(Plan::Free));
        }
    }

    #[test]
    fn an_open_feature_names_no_tier() {
        for plan in Plan::TIERS {
            let answer = features_for(plan);
            assert_eq!(answer.plan, plan);
            assert_eq!(answer.features.len(), IamFeature::ALL.len());
            for entry in answer.features {
                assert!(entry.open);
                assert_eq!(entry.opened_by, None);
            }
        }
    }

    #[test]
    fn the_wire_names_are_snake_case() {
        let names: Vec<String> = IamFeature::ALL
            .into_iter()
            .map(|feature| {
                serde_json::to_value(feature)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();
        let displayed: Vec<String> = IamFeature::ALL.into_iter().map(|f| f.to_string()).collect();
        assert_eq!(names, displayed);
    }
}
