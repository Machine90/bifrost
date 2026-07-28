use std::collections::HashSet;

use crate::domain::model::value::{role::Role, subject::Subject, tokens::TokensKind};

#[derive(Debug, Clone)]
pub struct UserBaseInfo {
    pub tokens_kind: TokensKind,
    pub user_subject: Subject,
    pub roles: HashSet<Role>,
}
