use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use remoteopsx_lib::host_identity;
use remoteopsx_lib::jump_host::JumpHostConfig;
use remoteopsx_lib::models::{Server, Tunnel};
use remoteopsx_lib::sftp_manager;
use remoteopsx_lib::ssh_manager;
use remoteopsx_lib::tunnel_manager::TunnelManager;

struct Fixture {
    host: String,
    user: String,
    target_port: u16,
    jump_port: u16,
    private_key: String,
    alt_host_public_key: PathBuf,
    temp: PathBuf,
    password: String,
}

fn fixture() -> Option<Fixture> {
    let get = |key: &str| env::var(key).ok();
    Some(Fixture {
        host: get("REMOTEOPSX_TEST_HOST")?,
        user: get("REMOTEOPSX_TEST_USER")?,
        target_port: get("REMOTEOPSX_TEST_TARGET_PORT")?.parse().ok()?,
        jump_port: get("REMOTEOPSX_TEST_JUMP_PORT")?.parse().ok()?,
        private_key: get("REMOTEOPSX_TEST_PRIVATE_KEY")?,
        alt_host_public_key: PathBuf::from(get("REMOTEOPSX_TEST_ALT_HOST_PUBLIC_KEY")?),
        temp: PathBuf::from(get("REMOTEOPSX_TEST_TEMP")?),
        password: get("REMOTEOPSX_INTEGRATION_PASSWORD")?,
    })
}

fn server(f: &Fixture, auth_type: &str) -> Server {
    Server {
        id: format!("fixture-{auth_type}"),
        name: format!("fixture-{auth_type}"),
        host: f.host.clone(),
        port: f.target_port,
        ftp_port: None,
        rdp_port: None,
        vnc_port: None,
        username: f.user.clone(),
        protocols: vec!["ssh".into(), "sftp".into()],
        auth_type: auth_type.into(),
        private_key_path: (auth_type == "key").then(|| f.private_key.clone()),
        tags: vec!["fixture".into()],
        group_name: None,
        environment: "dev".into(),
        notes: None,
        created_at: String::new(),
        updated_at: String::new(),
    }
}

fn jump(f: &Fixture, server_id: &str) -> JumpHostConfig {
    JumpHostConfig {
        server_id: server_id.into(),
        host: f.host.clone(),
        port: f.jump_port,
        username: f.user.clone(),
        private_key_path: f.private_key.clone(),
    }
}

fn trust_direct(host: &str, port: u16) {
    let report = host_identity::inspect(host, port).expect("scan direct host");
    assert_eq!(report.status, "unseen");
    let fingerprint = report
        .candidates
        .first()
        .expect("candidate")
        .fingerprint
        .clone();
    let trusted = host_identity::trust(host, port, &fingerprint, false).expect("trust direct host");
    assert_eq!(trusted.status, "trusted");
}

fn run_pty(server: &Server, jump: Option<&JumpHostConfig>) -> String {
    let (program, args) = ssh_manager::interactive_argv_via(server, jump).expect("pty argv");
    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("open pty");
    let mut command = CommandBuilder::new(program);
    for arg in args {
        command.arg(arg);
    }
    command.env("TERM", "xterm-256color");
    let mut child = pair.slave.spawn_command(command).expect("spawn ssh pty");
    drop(pair.slave);
    let mut reader = pair.master.try_clone_reader().expect("clone pty reader");
    let mut writer = pair.master.take_writer().expect("pty writer");
    let read_thread = thread::spawn(move || {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).expect("read pty");
        String::from_utf8_lossy(&bytes).to_string()
    });
    writer
        .write_all(b"printf '__REMOTEOPSX_PTY_OK__\\n'; tty; exit\n")
        .expect("write pty command");
    writer.flush().expect("flush pty");
    let _ = child.wait().expect("wait pty");
    drop(writer);
    let output = read_thread.join().expect("join reader");
    assert!(
        output.contains("__REMOTEOPSX_PTY_OK__"),
        "PTY output: {output}"
    );
    assert!(
        output.contains("/dev/pts/") || output.contains("/dev/tty"),
        "no remote tty: {output}"
    );
    output
}

fn available_port() -> u16 {
    let listener = TcpListener::bind(("127.0.0.1", 0)).expect("ephemeral port");
    listener.local_addr().unwrap().port()
}

fn assert_tunnel(server: &Server, jump: Option<&JumpHostConfig>) {
    let manager = TunnelManager::new();
    let local_port = available_port();
    let tunnel = Tunnel {
        id: format!("fixture-tunnel-{local_port}"),
        server_id: server.id.clone(),
        r#type: "local".into(),
        local_host: Some("127.0.0.1".into()),
        local_port,
        remote_host: Some("127.0.0.1".into()),
        remote_port: Some(server.port),
        status: "pending".into(),
        created_at: String::new(),
    };
    manager
        .start_via(server, jump, &tunnel)
        .expect("start tunnel");
    let mut stream = (0..30)
        .find_map(|_| {
            let result = TcpStream::connect(("127.0.0.1", local_port)).ok();
            if result.is_none() {
                thread::sleep(Duration::from_millis(50));
            }
            result
        })
        .expect("connect through local tunnel");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut banner = [0u8; 64];
    let count = stream.read(&mut banner).expect("read ssh banner");
    assert!(String::from_utf8_lossy(&banner[..count]).starts_with("SSH-"));
    manager.stop(&tunnel.id).expect("stop tunnel");
}

