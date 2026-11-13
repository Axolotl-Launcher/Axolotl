use super::*;

fn processor() -> d::modded::Processor {
    d::modded::Processor {
        jar: "net.neoforged.installertools:installertools:4.0.17:fatjar"
            .to_string(),
        classpath: Vec::new(),
        args: [
            "--task",
            "PROCESS_MINECRAFT_JAR",
            "--input",
            "{MINECRAFT_JAR}",
        ]
        .map(String::from)
        .to_vec(),
        outputs: None,
        sides: None,
    }
}

#[test]
fn failure_names_the_processor_task_and_status() {
    let message = processor_failure_message(
        &processor(),
        "exit code: 1",
        b"Task: PROCESS_MINECRAFT_JAR\r\n",
        b"Can only pass one or two --input arguments.\r\n",
    );
    assert_eq!(
        message,
        "Processor error: net.neoforged.installertools:installertools:4.0.17:fatjar PROCESS_MINECRAFT_JAR exited with exit code: 1\n\
             stderr:\n\
             Can only pass one or two --input arguments.\n\
             stdout:\n\
             Task: PROCESS_MINECRAFT_JAR"
    );
}

#[test]
fn jvm_errors_on_stdout_are_kept_when_stderr_is_empty() {
    let message = processor_failure_message(
            &processor(),
            "exit code: 1",
            b"#\n# There is insufficient memory for the Java Runtime Environment to continue.\n",
            b"",
        );
    assert!(!message.contains("stderr:"));
    assert!(message.ends_with(
            "stdout:\n#\n# There is insufficient memory for the Java Runtime Environment to continue."
        ));
}

#[test]
fn silent_failure_says_nothing_was_printed() {
    let message =
        processor_failure_message(&processor(), "exit code: 1", b"", b"\n");
    assert!(
        message
            .ends_with("exited with exit code: 1 without printing any output")
    );
}

#[test]
fn long_output_keeps_both_ends() {
    let stderr = (0..50)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    let message = processor_failure_message(
        &processor(),
        "exit code: 1",
        b"",
        stderr.as_bytes(),
    );
    let lines = message.lines().collect::<Vec<_>>();
    assert_eq!(lines[2], "line 0");
    assert_eq!(lines[21], "line 19");
    assert_eq!(lines[22], "... 10 lines omitted");
    assert_eq!(lines[23], "line 30");
    assert_eq!(lines.last(), Some(&"line 49"));
    assert_eq!(lines.len(), 2 + 20 + 1 + 20);
}

#[test]
fn long_lines_are_cut_at_a_character_boundary() {
    let line = "错".repeat(PROCESSOR_OUTPUT_LINE_CHARS + 1);
    let message = processor_failure_message(
        &processor(),
        "exit code: 1",
        b"",
        line.as_bytes(),
    );
    let kept = message.lines().nth(2).unwrap();
    assert_eq!(
        kept,
        format!("{}...", "错".repeat(PROCESSOR_OUTPUT_LINE_CHARS))
    );
}

#[test]
fn logged_output_drops_the_middle_of_large_streams() {
    assert_eq!(processor_output_for_log(b"small\n"), "small\n");
    let mut output = vec![b'a'; PROCESSOR_OUTPUT_LOG_EDGE_BYTES];
    output.extend(vec![b'b'; 10]);
    output.extend(vec![b'c'; PROCESSOR_OUTPUT_LOG_EDGE_BYTES]);
    let logged = processor_output_for_log(&output);
    assert_eq!(
        logged,
        format!(
            "{}\n... 10 bytes omitted\n{}",
            "a".repeat(PROCESSOR_OUTPUT_LOG_EDGE_BYTES),
            "c".repeat(PROCESSOR_OUTPUT_LOG_EDGE_BYTES)
        )
    );
}

#[test]
fn processors_run_on_the_console_java_executable() {
    assert_eq!(processor_java_executable("java"), PathBuf::from("java"));
    #[cfg(windows)]
    {
        assert_eq!(
            processor_java_executable("javaw.exe"),
            PathBuf::from("javaw.exe"),
            "a bare command may have no console sibling on PATH"
        );
        let dir = tempfile::tempdir().unwrap();
        let windowed = dir.path().join("javaw.exe");
        assert_eq!(
            processor_java_executable(windowed.to_str().unwrap()),
            windowed,
            "a runtime without java.exe keeps the selected executable"
        );
        std::fs::write(dir.path().join("java.exe"), b"").unwrap();
        assert_eq!(
            processor_java_executable(windowed.to_str().unwrap()),
            dir.path().join("java.exe")
        );
    }
}
