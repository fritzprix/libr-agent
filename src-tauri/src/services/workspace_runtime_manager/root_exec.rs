//! Root execution (`runShell` with `asRoot`) in a LibrAgent-managed session container.
//!
//! The container runs as the host user, and the workspace is a host bind mount. A root command
//! can write there as host root: a root-owned setuid file in the workspace would give the host
//! user root on the host. The container is created without `SETFCAP`/`MKNOD` and with
//! `no-new-privileges` (see `container::hardening_run_args`). After each root command,
//! [`RootExecLease::finish`] stops every process that is not the host user and gives the
//! read-write mounts back to the host user: device nodes are deleted, setuid/setgid bits are
//! removed, and files of other owners (or of group 0) are given to the host user.

use std::sync::Arc;

use dashmap::DashMap;
use once_cell::sync::Lazy;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::repositories::SessionMetadata;

use super::docker_cli::{
    current_uid_gid, docker_container_has_label, docker_output_slice, docker_rw_mount_destinations,
    run_docker_status, ROOT_HARDENING_LABEL,
};
use super::error::{RuntimeResult, WorkspaceRuntimeError};
use super::session_config::docker_container_name;
use super::WorkspaceRuntimeManager;

/// Whether tool arguments ask to run as root. Only an absent `asRoot` or exactly `false` means
/// "not root": models also send `"true"`, `1` and similar, and those must not run unapproved.
pub fn as_root_requested(args: &serde_json::Value) -> bool {
    !matches!(
        args.get("asRoot"),
        None | Some(serde_json::Value::Bool(false))
    )
}

/// One root command at a time per container: the cleanup stops every root process.
static ROOT_EXEC_LOCKS: Lazy<DashMap<String, Arc<Mutex<()>>>> = Lazy::new(DashMap::new);

/// Tools that the cleanup runs in the container.
const CLEANUP_TOOLS_CHECK: &str = "command -v find >/dev/null && command -v chown >/dev/null \
     && command -v chmod >/dev/null && command -v rm >/dev/null";

/// Stops every process whose real or effective uid is not `$1`, except PID 1, this shell and
/// its children. Only a root command can start such a process in a managed container.
const STOP_FOREIGN_PROCESSES_SCRIPT: &str = r#"me=$$
for d in /proc/[0-9]*; do
  p=${d#/proc/}
  case "$p" in "$me"|1) continue ;; esac
  ppid=; ruid=; euid=
  while read -r k a b _; do
    case "$k" in
      PPid:) ppid=$a ;;
      Uid:) ruid=$a; euid=$b; break ;;
    esac
  done 2>/dev/null < "$d/status"
  [ -n "$ruid" ] || continue
  [ "$ppid" = "$me" ] && continue
  if [ "$ruid" != "$1" ] || [ "$euid" != "$1" ]; then kill -9 "$p" 2>/dev/null; fi
done
exit 0"#;

/// For each mount in `$3...`: deletes device nodes, removes setuid/setgid bits from files and
/// gives entries of another owner (or of group 0) to `$1:$2`. Does not cross into other mounts.
const RESTORE_MOUNTS_SCRIPT: &str = r#"uid=$1; gid=$2; shift 2
status=0
for m in "$@"; do
  [ -d "$m" ] || continue
  find "$m" -xdev \( -type b -o -type c \) -exec rm -f {} + || status=1
  find "$m" -xdev -type f \( -perm -4000 -o -perm -2000 \) -exec chmod ug-s {} + || status=1
  find "$m" -xdev \( ! -user "$uid" -o -group 0 \) -exec chown -h "$uid:$gid" {} + || status=1
done
exit $status"#;

/// Holds the root-exec lock of a container. Call [`RootExecLease::finish`] when the root
/// command has ended (also after a timeout or a stop): it restores the mounts.
pub struct RootExecLease {
    container: String,
    _guard: OwnedMutexGuard<()>,
}

impl RootExecLease {
    /// Stops processes left by the root command and gives the read-write mounts back to the
    /// host user. Does nothing when the host user is unknown (no Unix host) or is root.
    pub async fn finish(self) -> RuntimeResult<()> {
        let Some((uid, gid)) = host_uid_gid().await else {
            return Ok(());
        };
        if uid == "0" {
            return Ok(());
        }
        let container = self.container.as_str();
        let stop = async {
            let stop_args = stop_foreign_processes_args(container, &uid);
            docker_output_slice(&as_str_args(&stop_args))
                .await
                .map(|_| ())
        };
        let restore = async {
            let mounts = docker_rw_mount_destinations(container).await?;
            if mounts.is_empty() {
                return Ok(());
            }
            let restore_args = restore_mounts_args(container, &uid, &gid, &mounts);
            docker_output_slice(&as_str_args(&restore_args))
                .await
                .map(|_| ())
        };
        stop_then_restore(stop, restore).await
    }
}