fn replace_target_known_host_with_alt(known_hosts: &Path, f: &Fixture) {
    let public = fs::read_to_string(&f.alt_host_public_key).expect("alt public key");
    let mut columns = public.split_whitespace();
    let key_type = columns.next().expect("alt key type");
    let key = columns.next().expect("alt key");
    let target = format!("[{}]:{}", f.host, f.target_port);
    let current = fs::read_to_string(known_hosts).expect("read known_hosts");
    let replaced = current
        .lines()
        .map(|line| {
            if line.split_whitespace().next() == Some(target.as_str()) {
                format!("{target} {key_type} {key}")
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    fs::write(known_hosts, format!("{replaced}\n")).expect("write mismatched known_hosts");
}

#[test]
fn live_ssh_transport_matrix() {
    let Some(f) = fixture() else {
        eprintln!("live SSH fixture env is absent; integration test skipped outside CI fixture");
        return;
    };

    let known_hosts = f.temp.join("remoteopsx-known-hosts");
    host_identity::init(known_hosts.clone()).expect("initialize known_hosts");
    let key_server = server(&f, "key");
    let password_server = server(&f, "password");

    trust_direct(&f.host, f.target_port);

    let key_exec =
        ssh_manager::run_remote(&key_server, "printf '__REMOTEOPSX_KEY_OK__'").expect("key exec");
    assert!(key_exec.success && key_exec.stdout.contains("__REMOTEOPSX_KEY_OK__"));

    let password_exec =
        ssh_manager::run_remote(&password_server, "printf '__REMOTEOPSX_PASSWORD_OK__'")
            .expect("password exec");
    assert!(password_exec.success && password_exec.stdout.contains("__REMOTEOPSX_PASSWORD_OK__"));
    let redacted = ssh_manager::run_remote(&password_server, &format!("printf '{}'", f.password))
        .expect("secret canary exec");
    assert!(!redacted.stdout.contains(&f.password));
    assert!(redacted.stdout.contains("••••••"));
    let (password_program, password_args) =
        ssh_manager::interactive_argv_via(&password_server, None).expect("password argv");
    assert_eq!(password_program, "sshpass");
    assert!(password_args.iter().all(|arg| !arg.contains(&f.password)));

    run_pty(&key_server, None);

    trust_direct(&f.host, f.jump_port);
    let jump = jump(&f, &key_server.id);
    host_identity::remove(&f.host, f.target_port).expect("remove direct target trust");
    let routed_report = host_identity::inspect_via_jump(&f.host, f.target_port, &jump)
        .expect("scan target through jump");
    assert_eq!(routed_report.status, "unseen");
    let target_fingerprint = routed_report.candidates[0].fingerprint.clone();
    let routed_trust =
        host_identity::trust_via_jump(&f.host, f.target_port, &jump, &target_fingerprint, false)
            .expect("trust target through jump");
    assert_eq!(routed_trust.status, "trusted");

    let routed_exec =
        ssh_manager::run_remote_via(&key_server, Some(&jump), "printf '__REMOTEOPSX_JUMP_OK__'")
            .expect("jump exec");
    assert!(routed_exec.success && routed_exec.stdout.contains("__REMOTEOPSX_JUMP_OK__"));
    run_pty(&key_server, Some(&jump));

    let remote_dir = format!("/tmp/remoteopsx-ci-{}", uuid::Uuid::new_v4());
    assert!(
        ssh_manager::run_remote_via(
            &key_server,
            Some(&jump),
            &format!("mkdir -p '{remote_dir}'")
        )
        .expect("mkdir remote")
        .success
    );
    let local_source = f.temp.join("upload-source.txt");
    let local_download = f.temp.join("download-copy.txt");
    fs::write(&local_source, b"remoteopsx-scp-payload").unwrap();
    sftp_manager::upload_via(
        &key_server,
        Some(&jump),
        local_source.to_str().unwrap(),
        &remote_dir,
    )
    .expect("scp upload through jump");
    let remote_file = format!("{remote_dir}/upload-source.txt");
    let listing = sftp_manager::list_dir_via(&key_server, Some(&jump), &remote_dir)
        .expect("list through jump");
    assert!(listing
        .iter()
        .any(|entry| entry.name == "upload-source.txt"));
    sftp_manager::download_via(
        &key_server,
        Some(&jump),
        &remote_file,
        local_download.to_str().unwrap(),
    )
    .expect("scp download through jump");
    assert_eq!(
        fs::read(&local_download).unwrap(),
        b"remoteopsx-scp-payload"
    );
    sftp_manager::rename_via(
        &key_server,
        Some(&jump),
        &remote_file,
        &format!("{remote_dir}/renamed.txt"),
    )
    .expect("rename through jump");
    sftp_manager::delete_via(&key_server, Some(&jump), &remote_dir).expect("delete through jump");

    assert_tunnel(&key_server, Some(&jump));

    replace_target_known_host_with_alt(&known_hosts, &f);
    let changed = host_identity::inspect_via_jump(&f.host, f.target_port, &jump)
        .expect("detect changed target identity");
    assert_eq!(changed.status, "changed");
    let blocked = ssh_manager::run_remote_via(&key_server, Some(&jump), "true")
        .expect("ssh process should report host-key failure");
    assert!(!blocked.success);
    assert!(
        blocked.stderr.to_lowercase().contains("host key")
            || blocked.stderr.to_lowercase().contains("identification")
    );

    let replacement = changed.candidates[0].fingerprint.clone();
    let restored = host_identity::trust_via_jump(&f.host, f.target_port, &jump, &replacement, true)
        .expect("explicitly replace changed target identity");
    assert_eq!(restored.status, "trusted");
    assert!(
        ssh_manager::run_remote_via(&key_server, Some(&jump), "printf '__REMOTEOPSX_RESTORED__'")
            .expect("exec after replace")
            .success
    );
}
