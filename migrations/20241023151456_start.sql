-- Add migration script here
CREATE TABLE IF NOT EXISTS res (
    no SERIAL PRIMARY KEY,
    name_and_trip TEXT NOT NULL,
    datetime TIMESTAMP NOT NULL,
    datetime_text TEXT NOT NULL,
    id TEXT NOT NULL,
    main_text TEXT NOT NULL,
    main_text_html TEXT NOT NULL,
    oekaki_id INT
);

CREATE TABLE IF NOT EXISTS oekaki (
    oekaki_id INT PRIMARY KEY,
    oekaki_title TEXT NOT NULL,
    original_oekaki_res_no INT
)
