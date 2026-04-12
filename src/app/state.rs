use std::{collections::HashMap, sync::Arc};

use tokio::sync::Mutex;

use crate::{
    app::metrics::Metrics, authoring::executor::DynExecutor, core::config::Config,
    messaging::media::DynMediaDownloader, storage::repository::Repository,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub repository: Repository,
    pub executor: DynExecutor,
    pub media_downloader: DynMediaDownloader,
    pub metrics: Metrics,
    pub conversation_locks: Arc<Mutex<HashMap<String, Arc<Mutex<()>>>>>,
}

pub async fn conversation_lock(state: &AppState, conversation_id: &str) -> Arc<Mutex<()>> {
    let mut locks = state.conversation_locks.lock().await;
    locks
        .entry(conversation_id.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone()
}
