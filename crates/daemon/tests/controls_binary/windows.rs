use crate::host_binary::{Process, TIMEOUT, authenticate, exchange};
use ads_core::{
    queue::DEFAULT_POLICY,
    vault::{Secret, SecretName, Vault, VaultError},
};
use ads_daemon::events::new_id;
use serde_json::json;
use std::collections::BTreeMap;
use std::process::Command;
use std::sync::Mutex;

#[derive(Default)]
pub(crate) struct MemoryVault(Mutex<BTreeMap<String, Secret>>);

impl Vault for MemoryVault {
    fn store(&self, name: SecretName, secret: &Secret) -> Result<(), VaultError> {
        self.0.lock().map_err(|_| VaultError::Unavailable)?.insert(
            name.as_str().into_owned(),
            Secret::new(secret.expose().to_vec())?,
        );
        Ok(())
    }

    fn read(&self, name: SecretName) -> Result<Option<Secret>, VaultError> {
        self.0
            .lock()
            .map_err(|_| VaultError::Unavailable)?
            .get(name.as_str().as_ref())
            .map(|secret| Secret::new(secret.expose().to_vec()))
            .transpose()
    }

    fn remove(&self, name: SecretName) -> Result<(), VaultError> {
        self.0
            .lock()
            .map_err(|_| VaultError::Unavailable)?
            .remove(name.as_str().as_ref());
        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
pub(crate) async fn daemon_serves_queue_and_approvals() {
    let run = new_id().unwrap();
    let root = crate::temp::directory().join(format!("controls-binary-{run}"));
    let profile = root.join("profile");
    let control = root.join("control");
    let workdir = root.join("workspace");
    for directory in [&profile, &control, &workdir] {
        std::fs::create_dir_all(directory).unwrap();
    }
    let identities =
        ads_private_dir::ControlDir::open_user_owned(&profile.join("ads"), true).unwrap();
    identities
        .publish(
            "local-identities.toml",
            b"queue_id = 'queue-01'\nowner_id = 'owner-01'\ndevice_id = 'device-01'\n",
        )
        .unwrap();
    drop(identities);

    let host_config = root.join("host.toml");
    let endpoint = format!(r"\\.\pipe\ads-daemon-controls-{run}");
    std::fs::write(&host_config, format!("channel_path = '{endpoint}'\n")).unwrap();
    let config = root.join("daemon.toml");
    let settings = json!({
        "storage": {"audit_database_path": root.join("audit/audit.db")},
        "workdirs": {"workspace": workdir}
    });
    std::fs::write(&config, toml::to_string(&settings).unwrap()).unwrap();

    let _host = Process::host(&host_config);
    let mut daemon = Process::start(
        Command::new(std::env::current_exe().unwrap())
            .arg("--instance-lock")
            .arg(control.join("daemon.lock"))
            .arg("--config")
            .arg(config)
            .arg("--host-config")
            .arg(host_config)
            .env_clear()
            .env("SystemRoot", std::env::var_os("SystemRoot").unwrap())
            .env("TEMP", &root)
            .env("TMP", &root)
            .env("LOCALAPPDATA", profile),
    );
    let launch_url = daemon.read_ready();
    tokio::time::timeout(TIMEOUT, async {
        let mut ws = authenticate(&launch_url).await;
        let queue = json!({"queue_id": "queue-01"});
        let created = exchange(&mut ws, "queue.create", queue.clone()).await;
        let read = exchange(&mut ws, "queue.get", queue).await;
        let listed = exchange(&mut ws, "approval.list", json!({})).await;
        let policy: toml::Table = toml::from_str(DEFAULT_POLICY).unwrap();

        assert!(
            created["result"]["accepted"] == true
                && read["result"]
                    == json!({
                        "queue_id": "queue-01",
                        "state": "stopped",
                        "policy": policy
                    })
                && listed["result"] == json!({"approvals": [], "more": false, "unreadable": 0}),
            "queue.create: {created}\nqueue.get: {read}\napproval.list: {listed}",
        );
    })
    .await
    .expect("queue and approval commands did not finish");
    println!("daemon_serves_queue_and_approvals passed");
}
