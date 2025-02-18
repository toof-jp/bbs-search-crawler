use scraper::{Html, Selector};
extern crate derive_more;
extern crate regex;
use chrono::NaiveDateTime;
use kuchikiki::traits::*;
use kuchikiki::NodeData;
use lazy_static::lazy_static;
use regex::Regex;

#[derive(Debug, Default)]
pub struct Oekaki {
    pub oekaki_id: i32,
    pub oekaki_title: Option<String>,
    pub original_oekaki_res_no: Option<i32>,
}

impl Oekaki {
    pub fn get_url(&self, bbs_id: &str) -> String {
        format!(
            "https://dic.nicovideo.jp/b/c/{}/oekaki/{}.png",
            bbs_id, self.oekaki_id
        )
    }
}

#[derive(Debug, Default)]
pub struct Res {
    pub no: i32,
    pub name_and_trip: String,
    pub datetime: NaiveDateTime, // TODO 削除されたレスのdateは前後のレスのdateから埋める？
    pub datetime_text: String,   // 曜日を含むdatetime
    pub id: String,
    pub main_text: String, // 検索用のテキスト
    pub main_text_html: String,
    pub oekaki_id: Option<i32>,
}

impl Res {
    pub fn parse_res_head(&mut self, html: &str) {
        let a_selector = Selector::parse("a").unwrap();
        let span_name_selector = Selector::parse("span.name").unwrap();
        let span_trip_selector = Selector::parse("span.trip").unwrap();

        let fragment = Html::parse_fragment(html);

        self.no = fragment
            .select(&a_selector)
            .next()
            .unwrap()
            .value()
            .attr("name")
            .unwrap()
            .parse()
            .unwrap();

        if let Some(e) = fragment.select(&span_name_selector).next() {
            let html = e.inner_html();
            let text = html_escape::decode_html_entities(&html);
            // self.name = Some(text.to_string());
            self.name_and_trip.push_str(&text);
        }

        if let Some(e) = fragment.select(&span_trip_selector).next() {
            let html = e.inner_html();
            let text = html_escape::decode_html_entities(&html);
            // self.trip = Some(text.to_string());
            self.name_and_trip.push(' ');
            self.name_and_trip.push_str(&text);
        }

        let ku_dt_selector = "dt";

        let ctx_name = html5ever::QualName::new(None, ns!(html), local_name!("dt"));
        let ctx_attr = Vec::new();
        let ku_fragment = kuchikiki::parse_fragment(ctx_name, ctx_attr).one(html);

        let a_node = ku_fragment.select(ku_dt_selector).unwrap().next().unwrap();

        let date_and_id_text = a_node
            .as_node()
            .children()
            .last()
            .unwrap()
            .as_text()
            .unwrap()
            .borrow()
            .to_string();

        lazy_static! {
            static ref DATETIME_REGEX: Regex =
                Regex::new(r"(\d{4}/\d{2}/\d{2})(\(\w+\)) (\d{2}:\d{2}:\d{2})").unwrap();
            static ref ID_REGEX: Regex = Regex::new(r"ID:\s*([\w+/]+)").unwrap();
        }

        match date_and_id_text.find("削除しました") {
            Some(_) => {
                self.datetime_text = "削除しました".to_string();
            }
            None => {
                let caps = DATETIME_REGEX.captures(&date_and_id_text).unwrap();
                let datetime = format!("{} {}", &caps[1], &caps[3]);
                self.datetime =
                    NaiveDateTime::parse_from_str(&datetime, "%Y/%m/%d %H:%M:%S").unwrap();
                self.datetime_text = format!("{}{} {}", &caps[1], &caps[2], &caps[3]);
            }
        }

        let caps = ID_REGEX.captures(&date_and_id_text).unwrap();
        self.id = caps[1].to_string();
    }

