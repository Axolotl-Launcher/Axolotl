ALTER TABLE settings ADD COLUMN hide_chinese_content_titles INTEGER NOT NULL DEFAULT 0 CHECK (hide_chinese_content_titles IN (0, 1));
