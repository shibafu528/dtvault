use dtvault_types::shibafu528::dtvault::encoder::encode_video_request::Part as EncodeVideoRequestPart;
use dtvault_types::shibafu528::dtvault::encoder::encode_video_response::{
    Datagram as EncodeVideoResponseDatagram, Part as EncodeVideoResponsePart,
};
use dtvault_types::shibafu528::dtvault::encoder::encoder_service_server::EncoderService as EncoderServiceTrait;
use dtvault_types::shibafu528::dtvault::encoder::{EncodeVideoRequest, EncodeVideoResponse};
use std::pin::Pin;
use tokio::prelude::*;
use tokio::stream::{Stream, StreamExt};
use tonic::{Request, Response, Status};

pub struct EncoderService {}

impl EncoderService {
    pub fn new() -> Self {
        EncoderService {}
    }
}

#[tonic::async_trait]
impl EncoderServiceTrait for EncoderService {
    type EncodeVideoStream = Pin<Box<dyn Stream<Item = Result<EncodeVideoResponse, Status>> + Send + Sync + 'static>>;

    async fn encode_video(
        &self,
        request: Request<tonic::Streaming<EncodeVideoRequest>>,
    ) -> Result<Response<Self::EncodeVideoStream>, Status> {
        unimplemented!()
    }
}