/// Runs `restore` also when `stop` fails: a root command can leave root-owned or setuid files in
/// the workspace whether or not its processes could be stopped. A restore error comes first,
/// because it leaves the host exposed.
async fn stop_then_restore(
    stop: impl std::future::Future<Output = RuntimeResult<()>>,
    restore: impl std::future::Future<Output = RuntimeResult<()>>,
) -> RuntimeResult<()> {
    let stop_result = stop.await;
    let restore_result = restore.await;
    match (stop_result, restore_result) {
        (_, Err(error)) => Err(WorkspaceRuntimeError::Io(format!(
            "Could not give the workspace back to the host user after the root command: {error}"
        ))),
        (Err(error), Ok(())) => Err(WorkspaceRuntimeError::Io(format!(
            "Could not stop the processes of the root command: {error}"
        ))),
        (Ok(()), Ok(())) => Ok(()),
    }
}

impl WorkspaceRuntimeManager {
    /// Takes the root-exec lock of the session container. Fails when root is not available in
    /// this session or another root command of the container is still running.
    pub fn begin_root_exec(session: &SessionMetadata) -> RuntimeResult<RootExecLease> {
        if let Some(blocker) = Self::root_exec_blocker(session) {
            return Err(WorkspaceRuntimeError::InvalidConfig(
                blocker.message().to_string(),
            ));
        }
        let container = docker_container_name(session)?;
        let lock = ROOT_EXEC_LOCKS
            .entry(container.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let guard = lock.try_lock_owned().map_err(|_| {
            WorkspaceRuntimeError::InvalidConfig(
                "Another asRoot command of this session is still running. Wait for it to end \
                 (workspace__waitForProcess) or stop it, then run this command again."
                    .to_string(),
            )
        })?;
        Ok(RootExecLease {
            container,
            _guard: guard,
        })
    }
}

/// Checks a running managed container before a root command: it must have the hardening, and
/// the cleanup tools must exist.
pub(super) async fn verify_root_exec_ready(session: &SessionMetadata) -> RuntimeResult<()> {
    let container = docker_container_name(session)?;
    if !docker_container_has_label(&container, ROOT_HARDENING_LABEL).await? {
        return Err(WorkspaceRuntimeError::InvalidConfig(format!(
            "Container '{container}' was created without the protection that asRoot needs. \
             Restart the session so that LibrAgent creates the container again."
        )));
    }
    run_docker_status([
        "exec",
        "-u",
        "0",
        &container,
        "sh",
        "-c",
        CLEANUP_TOOLS_CHECK,
    ])
    .await
    .map_err(|_| {
        WorkspaceRuntimeError::InvalidConfig(format!(
            "asRoot needs find, chown, chmod and rm in container '{container}' to give the \
             workspace back to the host user afterwards. Use an image that has them."
        ))
    })
}

async fn host_uid_gid() -> Option<(String, String)> {
    let value = current_uid_gid().await?;
    let (uid, gid) = value.split_once(':')?;
    Some((uid.to_string(), gid.to_string()))
}

fn as_str_args(args: &[String]) -> Vec<&str> {
    args.iter().map(String::as_str).collect()
}

fn stop_foreign_processes_args(container: &str, uid: &str) -> Vec<String> {
    [
        "exec",
        "-u",
        "0",
        container,
        "sh",
        "-c",
        STOP_FOREIGN_PROCESSES_SCRIPT,
        "sh",
        uid,
    ]
    .iter()
    .map(|arg| arg.to_string())
    .collect()
}

fn restore_mounts_args(container: &str, uid: &str, gid: &str, mounts: &[String]) -> Vec<String> {
    let mut args: Vec<String> = [
        "exec",
        "-u",
        "0",
        container,
        "sh",
        "-c",
        RESTORE_MOUNTS_SCRIPT,
        "sh",
        uid,
        gid,
    ]
    .iter()
    .map(|arg| arg.to_string())
    .collect();
    args.extend(mounts.iter().cloned());
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_an_absent_or_false_as_root_means_not_root() {
        for args in [
            serde_json::json!({ "command": "id" }),
            serde_json::json!({ "command": "id", "asRoot": false }),
        ] {
            assert!(!as_root_requested(&args), "{args}");
        }
        for value in [
            serde_json::json!(true),
            serde_json::json!("true"),
            serde_json::json!("false"),
            serde_json::json!(1),
            serde_json::json!(0),
            serde_json::json!(null),
            serde_json::json!({}),
        ] {
            let args = serde_json::json!({ "command": "id", "asRoot": value });
            assert!(as_root_requested(&args), "{args}");
        }
    }

    #[test]
    fn cleanup_runs_as_root_and_passes_ids_and_mounts_as_arguments() {
        let stop = stop_foreign_processes_args("c1", "1000");
        assert_eq!(stop[..4], ["exec", "-u", "0", "c1"]);
        assert_eq!(stop[stop.len() - 2..], ["sh", "1000"]);

        let mounts = vec!["/work".to_string(), "/work/.libragent/teamwork".to_string()];
        let restore = restore_mounts_args("c1", "1000", "1000", &mounts);
        assert_eq!(restore[..4], ["exec", "-u", "0", "c1"]);
        assert_eq!(
            restore[restore.len() - 5..],
            ["sh", "1000", "1000", "/work", "/work/.libragent/teamwork"]
        );
    }

    #[test]
    fn restore_removes_setuid_device_nodes_and_foreign_owners() {
        assert!(RESTORE_MOUNTS_SCRIPT.contains("-perm -4000 -o -perm -2000"));
        assert!(RESTORE_MOUNTS_SCRIPT.contains("chmod ug-s"));
        assert!(RESTORE_MOUNTS_SCRIPT.contains("-type b -o -type c"));
        assert!(RESTORE_MOUNTS_SCRIPT.contains(r#"! -user "$uid" -o -group 0"#));
        assert!(RESTORE_MOUNTS_SCRIPT.contains("-xdev"));
    }

    #[test]
    fn a_second_root_command_of_the_same_container_is_refused_while_one_runs() {
        let mut session = SessionMetadata::test_fixture("lease-s1");
        session.workspace_isolation =
            crate::models::workspace_isolation::WorkspaceIsolationMode::Docker;
        session.docker_config =
            Some(crate::models::workspace_isolation::DockerWorkspaceConfig::default());
        let first = WorkspaceRuntimeManager::begin_root_exec(&session).expect("first lease");
        let second = WorkspaceRuntimeManager::begin_root_exec(&session);
        assert!(matches!(
            second,
            Err(WorkspaceRuntimeError::InvalidConfig(_))
        ));
        drop(first);
        assert!(WorkspaceRuntimeManager::begin_root_exec(&session).is_ok());
    }

    #[tokio::test]
    async fn restore_runs_after_a_failed_stop() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let restored = AtomicBool::new(false);
        let result = stop_then_restore(
            async { Err(WorkspaceRuntimeError::Io("stop failed".to_string())) },
            async {
                restored.store(true, Ordering::SeqCst);
                Ok(())
            },
        )
        .await;
        assert!(
            restored.load(Ordering::SeqCst),
            "restore must run after a failed stop"
        );
        let error = result
            .expect_err("a failed stop is still reported")
            .to_string();
        assert!(error.contains("stop the processes"), "{error}");
        assert!(error.contains("stop failed"), "{error}");
    }

    #[tokio::test]
    async fn a_restore_error_is_reported_before_a_stop_error() {
        let error = stop_then_restore(
            async { Err(WorkspaceRuntimeError::Io("stop failed".to_string())) },
            async { Err(WorkspaceRuntimeError::Io("chown failed".to_string())) },
        )
        .await
        .expect_err("restore failed")
        .to_string();
        assert!(error.contains("give the workspace back"), "{error}");
        assert!(error.contains("chown failed"), "{error}");
    }

    #[tokio::test]
    async fn stop_and_restore_success_is_ok() {
        assert!(stop_then_restore(async { Ok(()) }, async { Ok(()) })
            .await
            .is_ok());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn restore_script_gives_the_mount_back_and_removes_setuid_bits() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        let mount = tempfile::tempdir().expect("mount");
        let file = mount.path().join("tool");
        std::fs::write(&file, "#!/bin/sh\n").expect("write");
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o4755)).expect("chmod");

        let uid = std::fs::metadata(mount.path())
            .expect("meta")
            .uid()
            .to_string();
        let gid = std::fs::metadata(mount.path())
            .expect("meta")
            .gid()
            .to_string();
        let output = std::process::Command::new("sh")
            .args(["-c", RESTORE_MOUNTS_SCRIPT, "sh", &uid, &gid])
            .arg(mount.path())
            .output()
            .expect("run restore script");
        assert!(output.status.success(), "{output:?}");

        let mode = std::fs::metadata(&file).expect("meta").permissions().mode();
        assert_eq!(mode & 0o6000, 0, "setuid/setgid bits must be removed");
        assert_eq!(mode & 0o777, 0o755);
    }
}
