use crate::program::ProgramKey;
use dtvault_types::shibafu528::dtvault::central::create_program_response::Status as ResponseStatus;
use dtvault_types::shibafu528::dtvault::Program;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::sync::{Arc, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

#[derive(Clone)]
pub struct StoredProgram {
    program: Program,
    metadata: HashMap<String, String>,
}

impl StoredProgram {
    fn new(program: Program) -> Self {
        StoredProgram {
            program,
            metadata: HashMap::new(),
        }
    }

    pub fn program(&self) -> &Program {
        &self.program
    }

    #[allow(dead_code)]
    pub fn metadata(&self) -> &HashMap<String, String> {
        &self.metadata
    }
}

type ProgramStoreBackend = BTreeMap<ProgramKey, Arc<StoredProgram>>;
type StoreReadPoisonError<'a> = PoisonError<RwLockReadGuard<'a, ProgramStoreBackend>>;
type StoreWritePoisonError<'a> = PoisonError<RwLockWriteGuard<'a, ProgramStoreBackend>>;

pub enum MetadataWriteError<'a> {
    ProgramNotFound(&'a ProgramKey),
    PoisonError(StoreWritePoisonError<'a>),
}

impl<'a> From<StoreWritePoisonError<'a>> for MetadataWriteError<'a> {
    fn from(err: StoreWritePoisonError<'a>) -> Self {
        Self::PoisonError(err)
    }
}

impl fmt::Display for MetadataWriteError<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProgramNotFound(key) => write!(f, "Program not found (id = {})", key),
            Self::PoisonError(e) => write!(f, "{}", e),
        }
    }
}

pub enum FindOrCreateNotice {
    Created,
    AlreadyExists,
}

impl Into<i32> for FindOrCreateNotice {
    fn into(self) -> i32 {
        match self {
            Self::Created => ResponseStatus::Created as i32,
            Self::AlreadyExists => ResponseStatus::AlreadyExists as i32,
        }
    }
}

pub struct ProgramStore {
    store: RwLock<ProgramStoreBackend>,
}

impl ProgramStore {
    pub fn new() -> Self {
        ProgramStore {
            store: RwLock::new(ProgramStoreBackend::new()),
        }
    }

    pub fn all(&self) -> Result<Vec<Arc<StoredProgram>>, StoreReadPoisonError> {
        let store = self.store.read()?;
        Ok(store.values().cloned().collect())
    }

    pub fn find(&self, key: &ProgramKey) -> Result<Option<Arc<StoredProgram>>, StoreReadPoisonError> {
        let store = self.store.read()?;
        Ok(store.get(key).map(Arc::clone))
    }

    pub fn find_or_create(
        &self,
        program: Program,
    ) -> Result<(Arc<StoredProgram>, FindOrCreateNotice), StoreWritePoisonError> {
        let mut store = self.store.write()?;
        let key = ProgramKey::from_program(&program);
        let mut notice = FindOrCreateNotice::AlreadyExists;
        let sp = store
            .entry(key)
            .or_insert_with(|| {
                notice = FindOrCreateNotice::Created;
                Arc::new(StoredProgram::new(program.clone()))
            })
            .clone();
        Ok((sp, notice))
    }

    pub fn update_program_metadata<'a>(
        &'a self,
        key: &'a ProgramKey,
        metadata_key: &str,
        metadata_value: &str,
    ) -> Result<(), MetadataWriteError<'a>> {
        let mut store = self.store.write()?;
        match store.get(key) {
            Some(sp) => {
                let mut sp = (**sp).clone();
                sp.metadata.insert(metadata_key.to_string(), metadata_value.to_string());
                store.insert(key.clone(), Arc::new(sp));
                Ok(())
            }
            None => Err(MetadataWriteError::ProgramNotFound(key)),
        }
    }
}
