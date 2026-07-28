use diesel::sql_types::{Array, Jsonb, Nullable, Text};

diesel::define_sql_function! {
    fn replace_gateway_privilege_roles(p_rules: Jsonb, old_role: Text, new_role: Text) -> Jsonb;
}

diesel::define_sql_function! {
    fn replace_gateway_user_roles(
        p_roles: Array<Nullable<Text>>,
        old_role: Text,
        new_role: Text
    ) -> Array<Nullable<Text>>;
}
