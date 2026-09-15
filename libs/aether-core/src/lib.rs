mod application;
pub(crate) mod domain;
pub(crate) mod infrastructure;

pub use application::*;
pub use domain::*;

// The one piece of infrastructure configuration the binary has to name:
// creating a client for a data plane is an act on the identity provider, and
// the credentials for it come from the same place every other one does.
pub use infrastructure::herald_identity::RealmAdmin;
