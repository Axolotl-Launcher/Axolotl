use super::*;

pub(super) fn entries() -> Vec<Entry> {
    use Access::*;
    let account = json!({"account_id":{"type":"string","minLength":72,"maxLength":72,"format":"account-id"}});
    let mut list = page();
    list["offline_only"] = json!({"type":"boolean"});
    vec![
        entry(
            "account_list",
            "List saved Minecraft account summaries using cached profiles only, without refreshing tokens or querying authentication servers. Paginated, optionally offline_only. Returns MCP account_id (a stable digest, distinct from native launcher IDs), account_type, player_name/UUID, active state and stored expiry. Tokens, passwords, third-party server addresses and skins/capes are omitted. Cached names and expiry are not proof a login will succeed.",
            Controlled,
            true,
            true,
            list,
            &[],
        ),
        entry(
            "account_get",
            "Read one saved account summary by MCP account_id from account_list. Uses cached profile data, does not refresh/validate login. Does not reveal credentials or authentication-server details.",
            Controlled,
            true,
            true,
            account.clone(),
            &["account_id"],
        ),
        entry(
            "account_get_default",
            "Read the configured default account summary. offline_mode=true selects the launcher's offline fallback (active offline account or alphabetically first offline account). May return null. Does not refresh tokens or change defaults; offline eligibility is queried separately by account_get_eligibility.",
            Controlled,
            true,
            true,
            json!({"offline_mode":{"type":"boolean"}}),
            &[],
        ),
        entry(
            "account_set_default",
            "Select an existing account from account_list as global default for future game launches. Returns operation_id; poll operations_get. Offline accounts must satisfy the runtime's existing eligibility rules. Does not modify a running game's account or disclose credentials; native account updates may hydrate cached profile state.",
            Controlled,
            false,
            true,
            account.clone(),
            &["account_id"],
        ),
        entry(
            "account_remove",
            "Remove one saved account and its profile cache from the launcher. Requires full access. Returns operation_id; poll operations_get. If active, native behavior may select another default account. Does not delete Microsoft/third-party identities remotely or stop existing game processes. Re-login may be needed; do not retry blindly.",
            FullAccess,
            false,
            false,
            account,
            &["account_id"],
        ),
        entry(
            "account_add_offline",
            "Create an offline Minecraft account using 1-16 letters/numbers/underscores, optionally a custom UUID. Uses existing offline eligibility checks and makes the new account active. Rejects an existing account with the same generated/custom UUID. Returns operation_id; poll operations_get for a credential-free account summary. Does not grant official ownership or bypass offline restrictions.",
            Controlled,
            false,
            false,
            json!({"username":{"type":"string","minLength":1,"maxLength":16,"format":"minecraft-username"},"uuid":{"type":"string","format":"uuid"}}),
            &["username"],
        ),
        entry(
            "account_get_eligibility",
            "Read the launcher's current offline-account eligibility status (region/restricted/revision) and offline_accounts_allowed. Does not change region, ownership proof or restriction settings. This is a snapshot; account changes and launches recheck eligibility at execution time.",
            Controlled,
            true,
            true,
            json!({}),
            &[],
        ),
    ]
}
