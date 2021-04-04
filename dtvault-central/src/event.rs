mod video_created;

use crate::config::Config;
use crate::event::video_created::handle_video_created;
pub use crate::event::video_created::VideoCreated;
use crate::program::ProgramStore;
use crate::video_storage::IStorage;
use std::sync::Arc;
use tokio::stream::StreamExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

pub type EventEmitter = mpsc::Sender<Event>;
pub type EventReceiver = mpsc::Receiver<Event>;

pub struct EventContext {
    pub config: Arc<Config>,
    pub program_store: Arc<ProgramStore>,
    pub storages: Vec<Arc<IStorage>>,
}

#[derive(Debug)]
pub enum Event {
    VideoCreated(VideoCreated),
}

pub fn make_event_channel() -> (EventEmitter, EventReceiver) {
    mpsc::channel(16)
}

pub fn spawn_event_consumer(ec: EventContext, mut rx: EventReceiver) -> JoinHandle<()> {
    tokio::spawn(async move {
        while let Some(event) = rx.next().await {
            println!("[EV] {:?}", event);
            let r = match event {
                Event::VideoCreated(params) => handle_video_created(&ec, params).await,
            };
            if let Err(e) = r {
                println!("[EV] error: {}", e);
            }
        }
    })
}
