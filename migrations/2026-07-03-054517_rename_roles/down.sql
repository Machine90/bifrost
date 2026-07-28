-- This file should undo anything in `up.sql`

DROP FUNCTION IF EXISTS replace_gateway_privilege_roles (jsonb, text, text);

DROP FUNCTION IF EXISTS replace_gateway_user_roles (text [], text, text);