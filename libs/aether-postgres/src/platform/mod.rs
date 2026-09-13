#[cfg_attr(coverage_nightly, coverage(off))]
pub mod estate_repository;

pub use estate_repository::PostgresEstateRepository;
