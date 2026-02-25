use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // user_id
    pub exp: usize,  // expiration timestamp
    pub iat: usize,  // issued at
    pub display_name: Option<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct GatewayAuth {
    pub token: String,
    pub claims: Option<Claims>,
}

impl GatewayAuth {
    pub fn new(token: String) -> Self {
        Self {
            token,
            claims: None,
        }
    }

    /// Decode JWT token (for client-side validation, not used for auth)
    /// The gateway validates the token, this is just for display
    pub fn decode_token(&mut self, secret: &str) -> Result<(), String> {
        let validation = Validation::new(Algorithm::HS256);
        let key = DecodingKey::from_secret(secret.as_bytes());

        match decode::<Claims>(&self.token, &key, &validation) {
            Ok(data) => {
                self.claims = Some(data.claims);
                Ok(())
            }
            Err(e) => Err(format!("Token decode error: {}", e)),
        }
    }

    pub fn user_id(&self) -> Option<String> {
        self.claims.as_ref().map(|c| c.sub.clone())
    }

    pub fn display_name(&self) -> Option<String> {
        self.claims.as_ref().and_then(|c| c.display_name.clone())
    }

    pub fn has_permission(&self, permission: &str) -> bool {
        self.claims
            .as_ref()
            .map(|c| c.permissions.contains(&permission.to_string()))
            .unwrap_or(false)
    }
}
