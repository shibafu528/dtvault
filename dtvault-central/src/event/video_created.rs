use crate::event::EventContext;
use crate::program::{ProgramKey, Video};
use crate::video_storage::StorageReader;
use dtvault_types::shibafu528::dtvault::encoder::encoder_service_client::EncoderServiceClient;
use dtvault_types::shibafu528::dtvault::encoder::generate_thumbnail_request::{
    Datagram as RequestDatagram, Header as RequestHeader, OutputFormat, Part as RequestPart,
};
use dtvault_types::shibafu528::dtvault::encoder::generate_thumbnail_response::Part as ResponsePart;
use dtvault_types::shibafu528::dtvault::encoder::GenerateThumbnailRequest;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::AsyncReadExt;
use tokio::stream::StreamExt;
use tokio::sync::mpsc;
use uuid::Uuid;

#[derive(Debug)]
pub struct VideoCreated {
    pub program_key: ProgramKey,
    pub video_id: Uuid,
}

pub async fn handle_video_created(ec: &EventContext, params: VideoCreated) -> Result<(), Box<dyn std::error::Error>> {
    create_video_thumbnail(ec, &params).await?;
    Ok(())
}

async fn create_video_thumbnail(ec: &EventContext, params: &VideoCreated) -> Result<(), Box<dyn std::error::Error>> {
    let video = match ec.program_store.find_video(&params.video_id)? {
        Some(v) => v,
        None => return Ok(()),
    };
    let encoder_url = match ec.config.outlet.encoder_url() {
        Some(e) => e,
        None => return Ok(()),
    };

    let storage_id = video.storage_id;
    let mut storage = None;
    for s in &ec.storages {
        if s.storage_id().await? == storage_id {
            storage = Some(s.clone());
            break;
        }
    }
    let storage = match storage {
        Some(s) => s,
        None => return Ok(()),
    };

    let video_stream = storage.find_bin(&video).await?;

    let mut encoder_service_client = EncoderServiceClient::connect(encoder_url).await?;
    let stream = make_request_stream(video.clone(), video_stream);
    let res = encoder_service_client.generate_thumbnail(stream).await?;
    let mut res_stream = res.into_inner();

    let mut buffer = vec![];
    while let Some(msg) = res_stream.next().await {
        let msg = msg?;
        let part = match msg.part {
            Some(part) => Ok(part),
            None => Err("Missing value: part"),
        }?;

        #[allow(unreachable_patterns)]
        match part {
            ResponsePart::Datagram(mut data) => buffer.append(&mut data.payload),
            _ => Err("Invalid part: need datagram")?,
        }
    }

    let length = buffer.len();

    ec.program_store
        .update_video_thumbnail(&video.id, buffer, mime::IMAGE_JPEG)?;

    println!(
        "[EV:video_created:create_video_thumbnail] Thumbnail created. Video ID = {}, Size = {}",
        &video.id, length
    );

    Ok(())
}

fn make_request_stream(
    video: Arc<Video>,
    mut video_stream: Pin<Box<dyn StorageReader + Send>>,
) -> mpsc::Receiver<GenerateThumbnailRequest> {
    let (mut tx, rx) = mpsc::channel(1);
    tokio::spawn(async move {
        let header = RequestHeader {
            total_length: video.total_length,
            output_format: OutputFormat::Jpeg as i32,
            width: 854,
            height: 480,
            position: 30,
        };
        let header_req = GenerateThumbnailRequest {
            part: Some(RequestPart::Header(header)),
        };
        if let Err(e) = tx.send(header_req).await {
            eprintln!("[[Error in task!]] {}", e);
            return;
        }

        let mut sent: usize = 0;
        loop {
            let mut buffer = vec![0; 1024 * 1024];
            match video_stream.read(&mut buffer).await {
                Ok(size) => match size {
                    0 => break,
                    n => {
                        buffer.resize(n, 0);
                        let datagram = RequestDatagram {
                            offset: sent as u64,
                            payload: buffer,
                        };
                        let datagram_req = GenerateThumbnailRequest {
                            part: Some(RequestPart::Datagram(datagram)),
                        };
                        if let Err(e) = tx.send(datagram_req).await {
                            eprintln!("[[Error in task!]] {}", e);
                            return;
                        };

                        sent += n;
                    }
                },
                Err(e) => {
                    eprintln!("[[Error in task!]] {}", e);
                    return;
                }
            };
        }
    });
    rx
}
