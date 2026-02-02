// Optional DockerRepo tests when Docker daemon is available

use homeserver::docker_repo::DockerRepo;

#[tokio::test]
async fn docker_repo_connect_and_list_running() {
    let repo = match DockerRepo::connect() {
        Ok(r) => r,
        Err(_) => return, // Skip when Docker is not available (e.g. CI without Docker)
    };
    let stats = repo.list_running_and_refresh_stats().await;
    // No panic; may be empty if no containers running
    let _ = stats;
}
