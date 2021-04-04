mod model;
mod program_key;
mod program_store;
mod prost_convert;
mod validator;

pub use self::model::*;
pub use self::program_key::*;
pub use self::program_store::*;
pub use self::validator::*;
use dtvault_types::shibafu528::dtvault::central::program_service_server::ProgramService as ProgramServiceTrait;
use dtvault_types::shibafu528::dtvault::central::*;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub struct ProgramService {
    store: Arc<ProgramStore>,
}

impl ProgramService {
    pub fn new(store: Arc<ProgramStore>) -> Self {
        ProgramService { store }
    }

    fn assign_thumbnail(&self, sp: Arc<Program>, xp: &mut dtvault_types::shibafu528::dtvault::Program) {
        let videos = match self.store.find_videos(sp.video_ids()) {
            Ok(v) => v,
            _ => return,
        };

        for video in videos {
            let video = match video {
                Some(video) if !video.thumbnail.is_empty() => video,
                _ => continue,
            };

            xp.thumbnail = video.thumbnail.clone();
            xp.thumbnail_mime_type = video
                .thumbnail_mime_type
                .as_ref()
                .map_or_else(|| "", |v| v.essence_str())
                .to_string();
            break;
        }
    }
}

#[tonic::async_trait]
impl ProgramServiceTrait for ProgramService {
    async fn get_program(&self, request: Request<GetProgramRequest>) -> Result<Response<GetProgramResponse>, Status> {
        let msg = request.into_inner();

        let program_id = match msg.program_id {
            Some(program_id) => match validate_program_id(&program_id) {
                Ok(_) => Ok(program_id),
                Err(msg) => Err(Status::invalid_argument(format!("Violation in program_id => {}", msg))),
            },
            None => Err(Status::invalid_argument("Missing value: program_id")),
        }?;

        let program_key = ProgramKey::from_program_id(&program_id);
        match self.store.find(&program_key) {
            Ok(result) => match result {
                Some(sp) => {
                    let mut xp = sp.exchangeable();
                    self.assign_thumbnail(sp.clone(), &mut xp);
                    Ok(Response::new(GetProgramResponse { program: Some(xp) }))
                }
                None => Err(Status::not_found(format!("Program not found (id = {})", program_key))),
            },
            Err(e) => Err(Status::aborted(format!("{}", e))),
        }
    }

    async fn list_programs(
        &self,
        _request: Request<ListProgramsRequest>,
    ) -> Result<Response<ListProgramsResponse>, Status> {
        let programs = self.store.all().map_err(|e| Status::aborted(format!("{}", e)))?;
        let res = ListProgramsResponse {
            programs: programs
                .iter()
                .map(|sp| {
                    let mut xp = sp.exchangeable();
                    self.assign_thumbnail(sp.clone(), &mut xp);
                    xp
                })
                .collect(),
        };
        Ok(Response::new(res))
    }

    async fn create_program(
        &self,
        request: Request<CreateProgramRequest>,
    ) -> Result<Response<CreateProgramResponse>, Status> {
        let msg = request.into_inner();
        let program = match msg.program {
            Some(p) => Ok(p),
            None => Err(Status::invalid_argument("Missing value: program".to_string())),
        }?;

        if program.service_id == 0 {
            return Err(Status::invalid_argument("Invalid value: service_id".to_string()));
        }
        if program.event_id == 0 {
            return Err(Status::invalid_argument("Invalid value: event_id".to_string()));
        }
        if program.start_at == None {
            return Err(Status::invalid_argument("Missing value: start_at".to_string()));
        }
        if program.name.is_empty() {
            return Err(Status::invalid_argument("Invalid value: name".to_string()));
        }
        if let Some(service) = &program.service {
            if let Err(msg) = validate_service(service) {
                return Err(Status::invalid_argument(format!("Violation in service => {}", msg)));
            }
        }

        println!("Accept => {:#?}", program);
        let res = match self.store.find_or_create(program) {
            Ok((sp, notice)) => Ok(CreateProgramResponse {
                status: notice.into(),
                program: Some(sp.exchangeable()),
            }),
            Err(e) => Err(Status::aborted(format!("{}", e))),
        }?;
        Ok(Response::new(res))
    }

