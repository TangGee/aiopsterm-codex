use crate::session_state::ThreadSessionState;
use codex_utils_path::write_atomically;
use serde::Serialize;
use std::path::Path;
use std::path::PathBuf;

const THREAD_INFO_FILE_ENV: &str = "AIOPSTERM_CODEX_THREAD_INFO_FILE";
const THREAD_REASON_ENV: &str = "AIOPSTERM_CODEX_THREAD_REASON";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AiopstermThreadInfo {
    thread_id: String,
    reason: String,
    cwd: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    rollout_path: Option<String>,
}

fn normalized_reason(value: &str) -> &'static str {
    match value.trim() {
        "resume" => "resume",
        "fork" => "fork",
        "switch" => "switch",
        _ => "new",
    }
}

fn persist_thread_info(path: &Path, info: &AiopstermThreadInfo) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut contents = serde_json::to_string(info).map_err(std::io::Error::other)?;
    contents.push('\n');
    write_atomically(path, &contents)
}

pub(crate) fn write_primary_thread_info(
    session: &ThreadSessionState,
    reason_override: Option<&str>,
) {
    let Some(path) = std::env::var_os(THREAD_INFO_FILE_ENV).map(PathBuf::from) else {
        return;
    };
    let env_reason = std::env::var(THREAD_REASON_ENV).unwrap_or_default();
    let info = AiopstermThreadInfo {
        thread_id: session.thread_id.to_string(),
        reason: normalized_reason(reason_override.unwrap_or(&env_reason)).to_string(),
        cwd: session.cwd.as_path().display().to_string(),
        rollout_path: session
            .rollout_path
            .as_ref()
            .map(|rollout_path| rollout_path.display().to_string()),
    };
    if let Err(err) = persist_thread_info(&path, &info) {
        tracing::warn!(path = %path.display(), "failed to persist aiopsterm Codex thread info: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_launch_reasons() {
        assert_eq!(normalized_reason("resume"), "resume");
        assert_eq!(normalized_reason("fork"), "fork");
        assert_eq!(normalized_reason("switch"), "switch");
        assert_eq!(normalized_reason("unexpected"), "new");
    }

    #[test]
    fn writes_atomic_camel_case_payload() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("thread.json");
        persist_thread_info(
            &path,
            &AiopstermThreadInfo {
                thread_id: "019-test-thread".to_string(),
                reason: "resume".to_string(),
                cwd: "/tmp/project".to_string(),
                rollout_path: Some("/tmp/rollout.jsonl".to_string()),
            },
        )
        .expect("write thread info");

        let value: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).expect("read thread info"))
                .expect("parse thread info");
        assert_eq!(value["threadId"], "019-test-thread");
        assert_eq!(value["reason"], "resume");
        assert_eq!(value["cwd"], "/tmp/project");
        assert_eq!(value["rolloutPath"], "/tmp/rollout.jsonl");
    }
}
