use headless_pi_player::file_manager::FilesManager;

#[tokio::main]
async fn main() {
    let _file_manager = FilesManager::init().await.expect("File manager should work");
}