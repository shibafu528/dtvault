mod video_created;

use crate::config::Config;
use crate::event::video_created::handle_video_created;
pub use crate::event::video_created::VideoCreated;
use crate::program::ProgramStore;
use crate::video_storage::IStorage;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;

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

pub fn spawn_event_consumer(ec: EventContext, rx: EventReceiver) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut stream = ReceiverStream::new(rx);
        while let Some(event) = stream.next().await {
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
