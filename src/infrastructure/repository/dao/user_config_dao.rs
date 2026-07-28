use anyhow::{Context, Result};
use cookie::time::OffsetDateTime;
use diesel::{
    BoolExpressionMethods, ExpressionMethods, QueryDsl, SelectableHelper, delete, dsl::update,
    insert_into,
};
use diesel_async::{AsyncPgConnection, RunQueryDsl};

use crate::{
    infrastructure::{
        repository::po::user_config_po::{
            UserConfigCreatePo, UserConfigQueryPo, UserConfigUpdatePo,
        },
        utility::diesel_errors_mapper::map_diesel_result,
    },
    schemas::{
        bifrost::gateway_user_config::{self, id, mtime, platform, roles, user_id},
        db_functions::replace_gateway_user_roles,
    },
};

pub(crate) async fn insert_record(
    conn: &mut AsyncPgConnection,
    po: UserConfigCreatePo,
) -> Result<Option<UserConfigQueryPo>> {
    let user_config = insert_into(gateway_user_config::table)
        .values(po)
        .on_conflict((platform, user_id))
        .do_nothing()
        .returning(UserConfigQueryPo::as_returning())
        .get_result(conn)
        .await;
    map_diesel_result(user_config)
}

pub(crate) async fn update_record(
    conn: &mut AsyncPgConnection,
    po: UserConfigUpdatePo,
) -> Result<Option<UserConfigQueryPo>> {
    if !po.should_update() {
        return Ok(None);
    }
    let changed = update(gateway_user_config::table)
        .filter(user_id.eq(&po.user_id).and(platform.eq(&po.platform)))
        .set((&po, mtime.eq(OffsetDateTime::now_utc())))
        .returning(UserConfigQueryPo::as_returning())
        .get_result(conn)
        .await;
    map_diesel_result(changed)
}

pub(crate) async fn delete_record(
    conn: &mut AsyncPgConnection,
    config_id: i32,
) -> Result<Option<UserConfigQueryPo>> {
    let ori_record = delete(gateway_user_config::table.filter(id.eq(config_id)))
        .returning(UserConfigQueryPo::as_returning())
        .get_result(conn)
        .await;
    map_diesel_result(ori_record)
}

pub(crate) async fn fetch_by_user_id(
    conn: &mut AsyncPgConnection,
    query_user_id: String,
) -> Result<Vec<UserConfigQueryPo>> {
    let records = gateway_user_config::table
        .select(UserConfigQueryPo::as_select())
        .filter(user_id.eq(query_user_id))
        .load(conn)
        .await
        .context("Failed to execute sql")?;
    Ok(records)
}

pub(crate) async fn fetch_any(
    conn: &mut AsyncPgConnection,
    offset: usize,
    limit: usize,
) -> Result<Vec<UserConfigQueryPo>> {
    let records = gateway_user_config::table
        .select(UserConfigQueryPo::as_select())
        .limit(limit as i64)
        .offset(offset as i64)
        .load(conn)
        .await
        .context("Failed to execute sql")?;
    Ok(records)
}

pub(crate) async fn fetch_count(conn: &mut AsyncPgConnection) -> Result<usize> {
    let count: i64 = gateway_user_config::table
        .count()
        .get_result(conn)
        .await
        .context("Failed to execute sql")?;
    Ok(count.min(0) as usize)
}

pub(crate) async fn fetch_by_ids(
    conn: &mut AsyncPgConnection,
    user_ids: Vec<String>,
) -> Result<Vec<UserConfigQueryPo>> {
    let records = gateway_user_config::table
        .select(UserConfigQueryPo::as_select())
        .filter(user_id.eq_any(user_ids))
        .get_results(conn)
        .await
        .context("Failed to execute sql")?;
    Ok(records)
}

pub(crate) async fn rename_role(
    conn: &mut AsyncPgConnection,
    old_role: String,
    new_role: String,
) -> Result<usize> {
    let updated_count = update(gateway_user_config::table)
        .set(roles.eq(replace_gateway_user_roles(roles, old_role, new_role)))
        .execute(conn)
        .await
        .context("Failed to execute sql")?;
    Ok(updated_count)
}
