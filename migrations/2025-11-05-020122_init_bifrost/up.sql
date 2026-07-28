CREATE TABLE IF NOT EXISTS public.gateway_privilege_config (
    id SERIAL NOT NULL,
    platform VARCHAR(255) NOT NULL,
    config_key VARCHAR(255) NOT NULL,
    -- e.g. { "key": "currentUser", "service": "user", "url_path": "/api/user/current", "roles": ["normal"] }
    backend_rules JSONB NOT NULL,
    config_version INT NOT NULL,
    operator_id VARCHAR(255) NOT NULL,
    status VARCHAR(20) NOT NULL CHECK (
        status IN ('enable', 'disable')
    ) DEFAULT 'enable',
    ctime TIMESTAMP
    WITH
        TIME ZONE DEFAULT CURRENT_TIMESTAMP,
        mtime TIMESTAMP
    WITH
        TIME ZONE DEFAULT CURRENT_TIMESTAMP,
        PRIMARY KEY (id),
        UNIQUE (platform, config_key),
        CHECK (
            jsonb_array_length (backend_rules) > 0
        )
);

CREATE INDEX idx_platform_privilege_key ON public.gateway_privilege_config (platform, config_key);

CREATE INDEX idx_gin_priv_roles ON public.gateway_privilege_config USING GIN (
    (backend_rules -> 'roles') jsonb_path_ops
);

CREATE TABLE IF NOT EXISTS public.gateway_user_config (
    id SERIAL NOT NULL,
    user_id VARCHAR(255) NOT NULL,
    platform VARCHAR(255) NOT NULL,
    -- e.g. ["admin", "manager"]
    roles TEXT [] NOT NULL,
    operator_id VARCHAR(255) NOT NULL,
    ctime TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    mtime TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (id),
    UNIQUE (platform, user_id),
    CHECK (array_length(roles, 1) > 0)
);

CREATE INDEX idx_platform_user_key ON public.gateway_user_config (platform, user_id);

CREATE INDEX idx_gin_user_roles ON public.gateway_user_config USING GIN (roles);