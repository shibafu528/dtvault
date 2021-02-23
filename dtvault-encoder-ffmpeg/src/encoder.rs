use crate::config::Config;
use dtvault_types::shibafu528::dtvault::encoder::encode_video_request::Part as EncodeVideoRequestPart;
use dtvault_types::shibafu528::dtvault::encoder::encode_video_response::{
    Datagram as EncodeVideoResponseDatagram, Part as EncodeVideoResponsePart,
};
use dtvault_types::shibafu528::dtvault::encoder::encoder_service_server::EncoderService as EncoderServiceTrait;
use dtvault_types::shibafu528::dtvault::encoder::{EncodeVideoRequest, EncodeVideoResponse};
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::{BufReader, BufWriter};
use tokio::prelude::*;
use tokio::stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};

pub struct EncoderService {
    config: Arc<Config>,
}

impl EncoderService {
    pub fn new(config: Arc<Config>) -> Self {
        EncoderService { config }
    }
}

#[tonic::async_trait]
impl EncoderServiceTrait for EncoderService {
    type EncodeVideoStream = Pin<Box<dyn Stream<Item = Result<EncodeVideoResponse, Status>> + Send + Sync + 'static>>;

    async fn encode_video(
        &self,
        request: Request<tonic::Streaming<EncodeVideoRequest>>,
    ) -> Result<Response<Self::EncodeVideoStream>, Status> {
        let mut stream = request.into_inner();
        let header = match stream.next().await {
            Some(msg) => {
                let msg = msg?;
                let part = match msg.part {
                    Some(part) => Ok(part),
                    None => Err(Status::invalid_argument("Missing value: part")),
                }?;

                match part {
                    EncodeVideoRequestPart::Header(h) => Ok(h),
                    _ => Err(Status::invalid_argument("Invalid part: need header")),
                }
            }
            None => Err(Status::invalid_argument("Empty stream")),
        }?;

        println!("EncodeVideo {:#?}", header);

        let preset = self.config.presets.first().unwrap();
        let mut cmd = match preset.make_command() {
            Ok(c) => Ok(c),
            Err(e) => Err(Status::internal(format!("Invalid preset command: {}", e))),
        }?;

        let mut child = match cmd.spawn() {
            Ok(c) => Ok(c),
            Err(e) => Err(Status::aborted(format!("Failed to spawn child process: {}", e))),
        }?;

        let stdin = child.stdin.take().unwrap();
        let mut stdin_writer = BufWriter::new(stdin);
        let stdout = child.stdout.take().unwrap();
        let mut stdout_reader = BufReader::new(stdout);

        // TODO: エラー時に両方のタスクを止めないとダメ エラーは1つしか送れない点も考慮が必要?
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let mut receiver_tx = tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = stream.next().await {
                let msg = match msg {
                    Ok(m) => m,
                    Err(e) => {
                        eprintln!("[[Error in receiver task!]] {}", e);
                        return;
                    }
                };
                let part = match msg.part {
                    Some(part) => part,
                    None => {
                        let status = Status::invalid_argument("Missing value: part");
                        if let Err(e) = receiver_tx.send(Err(status)).await {
                            eprintln!("[[Error in receiver task!]] {}", e);
                        }
                        return;
                    }
                };

                let result = match part {
                    EncodeVideoRequestPart::Datagram(data) => match stdin_writer.write_all(&data.payload).await {
                        Ok(_) => Ok(()),
                        Err(e) => Err(Status::internal(format!("IO error: {}", e))),
                    },
                    _ => Err(Status::invalid_argument("Invalid part: need datagram")),
                };
                if let Err(status) = result {
                    if let Err(e) = receiver_tx.send(Err(status)).await {
                        eprintln!("[[Error in receiver task!]] {}", e);
                    }
                    return;
                }
            }
            eprintln!("[[Finish receiver]]");
        });
        let mut sender_tx = tx.clone();
        tokio::spawn(async move {
            let mut sent: usize = 0;
            loop {
                let mut buffer = vec![0; 1024 * 1024];
                match stdout_reader.read(&mut buffer).await {
                    Ok(size) => match size {
                        0 => break,
                        n => {
                            buffer.resize(n, 0);
                            let datagram = EncodeVideoResponseDatagram {
                                offset: sent as u64,
                                payload: buffer,
                            };
                            let datagram_req = EncodeVideoResponse {
                                part: Some(EncodeVideoResponsePart::Datagram(datagram)),
                            };
                            if let Err(e) = sender_tx.send(Ok(datagram_req)).await {
                                eprintln!("[[Error in sender task!]] {}", e);
                                return;
                            };

                            sent += n;
                        }
                    },
                    Err(e) => {
                        eprintln!("[[Error in sender task!]] {}", e);
                        if let Err(e) = sender_tx
                            .send(Err(Status::aborted(format!("Error while reading stream: {}", e))))
                            .await
                        {
                            eprintln!("[[Error in sender task!]] {}", e);
                        }
                        return;
                    }
                }
            }
            eprintln!("ffmpeg stdout reach eof");

            match child.await {
                Ok(exit_status) => {
                    eprintln!("[[Exit status]] {}", exit_status);
                }
                Err(e) => {
                    eprintln!("[[Error in sender task!]] {}", e);
                }
            }
        });

        Ok(Response::new(Box::pin(rx)))
    }
}
