use std::fmt::Display;

use clap::{Parser, ValueEnum};

#[derive(Debug, Clone, ValueEnum, Default)]
pub enum Environment {
    #[default]
    Development,
    Production,
}

impl From<String> for Environment {
    fn from(value: String) -> Self {
        match value.as_str() {
            "development" => Environment::Development,
            "production" => Environment::Production,
            _ => Environment::Development,
        }
    }
}

impl Display for Environment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Environment::Development => write!(f, "development"),
            Environment::Production => write!(f, "production"),
        }
    }
}

#[derive(Debug, Clone, Parser)]
#[command(about, version)]
pub struct Args {
    #[command(flatten)]
    pub(crate) amqp: AmqpArgs,
}

#[derive(clap::Args, Debug, Clone)]
pub struct AmqpArgs {
    #[arg(
        long = "amqp-url",
        env = "AMQP_URL",
        default_value = "amqp://aether:aether@localhost:5672",
        name = "AMQP URL",
        help = "The AMQP URL for connecting to RabbitMQ (e.g. amqp://user:pass@host:port)"
    )]
    pub amqp_url: String,

    #[arg(
        long = "amqp-queue",
        env = "AMQP_QUEUE",
        default_value = "genesis.actions",
        name = "AMQP Queue",
        help = "The name of the AMQP queue to consume from"
    )]
    pub amqp_queue: String,
}

impl Default for AmqpArgs {
    fn default() -> Self {
        Self {
            amqp_url: "amqp://aether:aether@localhost:5672".into(),
            amqp_queue: "genesis.actions".into(),
        }
    }
}
