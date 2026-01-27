#[derive(Debug, Clone)]
pub struct CreateUserCommand {
    pub name: String,
    pub email: String,
    pub sub: String,
}
