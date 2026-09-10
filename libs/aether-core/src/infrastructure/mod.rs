// Only `role` survives here. Every other module re-exported a repository the
// application layer used to name explicitly; `#[transactional]` resolves those
// through the registry now, so the re-exports were dead. `role` stays because
// the permission provider needs a second repository built by hand.
pub mod provisioner;
pub mod role;