    async fn get_program_metadata(
        &self,
        request: Request<GetProgramMetadataRequest>,
    ) -> Result<Response<GetProgramMetadataResponse>, Status> {
        let msg = request.into_inner();

        let program_id = match msg.program_id {
            Some(program_id) => match validate_program_id(&program_id) {
                Ok(_) => Ok(program_id),
                Err(msg) => Err(Status::invalid_argument(format!("Violation in program_id => {}", msg))),
            },
            None => Err(Status::invalid_argument("Missing value: program_id")),
        }?;
        if msg.key.is_empty() {
            return Err(Status::invalid_argument("Invalid value: key"));
        }
        if msg.key.len() > 255 {
            return Err(Status::invalid_argument("String too long: key"));
        }

        let program_key = ProgramKey::from_program_id(&program_id);
        let response = match self.store.find(&program_key) {
            Ok(result) => match result {
                Some(sp) => {
                    let default = "".to_string();
                    let value = sp.metadata().get(&msg.key).unwrap_or(&default);
                    Ok(Response::new(GetProgramMetadataResponse {
                        program_id: Some(program_id),
                        key: msg.key,
                        value: value.to_string(),
                    }))
                }
                None => Err(Status::not_found(format!("Program not found (id = {})", program_key))),
            },
            Err(e) => Err(Status::aborted(format!("{}", e))),
        };
        response
    }

    async fn update_program_metadata(
        &self,
        request: Request<UpdateProgramMetadataRequest>,
    ) -> Result<Response<UpdateProgramMetadataResponse>, Status> {
        let msg = request.into_inner();

        let program_id = match msg.program_id {
            Some(program_id) => match validate_program_id(&program_id) {
                Ok(_) => Ok(program_id),
                Err(msg) => Err(Status::failed_precondition(format!(
                    "Violation in program_id => {}",
                    msg
                ))),
            },
            None => Err(Status::invalid_argument("Missing value: program_id")),
        }?;
        if msg.key.is_empty() {
            return Err(Status::invalid_argument("Invalid value: key"));
        }
        if msg.key.len() > 255 {
            return Err(Status::invalid_argument("String too long: key"));
        }
        if msg.value.len() > 1 * 1024 * 1024 {
            return Err(Status::invalid_argument("String too long: value"));
        }

        let program_key = ProgramKey::from_program_id(&program_id);
        let response = match self.store.update_program_metadata(&program_key, &msg.key, &msg.value) {
            Ok(_) => Ok(Response::new(UpdateProgramMetadataResponse {})),
            Err(e) => Err(Status::aborted(format!("{}", e))),
        };
        response
    }

    async fn list_videos_by_program(
        &self,
        request: Request<ListVideosByProgramRequest>,
    ) -> Result<Response<ListVideosByProgramResponse>, Status> {
        let msg = request.into_inner();

        let program_id = match msg.program_id {
            Some(program_id) => match validate_program_id(&program_id) {
                Ok(_) => Ok(program_id),
                Err(msg) => Err(Status::invalid_argument(format!("Violation in program_id => {}", msg))),
            },
            None => Err(Status::invalid_argument("Missing value: program_id")),
        }?;

        let program_key = ProgramKey::from_program_id(&program_id);
        match self.store.find(&program_key) {
            Ok(Some(sp)) => match self.store.find_videos(sp.video_ids()) {
                Ok(videos) => Ok(Response::new(ListVideosByProgramResponse {
                    videos: videos.into_iter().filter_map(|v| v).map(|v| v.exchangeable()).collect(),
                })),
                Err(e) => Err(Status::aborted(format!("{}", e))),
            },
            Ok(None) => Err(Status::not_found(format!("Program not found (id = {})", program_key))),
            Err(e) => Err(Status::aborted(format!("{}", e))),
        }
    }
}
