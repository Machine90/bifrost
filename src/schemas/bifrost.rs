// @generated automatically by Diesel CLI.

diesel::table! {
    gateway_privilege_config (id) {
        id -> Int4,
        #[max_length = 255]
        platform -> Varchar,
        #[max_length = 255]
        config_key -> Varchar,
        backend_rules -> Jsonb,
        config_version -> Int4,
        #[max_length = 255]
        operator_id -> Varchar,
        #[max_length = 20]
        status -> Varchar,
        ctime -> Nullable<Timestamptz>,
        mtime -> Nullable<Timestamptz>,
    }
}

diesel::table! {
    gateway_user_config (id) {
        id -> Int4,
        #[max_length = 255]
        user_id -> Varchar,
        #[max_length = 255]
        platform -> Varchar,
        roles -> Array<Nullable<Text>>,
        #[max_length = 255]
        operator_id -> Varchar,
        ctime -> Nullable<Timestamptz>,
        mtime -> Nullable<Timestamptz>,
    }
}

diesel::allow_tables_to_appear_in_same_query!(gateway_privilege_config, gateway_user_config,);
