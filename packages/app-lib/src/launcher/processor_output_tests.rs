use super::*;
use std::collections::HashMap;

fn processor(
    path: impl Into<String>,
    hash: impl Into<String>,
) -> d::modded::Processor {
    d::modded::Processor {
        jar: "example:processor:1.0".to_string(),
        classpath: Vec::new(),
        args: Vec::new(),
        outputs: Some(HashMap::from([(path.into(), hash.into())])),
        sides: None,
    }
}

fn data(
    path: impl Into<String>,
    hash: impl Into<String>,
) -> HashMap<String, d::modded::SidedDataEntry> {
    HashMap::from([
        (
            "OUTPUT".to_string(),
            d::modded::SidedDataEntry {
                client: path.into(),
                server: String::new(),
            },
        ),
        (
            "HASH".to_string(),
            d::modded::SidedDataEntry {
                client: hash.into(),
                server: String::new(),
            },
        ),
    ])
}

#[test]
fn client_processor_detection_respects_sides() {
    let mut processors = vec![d::modded::Processor {
        jar: "example:processor:1.0".to_string(),
        classpath: Vec::new(),
        args: Vec::new(),
        outputs: None,
        sides: Some(vec!["server".to_string()]),
    }];
    assert!(!has_client_processors(Some(&processors)));

    processors[0].sides = Some(vec!["client".to_string()]);
    assert!(has_client_processors(Some(&processors)));

    processors[0].sides = None;
    assert!(has_client_processors(Some(&processors)));
    assert!(!has_client_processors(None));
}

#[test]
fn detects_download_mojmaps_processor_by_task_argument() {
    let mut processor = d::modded::Processor {
        jar: "net.minecraftforge:installertools:1.4.1".to_string(),
        classpath: Vec::new(),
        args: vec![
            "--task".to_string(),
            "DOWNLOAD_MOJMAPS".to_string(),
            "--output".to_string(),
            "{MOJMAPS}".to_string(),
        ],
        outputs: None,
        sides: None,
    };
    assert!(is_download_mojmaps_processor(&processor));

    processor.args[1] = "MCP_DATA".to_string();
    assert!(!is_download_mojmaps_processor(&processor));
}

#[tokio::test]
async fn valid_processor_output_skips_execution() {
    let directory = tempfile::tempdir().unwrap();
    let libraries = directory.path().join("libraries");
    tokio::fs::create_dir_all(&libraries).await.unwrap();
    let output = libraries.join("output.jar");
    tokio::fs::write(&output, b"valid processor output")
        .await
        .unwrap();
    let hash = sha1_smol::Sha1::from(b"valid processor output").hexdigest();
    let data = data(output.to_string_lossy(), format!("'{hash}'"));

    assert!(
        processor_outputs_are_current(
            &processor("{OUTPUT}", "{HASH}"),
            &libraries,
            &data,
            &[&libraries],
        )
        .await
    );
}

#[tokio::test]
async fn binary_patcher_uses_the_matching_output_sha() {
    let directory = tempfile::tempdir().unwrap();
    let libraries = directory.path().join("libraries");
    tokio::fs::create_dir_all(&libraries).await.unwrap();
    let output = libraries.join("patched.jar");
    tokio::fs::write(&output, b"patched minecraft")
        .await
        .unwrap();
    let hash = sha1_smol::Sha1::from(b"patched minecraft").hexdigest();
    let data = HashMap::from([
        (
            "PATCHED".to_string(),
            d::modded::SidedDataEntry {
                client: output.to_string_lossy().into_owned(),
                server: String::new(),
            },
        ),
        (
            "PATCHED_SHA".to_string(),
            d::modded::SidedDataEntry {
                client: format!("'{hash}'"),
                server: String::new(),
            },
        ),
    ]);
    let processor = d::modded::Processor {
        jar: "net.minecraftforge:binarypatcher:1.1.1".to_string(),
        classpath: Vec::new(),
        args: vec!["--output".to_string(), "{PATCHED}".to_string()],
        outputs: None,
        sides: None,
    };

    assert!(
        processor_outputs_are_current(
            &processor,
            &libraries,
            &data,
            &[&libraries],
        )
        .await
    );
}

#[tokio::test]
async fn missing_processor_output_runs_execution() {
    let directory = tempfile::tempdir().unwrap();
    let libraries = directory.path().join("libraries");
    tokio::fs::create_dir_all(&libraries).await.unwrap();
    let output = libraries.join("missing.jar");
    let data = data(
        output.to_string_lossy(),
        "0000000000000000000000000000000000000000",
    );

    assert!(
        !processor_outputs_are_current(
            &processor("{OUTPUT}", "{HASH}"),
            &libraries,
            &data,
            &[&libraries],
        )
        .await
    );
}

#[tokio::test]
async fn wrong_processor_output_hash_runs_execution() {
    let directory = tempfile::tempdir().unwrap();
    let libraries = directory.path().join("libraries");
    tokio::fs::create_dir_all(&libraries).await.unwrap();
    let output = libraries.join("output.jar");
    tokio::fs::write(&output, b"wrong hash").await.unwrap();
    let data = data(
        output.to_string_lossy(),
        "0000000000000000000000000000000000000000",
    );

    assert!(
        !processor_outputs_are_current(
            &processor("{OUTPUT}", "{HASH}"),
            &libraries,
            &data,
            &[&libraries],
        )
        .await
    );
}

#[tokio::test]
async fn malformed_or_escaped_processor_outputs_run_execution() {
    let directory = tempfile::tempdir().unwrap();
    let libraries = directory.path().join("libraries");
    tokio::fs::create_dir_all(&libraries).await.unwrap();
    let outside = directory.path().join("outside.jar");
    tokio::fs::write(&outside, b"outside").await.unwrap();
    let hash = sha1_smol::Sha1::from(b"outside").hexdigest();
    let escaped_data = data(outside.to_string_lossy(), &hash);

    assert!(
        !processor_outputs_are_current(
            &processor("{OUTPUT}", "{HASH}"),
            &libraries,
            &escaped_data,
            &[&libraries],
        )
        .await
    );
    let malformed_data = data(outside.to_string_lossy(), "not-a-sha1");
    assert!(
        !processor_outputs_are_current(
            &processor("{OUTPUT}", "{HASH}"),
            &libraries,
            &malformed_data,
            &[directory.path()],
        )
        .await
    );
}
