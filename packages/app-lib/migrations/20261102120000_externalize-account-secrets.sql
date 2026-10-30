-- The account secrets move into the system credential store, and each row keeps
-- a mark saying which of its secret columns the store holds. A row whose secret
-- could not be moved keeps it in the column, so its mark stays 0 and the move
-- is tried again on the next start.
--
-- The mark is a set of bits, one per secret column of the row, so a column the
-- account never had (an offline account has no tokens, a Microsoft account no
-- Yggdrasil client token) is not mistaken for one the store lost.
--
-- A migration cannot reach the credential store, so this one only makes room for
-- the move; `state/store/secrets.rs` moves the rows at startup.

ALTER TABLE minecraft_users
	ADD COLUMN secrets_externalized INTEGER NOT NULL DEFAULT 0;

ALTER TABLE minecraft_device_tokens
	ADD COLUMN secrets_externalized INTEGER NOT NULL DEFAULT 0;

ALTER TABLE modrinth_users
	ADD COLUMN secrets_externalized INTEGER NOT NULL DEFAULT 0;
