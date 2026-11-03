use super::*;
use crate::state::instances::adapters::sqlite::test_support::test_pool;
use crate::state::{ContentOwnershipKind, ContentProvider, ProjectType};
use chrono::Utc;
use sqlx::SqlitePool;
#[tokio::test]
async fn materialized_curseforge_downloads_require_current_exact_content() {
    let pool = test_pool().await;
    sqlx::query(
        "ALTER TABLE instances ADD COLUMN applied_content_set_id TEXT NULL",
    )
    .execute(&pool)
    .await
    .expect("applied content set column");
    sqlx::raw_sql(
        "
        CREATE TABLE instance_files (
            id TEXT PRIMARY KEY,
            instance_id TEXT NOT NULL,
            missing INTEGER NOT NULL
        );
        CREATE TABLE instance_content_entries (
            id TEXT PRIMARY KEY,
            content_set_id TEXT NOT NULL,
            file_id TEXT NULL
        );
        CREATE TABLE instance_content_provider_refs (
            content_entry_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            provider_project_id TEXT NOT NULL,
            provider_release_id TEXT NULL
        );
        CREATE TABLE instance_pack_members (
            id TEXT PRIMARY KEY,
            content_set_id TEXT NOT NULL,
            content_entry_id TEXT NULL,
            provider TEXT NULL,
            provider_project_id TEXT NULL,
            provider_release_id TEXT NULL,
            materialization_state TEXT NOT NULL
        );
        ",
    )
    .execute(&pool)
    .await
    .expect("content tables");
    sqlx::query(
        "INSERT INTO instance_content_sets (id) VALUES ('target-set'), ('other-set')",
    )
    .execute(&pool)
    .await
    .expect("content sets");
    sqlx::query(
        "INSERT INTO instances (id, applied_content_set_id)
         VALUES ('target', 'target-set'), ('other', 'other-set')",
    )
    .execute(&pool)
    .await
    .expect("instances");
    sqlx::query(
        "INSERT INTO instance_files (id, instance_id, missing) VALUES
            ('file-present', 'target', 0),
            ('file-member-missing', 'target', 0),
            ('file-untracked', 'target', 0),
            ('file-missing', 'target', 1),
            ('file-other', 'other', 0)",
    )
    .execute(&pool)
    .await
    .expect("files");
    sqlx::query(
        "INSERT INTO instance_content_entries (id, content_set_id, file_id) VALUES
            ('entry-present', 'target-set', 'file-present'),
            ('entry-member-missing', 'target-set', 'file-member-missing'),
            ('entry-untracked', 'target-set', 'file-untracked'),
            ('entry-missing', 'target-set', 'file-missing'),
            ('entry-other', 'other-set', 'file-other')",
    )
    .execute(&pool)
    .await
    .expect("entries");
    sqlx::query(
        "INSERT INTO instance_content_provider_refs (
            content_entry_id, provider, provider_project_id,
            provider_release_id
         ) VALUES
            ('entry-present', 'curseforge', '1', '10'),
            ('entry-member-missing', 'curseforge', '1', '11'),
            ('entry-untracked', 'curseforge', '2', '20'),
            ('entry-missing', 'curseforge', '3', '30'),
            ('entry-other', 'curseforge', '9', '90')",
    )
    .execute(&pool)
    .await
    .expect("provider refs");
    sqlx::query(
        "INSERT INTO instance_pack_members (
            id, content_set_id, content_entry_id, provider,
            provider_project_id, provider_release_id,
            materialization_state
         ) VALUES
            ('member-present', 'target-set', 'entry-present',
                'curseforge', '1', '10', 'present'),
            ('member-missing', 'target-set', 'entry-member-missing',
                'curseforge', '1', '11', 'missing'),
            ('member-other', 'other-set', 'entry-other',
                'curseforge', '9', '90', 'present')",
    )
    .execute(&pool)
    .await
    .expect("pack members");

    let materialized = get_materialized_curseforge_downloads("target", &pool)
        .await
        .expect("materialized CurseForge downloads")
        .into_iter()
        .collect::<std::collections::HashSet<_>>();

    assert_eq!(
        materialized,
        std::collections::HashSet::from([
            ("1".to_string(), "10".to_string()),
            ("2".to_string(), "20".to_string()),
        ])
    );
}

