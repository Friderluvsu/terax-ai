pub mod ssh_config;
pub mod target;

use ssh_config::SshHost;

#[tauri::command]
pub async fn ssh_list_hosts() -> Result<Vec<SshHost>, String> {
    tauri::async_runtime::spawn_blocking(ssh_config::list_user_hosts)
        .await
        .map_err(|e| e.to_string())
}
