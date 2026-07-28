#[derive(Debug, Clone)]
pub struct RenameRoleResult {
    pub updated_privilege_count: usize,
    pub updated_user_count: usize,
}
