use anyhow::Result;
use assert_cmd::Command;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::sync::Once;

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| {
        env_logger::init();
    });

    let test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    std::env::set_current_dir(test_dir).unwrap();
}

const OUT_FILE: &str = "ac175545a9b0f6da0d5c03f5135563d8  ./file.txt";
const OUT_HIDFILE: &str = "28f9f80606380557b3a5034417227add  ./.hidden_file.txt";
const OUT_DIR_FILE: &str = "6657b6593444bd9a13d0131d47bef4f5  ./directory/file.txt";
const OUT_HIDDIR_FILE: &str = "13685f3b85a79a59e6e6c7aebdf2abd4  ./.hidden/file.txt";
const OUT_ARC_ZIP: &str = "96e0b59e98d0afac097caca640ae89a7  ./archive.zip";
const OUT_ARC_TAR: &str = "93bd005392ba45a764e048f936745f29  ./archive.tar";
const OUT_ARC_TAR_GZ: &str = "697bfc68b92d00748110bfe0003da43e  ./archive.tar.gz";

const OUT_ARC_CONTENTS: &str = "\
ac175545a9b0f6da0d5c03f5135563d8  ./archive.zip/file.txt
6657b6593444bd9a13d0131d47bef4f5  ./archive.zip/directory/file.txt
28f9f80606380557b3a5034417227add  ./archive.zip/.hidden_file.txt
";

fn sort_output(output: Vec<u8>) -> Result<String> {
    let output = String::from_utf8(output)?;
    let mut output: Vec<_> = output.split('\n').to_owned().collect();
    output.sort();
    Ok(output.join("\n"))
}

#[test]
fn test_files() -> Result<()> {
    setup();
    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.arg(".");
    cmd.assert().success();
    // sorting is necessary because the order of the output is not guaranteed
    let output = sort_output(cmd.output()?.stdout)?;
    assert_eq!(
        output,
        ["", OUT_ARC_TAR_GZ, OUT_ARC_TAR, OUT_ARC_ZIP, OUT_FILE].join("\n")
    );

    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.args([".", "-a"]);
    cmd.assert().success();
    let output = sort_output(cmd.output()?.stdout)?;
    assert_eq!(
        output,
        [
            "",
            OUT_HIDFILE,
            OUT_ARC_TAR_GZ,
            OUT_ARC_TAR,
            OUT_ARC_ZIP,
            OUT_FILE
        ]
        .join("\n")
    );

    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.args([".", "-ar"]);
    cmd.assert().success();
    let output = sort_output(cmd.output()?.stdout)?;
    assert_eq!(
        output,
        [
            "",
            OUT_HIDDIR_FILE,
            OUT_HIDFILE,
            OUT_DIR_FILE,
            OUT_ARC_TAR_GZ,
            OUT_ARC_TAR,
            OUT_ARC_ZIP,
            OUT_FILE
        ]
        .join("\n")
    );
    Ok(())
}

#[test]
fn test_zip() -> Result<()> {
    setup();
    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.args(["./archive.zip", "--archive"]);
    // no sorting necessary because the order of the output in archive file is guaranteed
    cmd.assert().success().stdout(OUT_ARC_CONTENTS);
    Ok(())
}

#[test]
fn test_tar() -> Result<()> {
    setup();
    let tar_contents = OUT_ARC_CONTENTS.replace(".zip", ".tar");

    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.args(["./archive.tar", "--archive"]);
    cmd.assert().success().stdout(tar_contents);

    let tar_gz_contents = OUT_ARC_CONTENTS.replace(".zip", ".tar.gz");
    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.args(["./archive.tar.gz", "--archive"]);
    cmd.assert().success().stdout(tar_gz_contents);

    Ok(())
}

#[test]
fn test_failure() -> Result<()> {
    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.assert().failure();

    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.arg("nonexistent");
    cmd.assert().failure();
    Ok(())
}
