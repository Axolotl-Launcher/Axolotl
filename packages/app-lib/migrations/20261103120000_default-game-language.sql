-- The language Minecraft uses the first time an instance launches.
-- An empty value keeps following the launcher locale.
ALTER TABLE settings ADD COLUMN default_game_language TEXT NOT NULL DEFAULT '';
