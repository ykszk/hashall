use anyhow::Result;
use assert_cmd::Command;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::sync::Once;

/// Leaky! but it's only for testing!
/// https://stackoverflow.com/questions/23975391/how-to-convert-a-string-into-a-static-str#answer-30527289
#[cfg(target_os = "windows")]
fn leaky_replace_to_win(s: &str) -> &'static str {
    let replaced = s.replace('/', "\\");
    Box::leak(replaced.into_boxed_str())
}

#[cfg(target_os = "windows")]
fn leaky_replace(s: &str, from: &str, to: &str) -> &'static str {
    let replaced = s.replace(from, to);
    Box::leak(replaced.into_boxed_str())
}

static INIT: Once = Once::new();
fn setup() {
    INIT.call_once(|| {
        env_logger::init();

        #[cfg(target_os = "windows")]
        {
            unsafe {
                OUT_FILE = leaky_replace_to_win(OUT_FILE);
                OUT_HIDFILE = leaky_replace_to_win(OUT_HIDFILE);
                OUT_DIR_FILE = leaky_replace_to_win(OUT_DIR_FILE);
                OUT_HIDDIR_FILE = leaky_replace_to_win(OUT_HIDDIR_FILE);
                OUT_ARC_ZIP = leaky_replace_to_win(OUT_ARC_ZIP);
                OUT_ARC_TAR = leaky_replace_to_win(OUT_ARC_TAR);
                OUT_ARC_TAR_GZ = leaky_replace_to_win(OUT_ARC_TAR_GZ);
                OUT_ARC_TAR_ZST = leaky_replace_to_win(OUT_ARC_TAR_ZST);
                OUT_ARC_TAR_BZ2 = leaky_replace_to_win(OUT_ARC_TAR_BZ2);
                OUT_ARC_TAR_XZ = leaky_replace_to_win(OUT_ARC_TAR_XZ);

                OUT_ARC_CONTENTS = leaky_replace_to_win(OUT_ARC_CONTENTS);

                // Not replacing in the archive
                OUT_ARC_CONTENTS = leaky_replace(
                    OUT_ARC_CONTENTS,
                    "directory\\file.txt",
                    "directory/file.txt",
                );
            }
        }
    });

    let test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data");
    std::env::set_current_dir(test_dir).unwrap();
}

static mut OUT_FILE: &str = "ac175545a9b0f6da0d5c03f5135563d8  ./file.txt";
static mut OUT_HIDFILE: &str = "28f9f80606380557b3a5034417227add  ./.hidden_file.txt";
static mut OUT_DIR_FILE: &str = "6657b6593444bd9a13d0131d47bef4f5  ./directory/file.txt";
static mut OUT_HIDDIR_FILE: &str = "13685f3b85a79a59e6e6c7aebdf2abd4  ./.hidden/file.txt";
static mut OUT_ARC_ZIP: &str = "96e0b59e98d0afac097caca640ae89a7  ./archive.zip";
static mut OUT_ARC_TAR: &str = "93bd005392ba45a764e048f936745f29  ./archive.tar";
static mut OUT_ARC_TAR_GZ: &str = "697bfc68b92d00748110bfe0003da43e  ./archive.tar.gz";
static mut OUT_ARC_TAR_ZST: &str = "2d091500d5eaf8b02cab3f82aabb85e5  ./archive.tar.zst";
static mut OUT_ARC_TAR_BZ2: &str = "11ead6a83b86a95427fca0f3d4dba0c7  ./archive.tar.bz2";
static mut OUT_ARC_TAR_XZ: &str = "f067faa0bcfbda70e280a85c40d74a4e  ./archive.tar.xz";

//

static mut OUT_ARC_CONTENTS: &str = "\
ac175545a9b0f6da0d5c03f5135563d8  archive.zip/file.txt
6657b6593444bd9a13d0131d47bef4f5  archive.zip/directory/file.txt
28f9f80606380557b3a5034417227add  archive.zip/.hidden_file.txt
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
        unsafe {
            [
                "",
                OUT_ARC_TAR_BZ2,
                OUT_ARC_TAR_ZST,
                OUT_ARC_TAR_GZ,
                OUT_ARC_TAR,
                OUT_ARC_ZIP,
                OUT_FILE,
                OUT_ARC_TAR_XZ,
            ]
        }
        .join("\n")
    );

    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.args([".", "-a"]);
    cmd.assert().success();
    let output = sort_output(cmd.output()?.stdout)?;
    assert_eq!(
        output,
        unsafe {
            [
                "",
                OUT_ARC_TAR_BZ2,
                OUT_HIDFILE,
                OUT_ARC_TAR_ZST,
                OUT_ARC_TAR_GZ,
                OUT_ARC_TAR,
                OUT_ARC_ZIP,
                OUT_FILE,
                OUT_ARC_TAR_XZ,
            ]
        }
        .join("\n")
    );

    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.args([".", "-ar"]);
    cmd.assert().success();
    let output = sort_output(cmd.output()?.stdout)?;
    assert_eq!(
        output,
        unsafe {
            [
                "",
                OUT_ARC_TAR_BZ2,
                OUT_HIDDIR_FILE,
                OUT_HIDFILE,
                OUT_ARC_TAR_ZST,
                OUT_DIR_FILE,
                OUT_ARC_TAR_GZ,
                OUT_ARC_TAR,
                OUT_ARC_ZIP,
                OUT_FILE,
                OUT_ARC_TAR_XZ,
            ]
        }
        .join("\n")
    );
    Ok(())
}