async fn mutation_target_pool() -> SqlitePool {
    let pool = test_pool().await;
    sqlx::query(
        "ALTER TABLE instances ADD COLUMN applied_content_set_id TEXT NULL",
    )
    .execute(&pool)
    .await
    .expect("applied content set column");
    sqlx::raw_sql(
        "
        CREATE TABLE instance_files (
            id TEXT PRIMARY KEY,
            instance_id TEXT NOT NULL,
            relative_path TEXT NOT NULL
        );
        CREATE TABLE instance_content_entries (
            id TEXT PRIMARY KEY,
            content_set_id TEXT NOT NULL,
            file_id TEXT NULL,
            ownership_kind TEXT NOT NULL,
            project_type TEXT NOT NULL
        );
        CREATE TABLE instance_pack_members (
            id TEXT PRIMARY KEY,
            content_set_id TEXT NOT NULL,
            content_entry_id TEXT NULL,
            project_type TEXT NOT NULL,
            provider TEXT NULL,
            provider_project_id TEXT NULL,
            provider_release_id TEXT NULL
        );
        CREATE TABLE instance_content_provider_refs (
            content_entry_id TEXT NOT NULL,
            provider TEXT NOT NULL,
            provider_project_id TEXT NOT NULL,
            provider_release_id TEXT NULL,
            is_origin INTEGER NOT NULL
        );
        ",
    )
    .execute(&pool)
    .await
    .expect("content tables");
    sqlx::query("INSERT INTO instance_content_sets (id) VALUES ('set')")
        .execute(&pool)
        .await
        .expect("insert content set");
    sqlx::query(
        "INSERT INTO instances (id, applied_content_set_id) VALUES ('instance', 'set')",
    )
    .execute(&pool)
    .await
    .expect("insert instance");
    sqlx::query(
        "INSERT INTO instance_files (id, instance_id, relative_path)
         VALUES ('file', 'instance', 'mods/example.jar')",
    )
    .execute(&pool)
    .await
    .expect("insert untracked file");

    pool
}

#[tokio::test]
async fn mutation_target_supports_untracked_instance_files() {
    let pool = mutation_target_pool().await;

    let target = get_content_mutation_target("instance", "file", &pool)
        .await
        .expect("resolve mutation target")
        .expect("mutation target exists");

    assert_eq!(target.entry_id, None);
    assert_eq!(target.member_id, None);
    assert_eq!(target.relative_path.as_deref(), Some("mods/example.jar"));
    assert_eq!(target.ownership_kind, ContentOwnershipKind::UserAdded);
    assert_eq!(target.project_type, ProjectType::Mod);
}

