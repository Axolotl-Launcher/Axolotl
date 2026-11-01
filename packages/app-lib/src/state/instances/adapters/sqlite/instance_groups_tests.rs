use super::*;
use crate::state::instances::adapters::sqlite::test_support::test_pool;

#[tokio::test]
async fn a_database_without_a_grouping_table_holds_nothing() {
    let pool = test_pool().await;

    assert!(matches!(
        read_legacy(&pool).await.unwrap(),
        LegacyGroups::Absent
    ));
}

#[tokio::test]
async fn the_membership_shape_reads_instance_and_group_name() {
    let pool = test_pool().await;
    sqlx::query(
        "CREATE TABLE instance_groups (
            instance_id TEXT NOT NULL,
            group_name TEXT NOT NULL,
            PRIMARY KEY (instance_id, group_name)
        )",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO instance_groups (instance_id, group_name)
         VALUES ('instance-1', 'Survival'), ('instance-2', 'Survival')",
    )
    .execute(&pool)
    .await
    .unwrap();

    let LegacyGroups::Memberships(rows) = read_legacy(&pool).await.unwrap()
    else {
        panic!("the membership shape was not recognized");
    };
    assert_eq!(
        rows,
        vec![
            ("instance-1".to_string(), "Survival".to_string()),
            ("instance-2".to_string(), "Survival".to_string()),
        ]
    );
}

#[tokio::test]
async fn the_named_shape_reads_its_membership_table_when_it_is_there() {
    let pool = test_pool().await;
    sqlx::raw_sql(
        "
        CREATE TABLE instance_groups (
            id TEXT NOT NULL,
            name TEXT NOT NULL,
            display_order INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (id)
        );
        CREATE TABLE instance_group_memberships (
            instance_id TEXT NOT NULL,
            group_id TEXT NOT NULL
        );
        INSERT INTO instance_groups (id, name) VALUES ('group', 'Survival');
        INSERT INTO instance_group_memberships (instance_id, group_id)
            VALUES ('instance-1', 'group');
        ",
    )
    .execute(&pool)
    .await
    .unwrap();

    let LegacyGroups::Named {
        groups,
        memberships,
    } = read_legacy(&pool).await.unwrap()
    else {
        panic!("the named shape was not recognized");
    };
    assert_eq!(groups, vec![("group".to_string(), "Survival".to_string())]);
    assert_eq!(
        memberships,
        vec![("instance-1".to_string(), "group".to_string())]
    );
}

#[tokio::test]
async fn a_grouping_table_of_another_shape_is_reported_with_its_columns() {
    let pool = test_pool().await;
    sqlx::query("CREATE TABLE instance_groups (something TEXT NOT NULL)")
        .execute(&pool)
        .await
        .unwrap();

    let LegacyGroups::Unrecognised(columns) = read_legacy(&pool).await.unwrap()
    else {
        panic!("an unknown shape was read as a known one");
    };
    assert_eq!(columns, vec!["something".to_string()]);
}