#[test]
fn test_zip() -> Result<()> {
    setup();
    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.args(["archive.zip", "--archive"]);
    // no sorting necessary because the order of the output in archive file is guaranteed
    unsafe {
        cmd.assert().success().stdout(OUT_ARC_CONTENTS);
    }
    Ok(())
}

#[test]
fn test_tar() -> Result<()> {
    setup();
    let tar_contents = unsafe { OUT_ARC_CONTENTS.replace(".zip", ".tar") };

    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.args(["archive.tar", "--archive"]);
    cmd.assert().success().stdout(tar_contents);

    test_tar_compress(".tar.gz")
}

fn test_tar_compress(extension: &str) -> Result<()> {
    setup();
    let contents = unsafe { OUT_ARC_CONTENTS.replace(".zip", extension) };
    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.args(["archive".to_owned() + extension, "--archive".to_owned()]);
    cmd.assert().success().stdout(contents);
    Ok(())
}

#[test]
fn test_zst() -> Result<()> {
    test_tar_compress(".tar.zst")
}

#[test]
fn test_bz2() -> Result<()> {
    test_tar_compress(".tar.bz2")
}

#[test]
fn test_xz() -> Result<()> {
    test_tar_compress(".tar.xz")
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

fn test_archive(algorithm: &str, hash: &str) -> Result<()> {
    let mut cmd = Command::cargo_bin("hashall").unwrap();
    cmd.args(["archive.zip", "--hash", algorithm]);
    cmd.assert()
        .success()
        .stdout(format!("{hash}  archive.zip\n"));
    Ok(())
}

#[test]
fn test_md5_sha1() -> Result<()> {
    setup();
    let md5 = ("md5", "96e0b59e98d0afac097caca640ae89a7");
    let sha1 = ("sha1", "029dd5579848f55c1361e8698771aac0c7967550");

    for (algorithm, hash) in [md5, sha1] {
        test_archive(algorithm, hash)?;
    }
    Ok(())
}

#[test]
fn test_sha2() -> Result<()> {
    setup();
    let sha224 = (
        "sha224",
        "7aceeca4574744e7bfa93c15831fd47e90fe95036f2b4b7feb7da8e0",
    );
    let sha256 = (
        "sha256",
        "4a4a57fb59aa108c670daf89ca0b2415cd2978b7f4dd608a3a047fbc6fb3462d",
    );
    let sha384 = (
        "sha384",
        "c4afe37730196f64ff6551c77fb9d92c9525519e82d46c134b9ff82081fef37efa8dd7d3576bac2a189fee75351b28bd",
    );
    let sha512_224 = (
        "sha512/224",
        "e9f63c5dd494a95525c3aa743078e0d132f8595988ccc2d5f374addc",
    );
    let sha512_256 = (
        "sha512/256",
        "e344e5d27cdf7597048b328a2e119f9bfe4974e946c8cdd9cdb6b881ef37b94f",
    );
    let sha512 = (
        "sha512",
        "411c6ea9093ea81e5b02e4ee638c4e339024b110fa59c617410c3c150d87698cc1f2abde724007c00e5af162e81c49fa98931d7db70a97d1d51cbde2b876b34c",
    );

    for (algorithm, hash) in [sha224, sha256, sha384, sha512_224, sha512_256, sha512] {
        test_archive(algorithm, hash)?;
    }
    Ok(())
}

#[test]
fn test_sha3() -> Result<()> {
    setup();
    let sha3_224 = (
        "sha3-224",
        "e9493c8438aa0d6f705b35dbd33bef28519ae2defcfb996b54167564",
    );
    let sha3_256 = (
        "sha3-256",
        "68205fb02cf950c74989ca514456c8859cbab54e82ca57d79caef29a1b8c20f7",
    );
    let sha3_384 = (
        "sha3-384",
        "27160bd7ba76dd0c890e26d7c894c0b0e5abfb7c7fe7814854048dd3413ec27581a49adb6327aa0805831fc7d89508d8",
    );
    let sha3_512 = (
        "sha3-512",
        "b40494126d2940e4a7b38da1833d8e02f63184f353042f80a056c64aee4f1d1df5f7c30e677150534e2aec0cba5d1107cd12f0f9a5755104fd6e1b10d0a5d8c4",
    );
    for (algorithm, hash) in [sha3_224, sha3_256, sha3_384, sha3_512] {
        println!("algorithm: {algorithm}");
        test_archive(algorithm, hash)?;
    }
    Ok(())
}
