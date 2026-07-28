-- Your SQL goes here

-- Function for replacing roles in gateway_privilege_config
CREATE OR REPLACE FUNCTION replace_gateway_privilege_roles(
    p_rules jsonb,
    p_old_role text,
    p_new_role text
)
RETURNS jsonb AS $$
BEGIN
    -- Return the original rules if the old role is not present
    IF NOT (p_rules @> jsonb_build_array(jsonb_build_object('roles', jsonb_build_array(p_old_role)))) THEN
        RETURN p_rules;
    END IF;

    RETURN (
        SELECT jsonb_agg(
            jsonb_set(
                elem,
                '{roles}',
                (
                    SELECT jsonb_agg(
                        CASE WHEN value = to_jsonb(p_old_role) THEN to_jsonb(p_new_role) ELSE value END
                    )
                    FROM jsonb_array_elements(elem->'roles') AS value
                )
            )
        )
        FROM jsonb_array_elements(p_rules) AS elem
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Function for replacing roles in gateway_user_config.roles array
CREATE OR REPLACE FUNCTION replace_gateway_user_roles(
    p_roles text[],
    p_old_role text,
    p_new_role text
)
RETURNS text[] AS $$
BEGIN
    RETURN (
        SELECT array_agg(DISTINCT elem)  -- 添加 DISTINCT 去重
        FROM (
            SELECT 
                CASE WHEN elem = p_old_role THEN p_new_role ELSE elem END AS elem
            FROM unnest(p_roles) AS elem
        ) t
        WHERE elem IS NOT NULL  -- 移除 NULL 值
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;
