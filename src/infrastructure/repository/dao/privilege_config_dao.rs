use anyhow::{Context, Result};
use cookie::time::OffsetDateTime;
use diesel::{
    ExpressionMethods, QueryDsl, SelectableHelper,
    dsl::{delete, update},
    insert_into,
};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::{
    infrastructure::{
        repository::po::privilege_po::{PrivilegeCreatePo, PrivilegeQueryPo, PrivilegeUpdatePo},
        utility::diesel_errors_mapper::map_diesel_result,
    },
    schemas::{
        bifrost::gateway_privilege_config::{
            self, backend_rules, config_key, id, mtime, platform, status,
        },
        db_functions::replace_gateway_privilege_roles,
    },
};

pub(crate) async fn insert_record(
    conn: &mut AsyncPgConnection,
    po: PrivilegeCreatePo,
) -> Result<Option<i32>> {
    let rule_id = insert_into(gateway_privilege_config::table)
        .values(po)
        .on_conflict((platform, config_key))
        .do_nothing()
        .returning(id)
        .get_result::<i32>(conn)
        .await;
    map_diesel_result(rule_id)
}

pub(crate) async fn update_record(
    conn: &mut AsyncPgConnection,
    po: PrivilegeUpdatePo,
) -> Result<Option<PrivilegeQueryPo>> {
    if !po.should_update() {
        return Ok(None);
    }
    let changed = update(gateway_privilege_config::table)
        .filter(id.eq(po.id))
        .set((&po, mtime.eq(OffsetDateTime::now_utc())))
        .returning(PrivilegeQueryPo::as_returning())
        .get_result(conn)
        .await;
    map_diesel_result(changed)
}

pub(crate) async fn delete_record(
    conn: &mut AsyncPgConnection,
    rule_id: i32,
) -> Result<Option<PrivilegeQueryPo>> {
    let ori_record = delete(gateway_privilege_config::table.filter(id.eq(rule_id)))
        .returning(PrivilegeQueryPo::as_returning())
        .get_result(conn)
        .await;
    map_diesel_result(ori_record)
}

pub(crate) async fn fetch_any(
    conn: &mut AsyncPgConnection,
    offset: usize,
    limit: usize,
) -> Result<Vec<PrivilegeQueryPo>> {
    let records = gateway_privilege_config::table
        .filter(status.eq("enable"))
        .select(PrivilegeQueryPo::as_select())
        .limit(limit as i64)
        .offset(offset as i64)
        .load(conn)
        .await
        .context("Failed to execute sql")?;
    Ok(records)
}

pub(crate) async fn fetch_count(conn: &mut AsyncPgConnection) -> Result<usize> {
    let count: i64 = gateway_privilege_config::table
        .filter(status.eq("enable"))
        .count()
        .get_result(conn)
        .await
        .context("Failed to execute sql")?;
    Ok(count.min(0) as usize)
}

pub(crate) async fn rename_role(
    conn: &mut AsyncPgConnection,
    old_role: String,
    new_role: String,
) -> Result<usize> {
    let updated_count = update(gateway_privilege_config::table)
        .set(backend_rules.eq(replace_gateway_privilege_roles(
            backend_rules,
            old_role,
            new_role,
        )))
        .execute(conn)
        .await
        .context("Failed to execute sql")?;
    Ok(updated_count)
}
