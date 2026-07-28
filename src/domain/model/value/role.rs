use enum_as_inner::EnumAsInner;
use enum_kinds::EnumKind;
use unic_langid::LanguageIdentifier;

const ROLE_ANONYMOUS_RAW: &str = "anonymous";
const ROLE_UNTAGGED_RAW: &str = "untagged";
const ROLE_GATEWAY_ADMIN_RAW: &str = "gateway_admin";

const ROLE_ANONYMOUS_ZH: &str = "访客";
const ROLE_UNTAGGED_ZH: &str = "登录用户";
const ROLE_GATEWAY_ADMIN_ZH: &str = "系统管理员";

const ROLE_ANONYMOUS_EN: &str = "Guest";
const ROLE_UNTAGGED_EN: &str = "Logged-in";
const ROLE_GATEWAY_ADMIN_EN: &str = "System-Admin";

#[derive(Debug, Clone, PartialEq, Eq, Hash, EnumKind)]
#[enum_kind(RoleKind, derive(EnumAsInner))]
pub enum Role {
    Anonymous,
    /// Logged in user without any roles configuration default to this role
    Untagged,
    /// User in this role has permission to request gateway management APIs
    GatewayAdmin,
    Tagged(String),
}

impl Role {
    pub fn from_str(role: &String) -> Self {
        let role = role.to_lowercase();
        let role = match role.as_str() {
            ROLE_ANONYMOUS_RAW | ROLE_ANONYMOUS_EN | ROLE_ANONYMOUS_ZH => Self::Anonymous,
            ROLE_GATEWAY_ADMIN_RAW | ROLE_GATEWAY_ADMIN_EN | ROLE_GATEWAY_ADMIN_ZH => {
                Self::GatewayAdmin
            }
            ROLE_UNTAGGED_RAW | ROLE_UNTAGGED_EN | ROLE_UNTAGGED_ZH => Self::Untagged,
            _ => Self::Tagged(role),
        };
        role
    }

    pub fn display_name(&self, language_ident: &LanguageIdentifier) -> String {
        let language = language_ident.language;
        let language_str = language.to_string().to_uppercase();
        const EN: &str = "EN";
        const ZH: &str = "ZH";
        match self {
            Role::Anonymous => match language_str.as_str() {
                EN => ROLE_ANONYMOUS_EN,
                ZH => ROLE_ANONYMOUS_ZH,
                _ => ROLE_ANONYMOUS_RAW,
            },
            Role::Untagged => match language_str.as_str() {
                EN => ROLE_UNTAGGED_EN,
                ZH => ROLE_UNTAGGED_ZH,
                _ => ROLE_UNTAGGED_RAW,
            },
            Role::GatewayAdmin => match language_str.as_str() {
                EN => ROLE_GATEWAY_ADMIN_EN,
                ZH => ROLE_GATEWAY_ADMIN_ZH,
                _ => ROLE_GATEWAY_ADMIN_RAW,
            },
            Role::Tagged(role) => role,
        }
        .to_string()
    }
}

impl ToString for Role {
    fn to_string(&self) -> String {
        match self {
            Role::Anonymous => ROLE_ANONYMOUS_RAW,
            Role::Untagged => ROLE_UNTAGGED_RAW,
            Role::GatewayAdmin => ROLE_GATEWAY_ADMIN_RAW,
            Role::Tagged(role) => role,
        }
        .to_string()
    }
}