#[tokio::test]
async fn mutation_target_resolves_identified_mod_after_pack_disconnect() {
    let pool = mutation_target_pool().await;
    sqlx::raw_sql(
        "INSERT INTO instance_content_entries VALUES
            ('entry', 'set', 'file', 'user_added', 'mod');
         INSERT INTO instance_content_provider_refs VALUES
            ('entry', 'modrinth', 'AANobbMI', '7pwil2dy', 0);",
    )
    .execute(&pool)
    .await
    .unwrap();

    let target = get_content_mutation_target("instance", "entry", &pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(target.provider, Some(ContentProvider::Modrinth));
    assert_eq!(target.provider_project_id.as_deref(), Some("AANobbMI"));
    assert_eq!(target.provider_release_id.as_deref(), Some("7pwil2dy"));
    assert_eq!(target.relative_path.as_deref(), Some("mods/example.jar"));
    assert_eq!(target.ownership_kind, ContentOwnershipKind::UserAdded);

    // A different provider ref must prevent an implicit provider switch.
    sqlx::query(
        "INSERT INTO instance_content_provider_refs VALUES
        ('entry', 'curseforge', '123', '456', 0)",
    )
    .execute(&pool)
    .await
    .unwrap();
    let target = get_content_mutation_target("instance", "entry", &pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(target.provider, None);

    sqlx::query(
        "UPDATE instance_content_provider_refs SET is_origin = 1
        WHERE provider = 'curseforge'",
    )
    .execute(&pool)
    .await
    .unwrap();
    let target = get_content_mutation_target("instance", "entry", &pool)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(target.provider, Some(ContentProvider::CurseForge));
    assert_eq!(target.provider_project_id.as_deref(), Some("123"));
    assert_eq!(target.provider_release_id.as_deref(), Some("456"));
}

async fn unlink_pool() -> SqlitePool {
    let pool = test_pool().await;
    sqlx::raw_sql(
        "
        ALTER TABLE instances ADD COLUMN applied_content_set_id TEXT NULL;
        ALTER TABLE instance_content_sets ADD COLUMN instance_id TEXT NOT NULL DEFAULT '';
        ALTER TABLE instance_content_sets ADD COLUMN source_kind TEXT NOT NULL DEFAULT '';
        ALTER TABLE instance_content_sets ADD COLUMN modified INTEGER NOT NULL DEFAULT 0;
        ALTER TABLE instance_content_sets ADD COLUMN revision INTEGER NOT NULL DEFAULT 0;
        CREATE TABLE instance_content_entries (
            content_set_id TEXT NOT NULL,
            source_kind TEXT NOT NULL,
            ownership_kind TEXT NOT NULL,
            modified_at INTEGER NOT NULL
        );
        CREATE TABLE instance_pack_members (content_set_id TEXT NOT NULL);
        CREATE TABLE resource_pack_unlinks (
            instance_id TEXT PRIMARY KEY NOT NULL,
            content_set_id TEXT NOT NULL,
            unlinked_at INTEGER NOT NULL
        );
        CREATE TABLE install_jobs (instance_id TEXT NOT NULL, status TEXT NOT NULL);
        CREATE TABLE instance_links (
            instance_id TEXT NOT NULL,
            link_kind TEXT NOT NULL
        );
        ",
    )
    .execute(&pool)
    .await
    .expect("pack unlink tables");
    pool
}

async fn pack_managed_instance(pool: &SqlitePool) {
    sqlx::query(
        "INSERT INTO instances (id, applied_content_set_id)
         VALUES ('instance', 'set')",
    )
    .execute(pool)
    .await
    .expect("insert instance");
    sqlx::query(
        "INSERT INTO instance_content_sets (id, instance_id, source_kind)
         VALUES ('set', 'instance', 'modpack')",
    )
    .execute(pool)
    .await
    .expect("insert content set");
    sqlx::query(
        "INSERT INTO instance_content_entries VALUES
            ('set', 'modpack', 'pack_managed', 0),
            ('set', 'modrinth', 'user_added', 0)",
    )
    .execute(pool)
    .await
    .expect("insert entries");
    sqlx::query(
        "INSERT INTO instance_pack_members (content_set_id) VALUES ('set')",
    )
    .execute(pool)
    .await
    .expect("insert pack member");
}

#[tokio::test]
async fn unlink_pack_rejects_a_content_set_of_another_instance() {
    let pool = unlink_pool().await;
    sqlx::query("INSERT INTO instances (id) VALUES ('instance')")
        .execute(&pool)
        .await
        .expect("insert instance");
    sqlx::query(
        "INSERT INTO instance_content_sets (id, instance_id) VALUES ('foreign', 'other')",
    )
    .execute(&pool)
    .await
    .expect("insert foreign content set");
    let mut tx = pool.begin().await.expect("begin transaction");

    let error =
        unlink_pack_in_transaction("instance", "foreign", Utc::now(), &mut tx)
            .await
            .expect_err("a content set of another instance must be refused");

    assert!(
        error
            .to_string()
            .contains("does not belong to this instance"),
        "unexpected error: {error}"
    );
}

#[tokio::test]
async fn unlink_pack_refuses_while_a_job_is_running() {
    let pool = unlink_pool().await;
    pack_managed_instance(&pool).await;
    sqlx::query(
        "INSERT INTO install_jobs (instance_id, status) VALUES ('instance', 'running')",
    )
    .execute(&pool)
    .await
    .expect("insert running job");
    let mut tx = pool.begin().await.expect("begin transaction");

    let error =
        unlink_pack_in_transaction("instance", "set", Utc::now(), &mut tx)
            .await
            .expect_err("a running job must block the unlink");
    assert!(
        error
            .to_string()
            .contains("Wait for the active instance task"),
        "unexpected error: {error}"
    );

    let markers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM resource_pack_unlinks")
            .fetch_one(&mut *tx)
            .await
            .expect("count markers");
    assert_eq!(markers, 0, "the refusal must not leave a marker");
}

#[tokio::test]
async fn unlink_pack_turns_pack_content_into_user_added_content() {
    let pool = unlink_pool().await;
    pack_managed_instance(&pool).await;
    let mut tx = pool.begin().await.expect("begin transaction");

    unlink_pack_in_transaction("instance", "set", Utc::now(), &mut tx)
        .await
        .expect("unlink the pack");
    tx.commit().await.expect("commit unlink");

    let marker: String =
        sqlx::query_scalar("SELECT content_set_id FROM resource_pack_unlinks")
            .fetch_one(&pool)
            .await
            .expect("stored marker");
    assert_eq!(marker, "set");
    let source_kind: String = sqlx::query_scalar(
        "SELECT source_kind FROM instance_content_sets WHERE id = 'set'",
    )
    .fetch_one(&pool)
    .await
    .expect("content set source kind");
    assert_eq!(source_kind, "local");
    let pack_entries: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM instance_content_entries
         WHERE ownership_kind = 'pack_managed'",
    )
    .fetch_one(&pool)
    .await
    .expect("count pack entries");
    assert_eq!(pack_entries, 0);
    let members: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM instance_pack_members")
            .fetch_one(&pool)
            .await
            .expect("count members");
    assert_eq!(members, 0);
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM instance_content_sets WHERE id = 'set'",
    )
    .fetch_one(&pool)
    .await
    .expect("content set revision");
    assert_eq!(revision, 1);

    let mut tx = pool.begin().await.expect("begin transaction");
    relink_pack_in_transaction("instance", &mut tx)
        .await
        .expect("relink the pack");
    tx.commit().await.expect("commit relink");

    let markers: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM resource_pack_unlinks")
            .fetch_one(&pool)
            .await
            .expect("count markers");
    assert_eq!(markers, 0);
}

