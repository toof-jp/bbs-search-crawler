use std::fs::{create_dir_all, File};
use std::io::copy;
use std::path::Path;
use std::time::Duration;

use anyhow::Result;
use niconico::UserSession;
use reqwest::{Client, Response};
use scraper::{Html, Selector};
use secrecy::ExposeSecret;
use tokio::time::sleep;

use crate::res::{Oekaki, Res};
use crate::sql_client::{get_max_res_no, insert_oekaki, insert_res};

#[derive(Clone, Debug)]
struct Offset {
    offset: i32,
}

impl From<i32> for Offset {
    fn from(res_no: i32) -> Self {
        assert!(res_no != 0);
        Offset {
            offset: (res_no - 1) / 30 * 30,
        }
    }
}

impl Iterator for Offset {
    type Item = Offset;

    fn next(&mut self) -> Option<Self::Item> {
        let offset = self.clone();
        self.offset += 30;
        Some(offset)
    }
}

#[derive(Debug)]
pub struct Board {
    pub top_bbs_url: String,
    pub bbs_id: String,
    pub hash_key: Option<String>,
}

impl Board {
    pub fn new(top_bbs_url: &str, bbs_id: &str) -> Board {
        Board {
            top_bbs_url: top_bbs_url.to_string(),
            bbs_id: bbs_id.to_string(),
            hash_key: None,
        }
    }

    pub async fn get_hash_key(&mut self, user_session: &UserSession) -> Result<()> {
        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 5;
        const RETRY_DELAY: Duration = Duration::from_secs(1);

        while attempts < MAX_ATTEMPTS {
            match self.try_get_hash_key(user_session).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    attempts += 1;
                    if attempts == MAX_ATTEMPTS {
                        return Err(e);
                    }
                    sleep(RETRY_DELAY).await;
                }
            }
        }
        unreachable!()
    }

    async fn try_get_hash_key(&mut self, user_session: &UserSession) -> Result<()> {
        let response = Client::new()
            .get(&self.top_bbs_url)
            .header(reqwest::header::COOKIE, user_session.0.expose_secret())
            .send()
            .await?
            .text()
            .await?;

        self.hash_key = Some(Self::extract_hash_key_from_html(&response)?);
        Ok(())
    }

    fn extract_hash_key_from_html(html: &str) -> Result<String> {
        let document = Html::parse_document(html);
        let iframe_selector = Selector::parse("#community-bbs").unwrap();

        let url_with_hash_key = document
            .select(&iframe_selector)
            .next()
            .unwrap()
            .value()
            .attr("src")
            .unwrap();
        let url_with_hash_key = reqwest::Url::parse(url_with_hash_key).unwrap();
        let hash_key = url_with_hash_key
            .query_pairs()
            .next()
            .unwrap()
            .1
            .to_string();

        Ok(hash_key)
    }

    pub async fn get_with_hash_key(
        &mut self,
        url: &str,
        user_session: &UserSession,
    ) -> Result<Response> {
        if self.hash_key.is_none() {
            self.get_hash_key(user_session).await?;
        }

        let mut attempts = 0;
        const MAX_ATTEMPTS: u32 = 5;
        const RETRY_DELAY: Duration = Duration::from_secs(1);

        while attempts < MAX_ATTEMPTS {
            let url = format!("{}?hash_key={}", url, self.hash_key.as_ref().unwrap());
            match reqwest::get(&url).await {
                Ok(response) => {
                    if response.status().as_u16() == 200 {
                        return Ok(response);
                    }

                    attempts += 1;
                    if attempts == MAX_ATTEMPTS {
                        return Err(anyhow::anyhow!("Failed to get response"));
                    }
                    sleep(RETRY_DELAY).await;

                    // hash_key is expired
                    if response.status().as_u16() == 403 {
                        self.get_hash_key(user_session).await?;
                    }
                }
                Err(e) => {
                    attempts += 1;
                    if attempts == MAX_ATTEMPTS {
                        return Err(e.into());
                    }
                    sleep(RETRY_DELAY).await;
                }
            }
        }
        unreachable!()
    }

    fn parse_html(&self, html: &str) -> Result<Vec<(Res, Option<Oekaki>)>> {
        let mut vec = Vec::new();
        let document = Html::parse_document(html);
        let dl_children_selector = Selector::parse("dl > *").unwrap();
        let mut res = Res::default();

        for element in document.select(&dl_children_selector) {
            match element.value().name() {
                "dt" => {
                    res.parse_res_head(&element.html());
                }
                "dd" => {
                    let oekakiko = res.parse_res_body(&element.html());
                    vec.push((res, oekakiko));
                    res = Res::default();
                }
                _ => (),
            }
        }

        Ok(vec)
    }

    // get res from offset + 1 to offset + 30
    async fn get_res(
        &mut self,
        user_session: &UserSession,
        offset: &Offset,
    ) -> Result<Vec<(Res, Option<Oekaki>)>> {
        let url = format!(
            "https://dic.nicovideo.jp/b/c/{}/{}-",
            self.bbs_id,
            offset.offset + 1,
        );

        let responce = self.get_with_hash_key(&url, user_session).await?;
        let html = responce.text().await.unwrap();

        self.parse_html(&html)
    }

    pub async fn seek_res(
        &mut self,
        pool: &sqlx::PgPool,
        user_session: &UserSession,
        save_dir: &str,
    ) {
        const INTERVAL: Duration = Duration::new(5, 0);

        loop {
            let mut max_no = get_max_res_no(pool).await.unwrap();
            let offset_counter = Offset::from(max_no + 1);

            for offset in offset_counter {
                let vec = match self.get_res(user_session, &offset).await {
                    Ok(vec) => vec,
                    Err(e) => {
                        dbg!(e);
                        break;
                    }
                };

                if vec.is_empty() {
                    dbg!("break");
                    dbg!(offset);
                    break;
                }

                for (res, oekaki) in vec {
                    dbg!(res.no);
                    if res.no > max_no {
                        insert_res(pool, &res).await.unwrap();
                        if let Some(o) = oekaki {
                            insert_oekaki(pool, &o).await.unwrap();
                            self.download_oekakiko(user_session, &o, save_dir)
                                .await
                                .unwrap();
                        }
                        max_no = res.no;
                    }
                }

                sleep(INTERVAL).await;
            }
            dbg!("here");

            sleep(INTERVAL).await;
        }
    }

    async fn download_oekakiko(
        &mut self,
        user_session: &UserSession,
        oekaki: &Oekaki,
        save_dir: &str,
    ) -> Result<()> {
        let response = self
            .get_with_hash_key(&oekaki.get_url(&self.bbs_id), user_session)
            .await?;

        let bytes = response.bytes().await?;
        create_dir_all(save_dir)?;
        let file_path = Path::new(save_dir).join(format!("{}.png", oekaki.oekaki_id));
        let mut file = File::create(file_path)?;
        copy(&mut bytes.as_ref(), &mut file)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test() -> Result<()> {
        let html = include_str!("test.html");
        let board = Board::new("https://ch.nicovideo.jp/unkchanel/bbs", "ch2598430");

        let vec = board.parse_html(html);

        dbg!(&vec);

        Ok(())
    }
}
