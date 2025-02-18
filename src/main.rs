use std::io::Write;

use nico_bbs::Board;
use niconico::{login, Credentials};

#[tokio::main]
async fn main() {
    let tmpfile = set_openssl_config();

    dotenvy::dotenv().ok();
    let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();

    let save_dir = std::env::var("SAVE_DIR").expect("SAVE_DIR must be set");

    let credentials = envy::from_env::<Credentials>().unwrap();
    let user_session = login(credentials).await.unwrap();

    let mut board = Board::new("https://ch.nicovideo.jp/unkchanel/bbs", "ch2598430");

    board.seek_res(&pool, &user_session, &save_dir).await;

    drop(tmpfile);
}

pub fn set_openssl_config() -> tempfile::NamedTempFile {
    let opensslconf_contents = include_str!("openssl.cnf");
    let mut tmpfile = tempfile::NamedTempFile::new().unwrap();
    tmpfile.write_all(opensslconf_contents.as_bytes()).unwrap();
    std::env::set_var("OPENSSL_CONF", tmpfile.path());

    tmpfile
}
