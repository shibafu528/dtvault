use crate::config::Config;
use dtvault_types::shibafu528::dtvault::encoder::encode_video_request::Part as EncodeVideoRequestPart;
use dtvault_types::shibafu528::dtvault::encoder::encode_video_response::{
    Datagram as EncodeVideoResponseDatagram, Part as EncodeVideoResponsePart,
};
use dtvault_types::shibafu528::dtvault::encoder::encoder_service_server::EncoderService as EncoderServiceTrait;
use dtvault_types::shibafu528::dtvault::encoder::generate_thumbnail_request::{
    OutputFormat, Part as GenerateThumbnailRequestPart,
};
use dtvault_types::shibafu528::dtvault::encoder::generate_thumbnail_response::{
    Datagram as GenerateThumbnailResponseDatagram, Part as GenerateThumbnailResponsePart,
};
use dtvault_types::shibafu528::dtvault::encoder::{
    EncodeVideoRequest, EncodeVideoResponse, GenerateThumbnailRequest, GenerateThumbnailResponse, ListPresetsRequest,
    ListPresetsResponse,
};
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::Command;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
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
    type EncodeVideoStream = ReceiverStream<Result<EncodeVideoResponse, Status>>;

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

        if header.total_length < 1 {
            return Err(Status::invalid_argument("Invalid value: total_length"));
        }
        if header.preset_id.is_empty() {
            return Err(Status::invalid_argument("Invalid value: preset_id"));
        }
        let preset = match self.config.presets.iter().find(|p| p.id == header.preset_id) {
            Some(p) => p,
            None => {
                return Err(Status::not_found(format!(
                    "Preset not found (id = {})",
                    header.preset_id
                )))
            }
        };

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
        let receiver_tx = tx.clone();
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
        let sender_tx = tx.clone();
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

            match child.wait().await {
                Ok(exit_status) => {
                    eprintln!("[[Exit status]] {}", exit_status);
                }
                Err(e) => {
                    eprintln!("[[Error in sender task!]] {}", e);
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn list_presets(
        &self,
        _request: Request<ListPresetsRequest>,
    ) -> Result<Response<ListPresetsResponse>, Status> {
        let res = ListPresetsResponse {
            presets: self.config.presets.iter().map(|p| p.exchangeable()).collect(),
        };
        Ok(Response::new(res))
    }

    type GenerateThumbnailStream = ReceiverStream<Result<GenerateThumbnailResponse, Status>>;

    async fn generate_thumbnail(
        &self,
        request: Request<tonic::Streaming<GenerateThumbnailRequest>>,
    ) -> Result<Response<Self::GenerateThumbnailStream>, Status> {
        let mut stream = request.into_inner();
        let header = match stream.next().await {
            Some(msg) => {
                let msg = msg?;
                let part = match msg.part {
                    Some(part) => Ok(part),
                    None => Err(Status::invalid_argument("Missing value: part")),
                }?;

                match part {
                    GenerateThumbnailRequestPart::Header(h) => Ok(h),
                    _ => Err(Status::invalid_argument("Invalid part: need header")),
                }
            }
            None => Err(Status::invalid_argument("Empty stream")),
        }?;

        println!("GenerateThumbnail {:#?}", header);

        if header.total_length < 1 {
            return Err(Status::invalid_argument("Invalid value: total_length"));
        }
        let output_format = match OutputFormat::from_i32(header.output_format) {
            Some(OutputFormat::Jpeg) => Ok(OutputFormat::Jpeg),
            _ => Err(Status::invalid_argument("Invalid value: output_format")),
        }?;
        let width = match header.width {
            std::u32::MIN..=0 => 320,
            _ => header.width,
        };
        let height = match header.height {
            std::u32::MIN..=0 => 180,
            _ => header.height,
        };

        let mut cmd = Command::new("ffmpeg");
        cmd.args(&[
            "-i",
            "pipe:0",
            "-ss",
            &header.position.to_string(),
            "-r",
            "10",
            "-vframes",
            "1",
            "-c:v",
            match output_format {
                OutputFormat::Jpeg => "mjpeg",
            },
            "-f",
            "image2",
            "-s",
            &format!("{}x{}", width, height),
            "pipe:1",
        ]);
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());
        cmd.kill_on_drop(true);

        let mut child = match cmd.spawn() {
            Ok(c) => Ok(c),
            Err(e) => Err(Status::aborted(format!("Failed to spawn child process: {}", e))),
        }?;

        let stdin = child.stdin.take().unwrap();
        let mut stdin_writer = BufWriter::new(stdin);
        let stdout = child.stdout.take().unwrap();
        let mut stdout_reader = BufReader::new(stdout);

        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let receiver_tx = tx.clone();
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
                    GenerateThumbnailRequestPart::Datagram(data) => match stdin_writer.write_all(&data.payload).await {
                        Ok(_) => Ok(()),
                        Err(e) => {
                            // TODO: ChildのExitStatusをノンブロックで確認した上でエラーにすべきか判断したいが、tokio 0.3以降に上げないとその手段がない
                            // Err(Status::internal(format!("IO error: {}", e)))
                            eprintln!("[[receiver]] IO error: {}", e);
                            break;
                        }
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
        let sender_tx = tx.clone();
        tokio::spawn(async move {
            let mut sent: usize = 0;
            loop {
                let mut buffer = vec![0; 1024 * 1024];
                match stdout_reader.read(&mut buffer).await {
                    Ok(size) => match size {
                        0 => break,
                        n => {
                            buffer.resize(n, 0);
                            let datagram = GenerateThumbnailResponseDatagram {
                                offset: sent as u64,
                                payload: buffer,
                            };
                            let datagram_req = GenerateThumbnailResponse {
                                part: Some(GenerateThumbnailResponsePart::Datagram(datagram)),
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

            match child.wait().await {
                Ok(exit_status) => {
                    eprintln!("[[Exit status]] {}", exit_status);
                }
                Err(e) => {
                    eprintln!("[[Error in sender task!]] {}", e);
                }
            }

            eprintln!("[[Finish sender]]");
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }
}
