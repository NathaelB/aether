use std::collections::BTreeMap;
use std::fmt::{self, Display};

use aether_crds::v1alpha::identity_dataplane_upgrade::DataplaneComponent;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DataplaneComponentKind {
    Herald,
    Genesis,
    Operator,
}

pub type ComponentVersions = BTreeMap<DataplaneComponentKind, String>;

impl DataplaneComponentKind {
    pub const ALL: [Self; 3] = [Self::Herald, Self::Genesis, Self::Operator];

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Herald => "herald",
            Self::Genesis => "genesis",
            Self::Operator => "operator",
        }
    }

    pub fn expand(requested: &[DataplaneComponent]) -> Vec<Self> {
        Self::ALL
            .into_iter()
            .filter(|kind| {
                requested
                    .iter()
                    .any(|component| kind.is_requested(component))
            })
            .collect()
    }

    pub fn from_component(component: DataplaneComponent) -> Option<Self> {
        match component {
            DataplaneComponent::Herald => Some(Self::Herald),
            DataplaneComponent::Genesis => Some(Self::Genesis),
            DataplaneComponent::Operator => Some(Self::Operator),
            DataplaneComponent::All => None,
        }
    }

    fn is_requested(&self, component: &DataplaneComponent) -> bool {
        matches!(
            (self, component),
            (_, DataplaneComponent::All)
                | (Self::Herald, DataplaneComponent::Herald)
                | (Self::Genesis, DataplaneComponent::Genesis)
                | (Self::Operator, DataplaneComponent::Operator)
        )
    }
}

impl From<DataplaneComponentKind> for DataplaneComponent {
    fn from(kind: DataplaneComponentKind) -> Self {
        match kind {
            DataplaneComponentKind::Herald => Self::Herald,
            DataplaneComponentKind::Genesis => Self::Genesis,
            DataplaneComponentKind::Operator => Self::Operator,
        }
    }
}

impl Display for DataplaneComponentKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_expands_to_every_component_in_upgrade_order() {
        assert_eq!(
            DataplaneComponentKind::expand(&[DataplaneComponent::All]),
            DataplaneComponentKind::ALL.to_vec()
        );
    }

    #[test]
    fn named_components_expand_without_duplicates() {
        let expanded = DataplaneComponentKind::expand(&[
            DataplaneComponent::Operator,
            DataplaneComponent::Herald,
            DataplaneComponent::Herald,
        ]);

        assert_eq!(
            expanded,
            vec![
                DataplaneComponentKind::Herald,
                DataplaneComponentKind::Operator
            ]
        );
    }

    #[test]
    fn nothing_requested_expands_to_nothing() {
        assert!(DataplaneComponentKind::expand(&[]).is_empty());
    }
}
