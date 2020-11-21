mod program_key;
mod program_store;
mod validator;

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
                Some(sp) => Ok(Response::new(GetProgramResponse {
                    program: Some(sp.program().clone()),
                })),
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
            programs: programs.iter().map(|sp| sp.program().clone()).collect(),
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
                program: Some(sp.program().clone()),
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
}