    pub fn parse_res_body(&mut self, html: &str) -> Option<Oekaki> {
        // TODO
        // TODO
        // sc_ means scraper
        // ku_ means kuchikiki
        let mut texts = Vec::<String>::new();
        let mut htmls = Vec::<String>::new();

        let sc_children_selector = Selector::parse("dd > *").unwrap();
        let sc_fragment = Html::parse_fragment(html);
        let mut elements = sc_fragment.select(&sc_children_selector);

        let ku_dd_selector = "dd";

        let ctx_name = html5ever::QualName::new(None, ns!(html), local_name!("dd"));
        let ctx_attr = Vec::new();
        let ku_fragment = kuchikiki::parse_fragment(ctx_name, ctx_attr).one(html);

        // TODO node?
        let dd_node = ku_fragment.select(ku_dd_selector).unwrap().next().unwrap();

        let mut oekaki_is_found = false;
        let mut oekakiko = Oekaki::default();

        lazy_static! {
            static ref TITLE_REGEX: Regex = Regex::new(r"^\s*タイトル:(.*)").unwrap();
            static ref OEKAKI_URL_REGEX: Regex = Regex::new(r"#(.*)").unwrap();
            static ref OEKAKI_ID_REGEX: Regex = Regex::new(r"^oekaki(.+)$").unwrap();
        }

        for child_node in dd_node.as_node().children() {
            match child_node.data() {
                NodeData::Text(t) => {
                    if oekaki_is_found {
                        // oekakiのタイトル
                        let text = t.borrow().to_string();
                        if let Some(caps) = TITLE_REGEX.captures(&text) {
                            let title = &caps[1];
                            texts.push(format!("タイトル:{}", title)); // 検索用
                            oekakiko.oekaki_title = Some(title.to_string());
                        }
                    } else {
                        // 本文
                        texts.push(t.borrow().to_string());
                        htmls.push(html_escape::encode_text(&t.borrow().to_string()).to_string());
                    }
                }
                NodeData::Element(_) => {
                    let element = elements.next().unwrap();

                    match element.value().name() {
                        "a" => {
                            if oekaki_is_found {
                                if let Some(original_oekaki_url) = element.value().attr("href") {
                                    let caps =
                                        OEKAKI_URL_REGEX.captures(original_oekaki_url).unwrap();
                                    let original_oekaki_res_no = caps[1].parse().unwrap();

                                    oekakiko.original_oekaki_res_no = Some(original_oekaki_res_no);
                                }
                            } else if element
                                .value()
                                .classes()
                                .any(|x| x == "auto" || x == "auto-hdn")
                            {
                                // 大百科への自動リンク
                                // "auto-hdn"は1文字のリンク
                                // 大百科のリンクはHTMLには残さない
                                texts.push(
                                    html_escape::decode_html_entities(&element.inner_html())
                                        .to_string(),
                                );
                                // TODO decode_html_entitiesでいいのか？
                                htmls.push(element.inner_html());
                            } else if element.value().attr("class") == Some("dic") {
                                // アンカー
                                let mut url = "https://dic.nicovideo.jp".to_string();
                                url.push_str(element.value().attr("href").unwrap());

                                let (t, h) = Self::parse_res_body_link(&element.html(), &url);
                                texts.push(t);
                                htmls.push(h);
                            } else if element.value().attr("target") == Some("_blank") {
                                // URLリンク
                                // ニコニコ動画などのリンク(>>sm9)

                                let (t, h) = Self::parse_res_body_link(
                                    &element.html(),
                                    element.value().attr("href").unwrap(),
                                );
                                texts.push(t);
                                htmls.push(h);
                            } else {
                                // TODO log
                            }
                        }
                        "br" => {
                            //texts.push("\n".to_string());
                            if !oekaki_is_found {
                                texts.push("\n".to_string());
                                htmls.push("<br>".to_string());
                            }
                        }
                        "iframe" => {
                            // TODO error
                            // iframeの前の改行を消す
                            assert_eq!(texts.pop(), Some("\n".to_string()));
                            assert_eq!(htmls.pop(), Some("<br>".to_string()));
                        }
                        "div" => {
                            oekaki_is_found = true;

                            // divの前の改行を消す
                            // text nodeの改行とbrタグを消す
                            texts.pop();
                            assert_eq!(texts.pop(), Some("\n".to_string()));
                            htmls.pop();
                            assert_eq!(htmls.pop(), Some("<br>".to_string()));

                            let id = element.value().id().unwrap();
                            let caps = OEKAKI_ID_REGEX.captures(id).unwrap();
                            oekakiko.oekaki_id = caps[1].to_string().parse().unwrap();
                        }
                        _ => (), // TODO log出力
                    }
                }
                _ => (),
            }
        }

        // dbg!(&htmls);

        self.main_text = texts.join("").trim().to_string();
        self.main_text_html = htmls.join("").trim().to_string();

        if oekaki_is_found {
            self.oekaki_id = Some(oekakiko.oekaki_id);
            Some(oekakiko)
        } else {
            None
        }

        // TODO String.trim()すると全角スペースも消える
        // TODO str.trim_matches で半角スペースだけ消す

        // htmlからmain_textを作る

        // main_textにoekakiのタイトルを入れる（検索用）

        // TODO self.main_text
        // TODO self.main_text_html
    }

    // return (texts: String, htmls: String)
    fn parse_res_body_link(html: &str, href: &str) -> (String, String) {
        // TODO
        // sc_ scraper
        // ku_ kuchikiki

        let mut texts: String = String::new();
        let mut htmls: String = String::new();

        let sc_fragment = Html::parse_fragment(html);
        let sc_children_selector = Selector::parse("a > *").unwrap();
        let mut sc_elements = sc_fragment.select(&sc_children_selector);

        let ku_a_selector = "a";

        let ctx_name = html5ever::QualName::new(None, ns!(html), local_name!("a"));
        let ctx_attr = Vec::new();
        let ku_fragment = kuchikiki::parse_fragment(ctx_name, ctx_attr).one(html);

        let a_node = ku_fragment.select(ku_a_selector).unwrap().next().unwrap();

        for child_node in a_node.as_node().children() {
            match child_node.data() {
                NodeData::Text(t) => {
                    let text = t.borrow().to_string();
                    texts.push_str(&text);
                    htmls.push_str(&html_escape::encode_text(&text));
                }
                NodeData::Element(_) => {
                    let a_element = sc_elements.next().unwrap();
                    match a_element.value().name() {
                        "wbr" => {
                            htmls.push_str("<wbr>");
                        }
                        _ => {
                            // TODO log
                        }
                    }
                }
                _ => (),
            }
        }

        (
            texts,
            format!(
                r#"<a href="{}">{}</a>"#,
                html_escape::encode_text(href),
                htmls
            ),
        )
    }
}
