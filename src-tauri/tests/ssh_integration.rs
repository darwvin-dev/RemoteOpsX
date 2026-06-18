//! End-to-end SSH integration test against a throwaway container.
//!
//! This closes the gap that unit tests can't: it proves the *live* remote-ops
//! path actually works — real SSH exec, real agentless health collection, and
//! real runbook step execution — not just that the code compiles.
//!
//! It is marked `#[ignore]` so the normal `cargo test` (and the default CI job)
//! skip it. Run it explicitly where Docker is available:
//!
//!     cargo test --manifest-path src-tauri/Cargo.toml --test ssh_integration -- --ignored --nocapture
//!
//! It uses **key-based auth** (an ephemeral ed25519 keypair injected into the
//! container) so it needs neither the OS keyring nor `sshpass`.

use std::process::Command;
use std::time::{Duration, Instant};

use remoteopsx_lib::health_collector::HealthState;
use remoteopsx_lib::models::{RunbookStep, Server};
use remoteopsx_lib::{runbook_runner, ssh_manager};

/// Image used for the SSH target. By default the test builds a minimal sshd
/// image from `tests/fixtures/sshd` (portable — no Docker Hub SSH image
/// needed). Override with REMOTEOPSX_TEST_SSH_IMAGE to use a prebuilt one.
const LOCAL_IMAGE_TAG: &str = "remoteopsx-sshd-test:latest";
const USER: &str = "ops";

/// Removes the container on drop so a failed assertion never leaks it.
struct Container(String);
impl Drop for Container {
    fn drop(&mut self) {
        let _ = Command::new("docker").args(["rm", "-f", &self.0]).output();
    }
}

fn docker_available() -> bool {
    Command::new("docker")
        .arg("info")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run(cmd: &mut Command) -> (bool, String, String) {
    let out = cmd.output().expect("failed to spawn process");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

fn make_server(host: &str, port: u16, key_path: &str) -> Server {
    Server {
        id: "it-server".into(),
        name: "integration".into(),
        host: host.into(),
        port,
        username: USER.into(),
        protocols: vec!["ssh".into()],
        auth_type: "key".into(),
        private_key_path: Some(key_path.into()),
        tags: vec![],
        group_name: None,
        environment: "dev".into(),
        notes: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

#[test]
#[ignore = "requires Docker; run with --ignored"]
fn ssh_health_and_runbook_end_to_end() {
    if !docker_available() {
        eprintln!("SKIP: Docker not available");
        return;
    }

    // 1. Ephemeral keypair in a temp dir.
    let dir = std::env::temp_dir().join(format!("remoteopsx-it-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let key_path = dir.join("id_ed25519");
    let key_str = key_path.to_string_lossy().to_string();
    let (ok, _, err) = run(Command::new("ssh-keygen").args([
        "-t", "ed25519", "-N", "", "-f", &key_str, "-q",
    ]));
    assert!(ok, "ssh-keygen failed: {err}");
    let pubkey = std::fs::read_to_string(format!("{key_str}.pub")).unwrap();

    // 2. Resolve the image: either a caller-provided one or a locally-built
    //    minimal sshd image (portable, no Hub SSH image dependency).
    let image = match std::env::var("REMOTEOPSX_TEST_SSH_IMAGE") {
        Ok(img) if !img.is_empty() => img,
        _ => {
            let fixtures = format!("{}/tests/fixtures/sshd", env!("CARGO_MANIFEST_DIR"));
            eprintln!("building {LOCAL_IMAGE_TAG} from {fixtures} …");
            let (ok, _, err) = run(Command::new("docker").args(["build", "-t", LOCAL_IMAGE_TAG, &fixtures]));
            assert!(ok, "docker build failed: {err}");
            LOCAL_IMAGE_TAG.to_string()
        }
    };

    // 3. Launch the SSH container with the public key injected.
    let (ok, id_out, err) = run(Command::new("docker").args([
        "run", "-d",
        "-p", "127.0.0.1::2222",
        "-e", &format!("PUBLIC_KEY={}", pubkey.trim()),
        "-e", &format!("USER_NAME={USER}"),
        &image,
    ]));
    assert!(ok, "docker run failed: {err}");
    let container = Container(id_out.trim().to_string());

    // 4. Resolve the mapped host port.
    let (ok, port_out, err) = run(Command::new("docker").args(["port", &container.0, "2222"]));
    assert!(ok, "docker port failed: {err}");
    let host_port: u16 = port_out
        .lines()
        .next()
        .and_then(|l| l.rsplit(':').next())
        .and_then(|p| p.trim().parse().ok())
        .unwrap_or_else(|| panic!("could not parse host port from: {port_out:?}"));
    eprintln!("container {} ssh on 127.0.0.1:{host_port}", &container.0[..12]);

    let server = make_server("127.0.0.1", host_port, &key_str);

    // 5. Wait for sshd to accept our key (host-key gen + service start take time).
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut ready = false;
    while Instant::now() < deadline {
        if let Ok(out) = ssh_manager::run_remote(&server, "echo READY") {
            if out.success && out.stdout.contains("READY") {
                ready = true;
                break;
            }
        }
        std::thread::sleep(Duration::from_secs(2));
    }
    assert!(ready, "ssh never became ready within timeout");

    // 6. Agentless health collection. First sample seeds rate counters; the
    //    second yields real CPU%/net deltas.
    let health = HealthState::new();
    let _ = health.collect(&server).expect("first health collect");
    std::thread::sleep(Duration::from_secs(2));
    let snap = health.collect(&server).expect("second health collect");

    assert!(snap.mem_total_kb > 0, "expected MemTotal > 0 from /proc/meminfo");
    assert!(!snap.os_name.is_empty(), "expected an OS name from /etc/os-release");
    assert!(snap.uptime_secs > 0 || snap.mem_used_kb > 0, "expected live proc data");
    eprintln!(
        "health: os='{}' kernel='{}' mem={}kB cpu={:.1}%",
        snap.os_name, snap.kernel, snap.mem_total_kb, snap.cpu_percent
    );

    // 7. Runbook step execution: success + failure classification.
    let ok_step = runbook_runner::run_step(
        &server,
        &RunbookStep {
            name: "echo".into(),
            command: "echo hello-runbook".into(),
            requires_confirmation: false,
            success_pattern: Some("hello-runbook".into()),
            failure_pattern: None,
        },
    );
    assert_eq!(ok_step.status, "success", "stdout: {} stderr: {}", ok_step.stdout, ok_step.stderr);
    assert!(ok_step.stdout.contains("hello-runbook"));

    let fail_step = runbook_runner::run_step(
        &server,
        &RunbookStep {
            name: "false".into(),
            command: "exit 3".into(),
            requires_confirmation: false,
            success_pattern: None,
            failure_pattern: None,
        },
    );
    assert_eq!(fail_step.status, "failure");
    assert_eq!(fail_step.exit_code, 3);

    eprintln!("✓ live SSH exec, health collection and runbook execution all verified");
    // `container` drops here -> docker rm -f
    let _ = std::fs::remove_dir_all(&dir);
}
