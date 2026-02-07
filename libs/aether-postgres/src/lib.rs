#![cfg_attr(coverage_nightly, feature(coverage_attribute))]

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod action;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod deployments;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod dataplane;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod organisation;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod role;

#[cfg_attr(coverage_nightly, coverage(off))]
pub mod user;
