use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone, Default)]
pub struct Metrics {
    inbound_messages: Arc<AtomicU64>,
    successful_jobs: Arc<AtomicU64>,
    failed_jobs: Arc<AtomicU64>,
}

impl Metrics {
    pub fn inc_inbound(&self) {
        self.inbound_messages.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_success(&self) {
        self.successful_jobs.fetch_add(1, Ordering::Relaxed);
    }

    pub fn inc_failure(&self) {
        self.failed_jobs.fetch_add(1, Ordering::Relaxed);
    }

    pub fn render(&self) -> String {
        format!(
            "inbound_messages {}\nsuccessful_jobs {}\nfailed_jobs {}\n",
            self.inbound_messages.load(Ordering::Relaxed),
            self.successful_jobs.load(Ordering::Relaxed),
            self.failed_jobs.load(Ordering::Relaxed)
        )
    }
}
