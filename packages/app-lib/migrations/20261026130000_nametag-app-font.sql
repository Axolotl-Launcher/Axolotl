-- Whether the skins-page nametag follows the launcher interface font.
-- 0 keeps the Minecraft pixel font, which previews how a name renders in-game.
ALTER TABLE settings ADD COLUMN nametag_uses_app_font INTEGER NOT NULL DEFAULT 0 CHECK (nametag_uses_app_font IN (0, 1));
