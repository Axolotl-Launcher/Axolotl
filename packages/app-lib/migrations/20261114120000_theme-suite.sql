ALTER TABLE settings ADD COLUMN theme_source TEXT NOT NULL DEFAULT 'builtin';
ALTER TABLE settings ADD COLUMN custom_theme_colors TEXT;
