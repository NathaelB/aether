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
        validation.validate_exp = false;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::models::claims::{Audience, Claims, Subject};
    use chrono::{Duration, Utc};
    use httpmock::Method::GET;
    use httpmock::MockServer;
    use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};

    const PRIVATE_KEY_PEM: &str = "-----BEGIN PRIVATE KEY-----\nMIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQCrmNlnOOd0ptY2\n\
iKCRk/9JOkTiuVJZ59oquSDoLuCRCWObmhw214t5N8NZoRzBPIRBPu4HXG0IG9o/\n\
LW4CEyymdB0qLJEiNFmuqZgiR0en21tRhoGguPDvUv+/F2LKNTK9BYDqd1u0UqMn\n\
A5zI7PSeVYwbhommyN9HiSjFGJAaN0HMjGs+UMXmdpzBKzK5OjLK4J8PgFGIrfbr\n\
S226NQKmNuvwYBz9Dz3W3XcFJm+roG6W4wyM+Ak7hGaSRNAuEmt0iSKKpN+CDxvn\n\
sj06tqH7MaMz2yeJAO8W/BTHEBrKDO1zwZefE5EvGmJi/dr1RAzbtALY9Vg8zb57\n\
PefVe7UjAgMBAAECggEAEq8x/OtVSH5iFM4Lrc5PncmadpV3QhLczooZ6y4vhZtg\n\
HTFKoS4XIbvQqZHBb8kHHZOcl3CY7qkZuodX0yIDWpyKEG2J4A+TNFGDHzhjtQNN\n\
jjL3Kmj40xZWgpgpSZtBSxOuVdlpQtk8qqLaD4a9m//0oYqksmRs630j01I5XqE3\n\
1Kdv148LqOjKsxCuibmdd4Pra1jE831YCc1kAXsE76wLCMb5BBWX2KLG/cdcMg79\n\
+MFDOqRc0uRM1dP/3U0wsxlS+nw9XHByi9Dx2lSr2UE9dxc/Cx24fbSqPU5ZDJbR\n\
Xycg0EC6tAU7cKkuEeUofp0i4o6HfPDn/Rkec1qAAQKBgQDlXX+POOXsviCRyXx3\n\
2a7o3vKbAmlehfbpnO5U10mKDRewIalwvtDe8GPKFJndFafc0EZmnoWSpHSn+PwS\n\
1CyVtFnuXgr5cVQ1pd5E3dwCT+Y9wZWDf9Z2YIGiCDcr+22GN1Ciz6T4fDhZZXak\n\
F5mKbcYFNmUSNiW2zAr1pHAyAQKBgQC/hglBFGGMiAhssdJCiScoA/zVlYybUv3Q\n\
NOnNu+xjjQh2wXCU4wdp3qHeSG3LErMdjT2CfawrIxMywW7+Kly5CnGRmR0Pp8Sx\n\
lYFmKhvWRc1zf04H01OrY1oFCuFQkTXcsKgPhefjuKALph2lP7PlBULjPhB8t7fs\n\
x0RTUpbfIwKBgQDcFI7lOk9EjlpqCM8poPI3+FUJb5LzY8+78Ryuw7SIhm+ITYRr\n\
7mw0vqzBpmrMvI7JTf9/T/QS9UIKOtqEppnxO5BfLFWTa67Fm1Ze9hK5FTlzYGC6\n\
QNvj0k4Qz5lA1owNEN6KmntNAsR+4uCoKwzkytgLAFqS0un1MGwDr7kIAQKBgB/F\n\
pJNfRi+CAaPGfBL9nblNsAvem0zJH8IChSbUHgsFwnmw7XRFlV1CyaeObGhb2cr1\n\
O1cCciVV1EF/RWJ0tJ0d1mlI9UE7m626F5VTNvr86XBXliJGNIMiIDTl8SrkbAMI\n\
a1jn5egpIKPOEuzu/HDpxobcLPADqkdlZzhLYyvxAoGBAJSRmIllkxc7PGrzAUEk\n\
jLhd+VSdTIWuNN2hl6wdXjwpj9D3gKByyFTfXTs2QrskyPTql3Ikl4SegwmXslkF\n\
XYUx5rebTnBZJ5rImd9l1o6w4W76HNzZ+jgErRn10JFahVCb0O3MBPgFzfkkTo+W\n\
CuS3pkf78EONr41Q+iqYZW+5\n-----END PRIVATE KEY-----\n";

    const JWK_N: &str = "q5jZZzjndKbWNoigkZP_STpE4rlSWefaKrkg6C7gkQljm5ocNteLeTfDWaEcwTyEQT7uB1xtCBvaPy1uAhMspnQdKiyRIjRZrqmYIkdHp9tbUYaBoLjw71L_vxdiyjUyvQWA6ndbtFKjJwOcyOz0nlWMG4aJpsjfR4koxRiQGjdBzIxrPlDF5nacwSsyuToyyuCfD4BRiK3260ttujUCpjbr8GAc_Q891t13BSZvq6BuluMMjPgJO4RmkkTQLhJrdIkiiqTfgg8b57I9Orah-zGjM9sniQDvFvwUxxAaygztc8GXnxORLxpiYv3a9UQM27QC2PVYPM2-ez3n1Xu1Iw";
    const JWK_E: &str = "AQAB";

    fn jwks_response(kid: &str) -> serde_json::Value {
        serde_json::json!({
            "keys": [{
                "kid": kid,
                "n": JWK_N,
                "e": JWK_E
            }]
        })
    }

    fn base_claims(exp: Option<i64>) -> Claims {
        Claims {
            sub: Subject("user-123".to_string()),
            iss: "http://issuer.test".to_string(),
            aud: Some(Audience::Single("aud".to_string())),
            exp,
            email: Some("john.doe@example.com".to_string()),
            email_verified: Some(true),
            name: Some("John Doe".to_string()),
            preferred_username: Some("johndoe".to_string()),
            given_name: Some("John".to_string()),
            family_name: Some("Doe".to_string()),
            scope: "openid profile email".to_string(),
            client_id: None,
            extra: serde_json::Map::new(),
        }
    }

    fn sign_token(kid: Option<String>, claims: &Claims) -> String {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = kid;
        let key = EncodingKey::from_rsa_pem(PRIVATE_KEY_PEM.as_bytes()).expect("valid pem");
        encode(&header, claims, &key).expect("token")
    }

    #[tokio::test]
    async fn validate_token_success() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/protocol/openid-connect/certs");
            then.status(200).json_body(jwks_response("test-key"));
        });

        let repo = KeycloakAuthRepository::new(server.base_url(), None);
        let claims = base_claims(Some((Utc::now() + Duration::minutes(5)).timestamp()));
        let token = sign_token(Some("test-key".to_string()), &claims);

        let result = repo.validate_token(&token).await.unwrap();
        assert_eq!(result.sub.0, "user-123");
        assert_eq!(result.email, Some("john.doe@example.com".to_string()));
    }

    #[tokio::test]
    async fn validate_token_expired() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/protocol/openid-connect/certs");
            then.status(200).json_body(jwks_response("test-key"));
        });

        let repo = KeycloakAuthRepository::new(server.base_url(), None);
        let claims = base_claims(Some((Utc::now() - Duration::minutes(5)).timestamp()));
        let token = sign_token(Some("test-key".to_string()), &claims);

        let result = repo.validate_token(&token).await;
        assert!(matches!(result, Err(AuthError::Expired)));
    }

    #[tokio::test]
    async fn validate_token_missing_kid() {
        let repo = KeycloakAuthRepository::new("http://issuer.test".to_string(), None);
        let claims = base_claims(Some((Utc::now() + Duration::minutes(5)).timestamp()));
        let token = sign_token(None, &claims);

        let result = repo.validate_token(&token).await;
        assert!(matches!(result, Err(AuthError::InvalidToken { .. })));
    }

    #[tokio::test]
    async fn validate_token_key_not_found() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/protocol/openid-connect/certs");
            then.status(200).json_body(jwks_response("other-key"));
        });

        let repo = KeycloakAuthRepository::new(server.base_url(), None);
        let claims = base_claims(Some((Utc::now() + Duration::minutes(5)).timestamp()));
        let token = sign_token(Some("test-key".to_string()), &claims);

        let result = repo.validate_token(&token).await;
        assert!(matches!(result, Err(AuthError::KeyNotFound { .. })));
    }

    #[tokio::test]
    async fn validate_token_jwks_http_error() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/protocol/openid-connect/certs");
            then.status(500);
        });

        let repo = KeycloakAuthRepository::new(server.base_url(), None);
        let claims = base_claims(Some((Utc::now() + Duration::minutes(5)).timestamp()));
        let token = sign_token(Some("test-key".to_string()), &claims);

        let result = repo.validate_token(&token).await;
        assert!(matches!(result, Err(AuthError::Network { .. })));
    }

    #[tokio::test]
    async fn identify_returns_identity() {
        let server = MockServer::start();
        let _mock = server.mock(|when, then| {
            when.method(GET).path("/protocol/openid-connect/certs");
            then.status(200).json_body(jwks_response("test-key"));
        });

        let repo = KeycloakAuthRepository::new(server.base_url(), None);
        let claims = base_claims(Some((Utc::now() + Duration::minutes(5)).timestamp()));
        let token = sign_token(Some("test-key".to_string()), &claims);

        let identity = repo.identify(&token).await.unwrap();
        assert!(identity.is_user());
        assert_eq!(identity.id(), "user-123");
    }
}
