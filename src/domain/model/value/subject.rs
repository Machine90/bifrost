use anyhow::{Context, anyhow};
use enum_as_inner::EnumAsInner;
use enum_kinds::EnumKind;

use crate::domain::model::value::tokens::Tokens;

#[derive(Debug, Clone, EnumKind)]
#[enum_kind(SubjectKind, derive(EnumAsInner))]
pub enum Subject {
    FromCookie {
        value: String,
        issued_at: Option<u64>,
    },
    FromAuthToken {
        value: String,
        issued_at: Option<u64>,
    },
}

impl Subject {
    pub fn get_subject_value(&self) -> &str {
        match self {
            Subject::FromCookie { value, .. } => value,
            Subject::FromAuthToken { value, .. } => value,
        }
    }

    pub fn get_issue_at(&self) -> Option<u64> {
        match self {
            Subject::FromCookie { issued_at, .. } => *issued_at,
            Subject::FromAuthToken { issued_at, .. } => *issued_at,
        }
    }

    pub fn get_suggest_cache_key(&self) -> String {
        let subject = self.get_subject_value();
        let iat = self.get_issue_at().unwrap_or(0);
        format!("{subject}:{iat}")
    }
}

impl TryFrom<&Tokens> for Subject {
    type Error = anyhow::Error;

    fn try_from(tokens: &Tokens) -> Result<Self, Self::Error> {
        let subject = match tokens {
            Tokens::PlaintextFromAuthHeader { subject } => Subject::FromAuthToken {
                value: subject.to_string(),
                issued_at: None,
            },
            Tokens::JwtTokensFromCookies {
                access_token,
                refresh_token,
            } => {
                let subject = match (access_token, refresh_token) {
                    (Some(access_token), _) => {
                        let token_expired =
                            access_token.is_expired().context("Invalid exp time")?;
                        let sub_missing = access_token.token_data.claims.sub.is_none();
                        if token_expired || sub_missing {
                            return Err(anyhow!("Invalid token"));
                        }
                        let issued_at = access_token.token_data.claims.iat;
                        let subject = access_token
                            .token_data
                            .claims
                            .sub
                            .clone()
                            .context("Failed to find subject from jwt token")?;
                        Subject::FromCookie {
                            value: subject,
                            issued_at,
                        }
                    }
                    (None, Some(refresh_token)) => {
                        let token_expired =
                            refresh_token.is_expired().context("Invalid exp time")?;
                        let sub_missing = refresh_token.token_data.claims.sub.is_none();
                        if token_expired || sub_missing {
                            return Err(anyhow!("Invalid token"));
                        }
                        let issued_at = refresh_token.token_data.claims.iat;
                        let subject = refresh_token
                            .token_data
                            .claims
                            .sub
                            .clone()
                            .context("Failed to find subject from jwt token")?;
                        Subject::FromCookie {
                            value: subject,
                            issued_at,
                        }
                    }
                    _ => {
                        return Err(anyhow!("Token is missing"));
                    }
                };
                subject
            }
            Tokens::JwtTokenFromAuthHeader { auth_token } => {
                let payload = &auth_token.token_data.claims;
                let subject = payload
                    .sub
                    .clone()
                    .context("Failed to find subject from jwt token")?;
                let issued_at = payload.iat;
                Subject::FromAuthToken {
                    value: subject,
                    issued_at,
                }
            }
        };
        Ok(subject)
    }
}
