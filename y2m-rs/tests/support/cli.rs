#![allow(dead_code)]

use std::{
    fs,
    io::{BufRead, BufReader, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, Output, Stdio},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use uuid::Uuid;

pub struct ProcessHandle {
    child: Child,
    stdin: Option<ChildStdin>,
    rx: mpsc::Receiver<String>,
    log: Arc<Mutex<String>>,
}

impl ProcessHandle {
    pub fn spawn(
        program: impl AsRef<Path>,
        args: &[&str],
        current_dir: &Path,
        envs: &[(&str, &str)],
    ) -> anyhow::Result<Self> {
        let mut command = Command::new(program.as_ref());
        command
            .args(args)
            .current_dir(current_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        for (key, value) in envs {
            command.env(key, value);
        }

        let mut child = command.spawn()?;
        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let (tx, rx) = mpsc::channel();
        let log = Arc::new(Mutex::new(String::new()));

        if let Some(stdout) = stdout {
            spawn_reader(stdout, tx.clone(), log.clone());
        }
        if let Some(stderr) = stderr {
            spawn_reader(stderr, tx, log.clone());
        }

        Ok(Self {
            child,
            stdin,
            rx,
            log,
        })
    }

    pub fn write_line(&mut self, line: &str) -> anyhow::Result<()> {
        let Some(stdin) = self.stdin.as_mut() else {
            anyhow::bail!("stdin is not available");
        };
        stdin.write_all(line.as_bytes())?;
        stdin.write_all(b"\n")?;
        stdin.flush()?;
        Ok(())
    }

    pub fn wait_for_contains(&mut self, needle: &str, timeout: Duration) -> anyhow::Result<String> {
        self.wait_for_match(timeout, |line| line.contains(needle).then(|| line.to_string()))
    }

    pub fn wait_for_match<T, F>(&mut self, timeout: Duration, matcher: F) -> anyhow::Result<T>
    where
        F: Fn(&str) -> Option<T>,
    {
        let snapshot = self.output();
        for line in snapshot.lines() {
            if let Some(value) = matcher(line) {
                return Ok(value);
            }
        }

        let deadline = Instant::now() + timeout;
        loop {
            let now = Instant::now();
            if now >= deadline {
                anyhow::bail!("timeout waiting for process output\n{}", self.output());
            }
            let remaining = deadline.saturating_duration_since(now);
            match self.rx.recv_timeout(remaining.min(Duration::from_millis(100))) {
                Ok(line) => {
                    if let Some(value) = matcher(&line) {
                        return Ok(value);
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    anyhow::bail!("process output channel closed\n{}", self.output());
                }
            }
        }
    }

    pub fn output(&self) -> String {
        self.log.lock().expect("lock process log").clone()
    }

    pub fn try_wait(&mut self) -> anyhow::Result<Option<std::process::ExitStatus>> {
        Ok(self.child.try_wait()?)
    }

    pub fn wait(&mut self) -> anyhow::Result<std::process::ExitStatus> {
        Ok(self.child.wait()?)
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        self.kill();
    }
}

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

pub fn y2m_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_y2m"))
}

pub fn run_y2m(
    args: &[&str],
    current_dir: &Path,
    envs: &[(&str, &str)],
) -> anyhow::Result<Output> {
    let mut command = Command::new(y2m_binary());
    command.args(args).current_dir(current_dir);
    for (key, value) in envs {
        command.env(key, value);
    }
    Ok(command.output()?)
}

pub fn run_y2m_checked(
    args: &[&str],
    current_dir: &Path,
    envs: &[(&str, &str)],
) -> anyhow::Result<String> {
    let output = run_y2m(args, current_dir, envs)?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() {
        anyhow::bail!(
            "command failed: y2m {}\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            stdout,
            stderr
        );
    }
    Ok(format!("{stdout}{stderr}"))
}

pub fn spawn_y2m(
    args: &[&str],
    current_dir: &Path,
    envs: &[(&str, &str)],
) -> anyhow::Result<ProcessHandle> {
    ProcessHandle::spawn(y2m_binary(), args, current_dir, envs)
}

pub fn spawn_server_process() -> anyhow::Result<(ProcessHandle, String)> {
    let addr = reserve_local_addr()?;
    let process = spawn_server_process_at(addr)?;
    Ok((process, format!("ws://{addr}/ws")))
}

pub fn spawn_server_process_at(addr: SocketAddr) -> anyhow::Result<ProcessHandle> {
    let addr_text = addr.to_string();
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_string());
    let mut process = ProcessHandle::spawn(
        cargo,
        &["run", "--quiet", "-p", "y2m-server"],
        &workspace_root(),
        &[("Y2M_SERVER_ADDR", &addr_text)],
    )?;
    wait_for_server_ready(addr, Duration::from_secs(30))?;
    if let Some(status) = process.try_wait()? {
        anyhow::bail!(
            "server exited early: {status}\n{}",
            process.output()
        );
    }
    Ok(process)
}

pub fn reserve_server_addr() -> anyhow::Result<SocketAddr> {
    reserve_local_addr()
}

pub fn create_temp_dir(prefix: &str) -> anyhow::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("y2m-cli-e2e-{prefix}-{}", Uuid::new_v4()));
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn init_client_config(
    config_path: &Path,
    server_url: &str,
    group: &str,
    client: &str,
    download_dir: Option<&Path>,
) -> anyhow::Result<()> {
    let config_text = config_path.to_string_lossy().to_string();
    let mut args = vec![
        "init",
        "--config",
        config_text.as_str(),
        "--server-url",
        server_url,
        "--group",
        group,
        "--client",
        client,
    ];

    let download_text;
    if let Some(download_dir) = download_dir {
        download_text = download_dir.to_string_lossy().to_string();
        args.push("--download-dir");
        args.push(download_text.as_str());
    }

    let _ = run_y2m_checked(&args, &workspace_root(), &[])?;
    Ok(())
}

fn reserve_local_addr() -> anyhow::Result<SocketAddr> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;
    drop(listener);
    Ok(addr)
}

fn wait_for_server_ready(addr: SocketAddr, timeout: Duration) -> anyhow::Result<()> {
    let deadline = Instant::now() + timeout;
    loop {
        if TcpStream::connect(addr).is_ok() {
            return Ok(());
        }
        if Instant::now() >= deadline {
            anyhow::bail!("server not ready on {addr}");
        }
        thread::sleep(Duration::from_millis(50));
    }
}

fn spawn_reader<R>(reader: R, tx: mpsc::Sender<String>, log: Arc<Mutex<String>>)
where
    R: std::io::Read + Send + 'static,
{
    thread::spawn(move || {
        let reader = BufReader::new(reader);
        for line in reader.lines() {
            let Ok(line) = line else {
                break;
            };
            {
                let mut text = log.lock().expect("lock process log");
                text.push_str(&line);
                text.push('\n');
            }
            let _ = tx.send(line);
        }
    });
}

#[allow(dead_code)]
fn _keep_types(_: Option<(ChildStdout, ChildStderr)>) {}