#[tokio::test]
async fn reconcile_pack_unlink_reapplies_a_stale_marker() {
    let pool = unlink_pool().await;
    pack_managed_instance(&pool).await;
    sqlx::query(
        "INSERT INTO instance_links (instance_id, link_kind) VALUES ('instance', 'unmanaged')",
    )
    .execute(&pool)
    .await
    .expect("insert unmanaged link");
    sqlx::query(
        "INSERT INTO resource_pack_unlinks VALUES ('instance', 'set', 0)",
    )
    .execute(&pool)
    .await
    .expect("insert marker");
    let mut tx = pool.begin().await.expect("begin transaction");

    reconcile_pack_unlink_in_transaction("instance", &mut tx)
        .await
        .expect("reconcile the marker");
    tx.commit().await.expect("commit reconcile");

    let pack_entries: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM instance_content_entries
         WHERE ownership_kind = 'pack_managed'",
    )
    .fetch_one(&pool)
    .await
    .expect("count pack entries");
    assert_eq!(pack_entries, 0);
    let entries: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM instance_content_entries")
            .fetch_one(&pool)
            .await
            .expect("count entries");
    assert_eq!(entries, 2, "a user-added entry must stay as it was");
    let members: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM instance_pack_members")
            .fetch_one(&pool)
            .await
            .expect("count members");
    assert_eq!(members, 0);
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM instance_content_sets WHERE id = 'set'",
    )
    .fetch_one(&pool)
    .await
    .expect("content set revision");
    assert_eq!(revision, 1);

    let mut tx = pool.begin().await.expect("begin transaction");
    reconcile_pack_unlink_in_transaction("instance", &mut tx)
        .await
        .expect("reconcile without pack content");
    tx.commit().await.expect("commit second reconcile");

    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM instance_content_sets WHERE id = 'set'",
    )
    .fetch_one(&pool)
    .await
    .expect("content set revision");
    assert_eq!(revision, 1, "a reconciled marker must not bump again");
}

#[tokio::test]
async fn reconcile_pack_unlink_leaves_a_linked_instance_alone() {
    let pool = unlink_pool().await;
    pack_managed_instance(&pool).await;
    sqlx::query(
        "INSERT INTO instance_links (instance_id, link_kind) VALUES ('instance', 'official')",
    )
    .execute(&pool)
    .await
    .expect("insert official link");
    sqlx::query(
        "INSERT INTO resource_pack_unlinks VALUES ('instance', 'set', 0)",
    )
    .execute(&pool)
    .await
    .expect("insert marker");
    let mut tx = pool.begin().await.expect("begin transaction");

    reconcile_pack_unlink_in_transaction("instance", &mut tx)
        .await
        .expect("reconcile the marker");
    tx.commit().await.expect("commit reconcile");

    let pack_entries: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM instance_content_entries
         WHERE ownership_kind = 'pack_managed'",
    )
    .fetch_one(&pool)
    .await
    .expect("count pack entries");
    assert_eq!(pack_entries, 1, "a linked instance keeps its pack content");
    let members: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM instance_pack_members")
            .fetch_one(&pool)
            .await
            .expect("count members");
    assert_eq!(members, 1);
    let revision: i64 = sqlx::query_scalar(
        "SELECT revision FROM instance_content_sets WHERE id = 'set'",
    )
    .fetch_one(&pool)
    .await
    .expect("content set revision");
    assert_eq!(revision, 0);
}
