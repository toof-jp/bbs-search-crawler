use anyhow::Result;

use crate::res::{Oekaki, Res};

pub async fn insert_res(pool: &sqlx::PgPool, res: &Res) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO res (no, name_and_trip, datetime, datetime_text, id, main_text, main_text_html, oekaki_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        "#,
        res.no,
        res.name_and_trip,
        res.datetime,
        res.datetime_text,
        res.id,
        res.main_text,
        res.main_text_html,
        res.oekaki_id,
    )
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_max_res_no(pool: &sqlx::PgPool) -> Result<i32> {
    let no = sqlx::query!("SELECT MAX(no) FROM res")
        .fetch_one(pool)
        .await?;

    Ok(no.max.unwrap_or(0))
}

pub async fn insert_oekaki(pool: &sqlx::PgPool, oekaki: &Oekaki) -> Result<()> {
    sqlx::query!(
        r#"
        INSERT INTO oekaki (oekaki_id, oekaki_title, original_oekaki_res_no)
        VALUES ($1, $2, $3)
        "#,
        oekaki.oekaki_id,
        oekaki.oekaki_title,
        oekaki.original_oekaki_res_no,
    )
    .execute(pool)
    .await?;

    Ok(())
}
