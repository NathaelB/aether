use std::sync::Arc;

use crate::domain::{
    models::{claims::Claims, errors::AuthError, identity::Identity},
    ports::AuthRepository,
};
use chrono::Utc;
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Jwk {
    kid: String,
    n: String,
    e: String,
}

#[derive(Clone)]
pub struct KeycloakAuthRepository {
    pub http: Arc<Client>,
    pub issuer: String,
    pub audience: Option<String>,
}

impl KeycloakAuthRepository {
    pub fn new(issuer: impl Into<String>, audience: Option<String>) -> Self {
        Self {
            http: Arc::new(Client::new()),
            issuer: issuer.into(),
            audience,
        }
    }

    async fn fetch_jwks(&self) -> Result<Jwks, AuthError> {
        let url = format!("{}/protocol/openid-connect/certs", self.issuer);

        let resp = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| AuthError::Network {
                message: e.to_string(),
            })?;

        if resp.status().is_client_error() || resp.status().is_server_error() {
            return Err(AuthError::Network {
                message: format!("failed to fetch jwks: {}", resp.status()),
            });
        }

        let bytes = resp.bytes().await.map_err(|e| AuthError::Network {
            message: e.to_string(),
        })?;

        let jwks: Jwks = serde_json::from_slice(&bytes).map_err(|e| AuthError::Network {
            message: e.to_string(),
        })?;

        Ok(jwks)
    }
}

impl AuthRepository for KeycloakAuthRepository {
    async fn validate_token(
        &self,
        token: &str,
    ) -> Result<crate::domain::models::claims::Claims, AuthError> {
        let header = decode_header(token).map_err(|e| AuthError::InvalidToken {
            message: e.to_string(),
        })?;

        let kid = header.kid.ok_or_else(|| AuthError::InvalidToken {
            message: "missing kind".into(),
        })?;

        let jwks = self.fetch_jwks().await?;

        let keys = jwks.keys;

        let key = keys
            .iter()
            .find(|k| k.kid == kid)
            .ok_or_else(|| AuthError::KeyNotFound { key: kid.clone() })?;

        let decoding_key =
            DecodingKey::from_rsa_components(&key.n, &key.e).map_err(|e| AuthError::Internal {
                message: e.to_string(),
            })?;

        let mut validation = Validation::new(Algorithm::RS256);

        validation.validate_aud = false;

        let data = decode::<Claims>(token, &decoding_key, &validation).map_err(|e| {
            AuthError::InvalidToken {
                message: e.to_string(),
            }
        })?;

        let claims = data.claims;

        let now = Utc::now().timestamp();

        if claims.exp.unwrap_or(0) < now {
            return Err(AuthError::Expired);
        }

        Ok(claims)
    }

    async fn identify(
        &self,
        token: &str,
    ) -> Result<crate::domain::models::identity::Identity, AuthError> {
        let claims = self.validate_token(token).await?;

        Ok(Identity::from(claims))
    }
}
