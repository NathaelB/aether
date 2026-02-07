pub mod actions;
pub mod dataplanes;
pub mod deployments;
pub mod organisations;
pub mod roles;
pub mod users;

pub fn default_limit() -> usize {
    10
}

#[cfg(test)]
mod tests {
    use super::default_limit;

    #[test]
    fn default_limit_is_ten() {
        assert_eq!(default_limit(), 10);
    }
}
