use std::{
    collections::HashMap, path::{Path, PathBuf}
};
use std::sync::{Arc, Mutex, Weak, mpsc::Sender};
use serde::Deserialize;
use shared_child::SharedChild;
use songbird::input::{AuxMetadata, Compose, YoutubeDl};
use tracing::warn;

use crate::prelude::*;

#[allow(non_upper_case_globals)]
static mut model_list_static: Option<[Option<ModelEntry>; 512]> = None;

lazy_static::lazy_static! {
    static ref model_library: Arc<Mutex<ModelLibrary>> = Arc::new(Mutex::new(ModelLibrary::load_or_warn()));
    static ref processor_queue: RVCProcessorQueue = RVCProcessorQueue::new();
}

pub fn reload() {
    *model_library.lock().unwrap() = ModelLibrary::load_or_warn();
}

#[derive(Clone, Copy, Debug)]
pub enum RVCProcessorResult {
    Good,
    Canceled,
    Error,
}

#[derive(Debug)]
enum RVCProcessorState {
    Wait(std::process::Command),
    Running(Arc<SharedChild>),
    Completed(RVCProcessorResult),
}


struct RVCProcessor {
    working_dir: PathBuf,
    state: Option<RVCProcessorState>,
}

impl RVCProcessor {
    fn new(working_dir: PathBuf, command: std::process::Command) -> Self {
        Self {
            working_dir,
            state: Some(RVCProcessorState::Wait(command))
        }
    }

    fn run(&mut self) -> Option<Arc<SharedChild>> {
        match self.state.take() {
            Some(RVCProcessorState::Wait(command)) => {
                match self.run_inner(command) {
                    Ok(child) => {
                        let child = Arc::new(child);
                        self.state = Some(RVCProcessorState::Running(child.clone()));
                        return Some(child);
                    },
                    Err(_) => {
                        self.state = Some(RVCProcessorState::Completed(RVCProcessorResult::Error))
                    }
                }
            },
            Some(state) => {
                self.state = Some(state)
            }
            None => {}
        }

        None
    }

    fn run_inner(&mut self, mut command: std::process::Command) -> Result<SharedChild, Error> {
        std::fs::create_dir_all(&self.working_dir)?;
        let child = SharedChild::spawn(&mut command)?;
        Ok(child)
    }
    
    fn completed(&mut self) {
        match self.state.take() {
            Some(RVCProcessorState::Running(_)) => {
                self.state = Some(RVCProcessorState::Completed(RVCProcessorResult::Good));
            },
            Some(state) => self.state = Some(state),
            None => {}
        }
    }

    fn cancel(&mut self) {
        let child = match self.state.take() {
            Some(RVCProcessorState::Running(child)) => Some(child),
            _ => None,
        };

        self.state = Some(RVCProcessorState::Completed(RVCProcessorResult::Canceled));

        if let Some(child) = child {
            child.kill().ok();
        }
    }
}

impl Drop for RVCProcessor {
    fn drop(&mut self) {
        if cfg!(debug_assertions) {
            return;
        }

        std::fs::remove_dir_all(&self.working_dir).ok();
    }
}

struct RVCProcessorQueue {
    tx: Sender<Weak<Mutex<RVCProcessor>>>,
}

impl RVCProcessorQueue {
    fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            loop {
                let processor_weak: Weak<Mutex<RVCProcessor>> = rx.recv().unwrap();
                let processor = match processor_weak.upgrade() {
                    Some(processor) => processor,
                    None => continue,
                };
                
                let child = processor.lock().unwrap().run();
                if let Some(child) = child {
                    child.wait().ok();
                }

                processor.lock().unwrap().completed();
            }
        });

        Self {
            tx
        }
    }

    fn queue(&self, processor: Weak<Mutex<RVCProcessor>>) {
        self.tx.send(processor).ok();
    }
}

#[derive(PartialEq, Eq, Deserialize)]
struct ModelGroup {
    #[serde(default, rename = "group_name")]
    name: String,

    models: Vec<ModelEntry>,
}

#[derive(PartialEq, Eq, Deserialize)]
struct ModelEntry {
    raw_name: String,

    #[serde(flatten)]
    metadata: ModelMetadata,
}

impl ModelEntry {
    fn register(self) -> Option<&'static Self> {
        unsafe {
            if model_list_static.is_none() {
                model_list_static = Some(std::array::from_fn(|_| None));
            }

            let model_list = model_list_static.as_mut().unwrap();

            for entry in model_list.iter_mut() {
                match entry {
                    Some(entry) => {
                        if entry == &self {
                            return Some(entry);
                        }
                    }
                    None => {
                        *entry = Some(self);
                        return entry.as_ref();
                    }
                }
            }

            warn!("model overflow");
            None
        }
    }
}

#[derive(Clone, PartialEq, Eq, Deserialize)]
struct ModelMetadata {
    name: String,

    #[serde(default)]
    localizations: std::collections::HashMap<String, String>,

    #[serde(skip)]
    group: String,

    #[serde(skip)]
    select_name: String,

    #[serde(skip)]
    select_localizations: std::collections::HashMap<String, String>,
}

impl ModelMetadata {
    fn set_group(&mut self, group: &str) {
        self.group = group.trim().to_string();
        if self.group.is_empty() {
            self.select_name = self.name.clone();
            self.select_localizations = self.localizations.clone();
        } else {
            self.select_name = format!("{} - {}", self.group, self.name);
            self.select_localizations = self
                .localizations
                .iter()
                .map(|(k, v)| (k.clone(), format!("{} - {}", self.group, v)))
                .collect();
        }
    }
}

impl From<&ModelMetadata> for poise::CommandParameterChoice {
    fn from(value: &ModelMetadata) -> Self {
        Self {
            name: value.select_name.clone(),
            localizations: value.select_localizations.clone(),
            __non_exhaustive: (),
        }
    }
}

#[derive(Default)]
pub struct ModelLibrary {
    models_map: HashMap<&'static str, &'static ModelMetadata>,
    name_order: Vec<&'static str>,
    name_map: HashMap<&'static str, &'static str>,
}

impl ModelLibrary {
    fn load() -> Result<Self, Error> {
        let mut models_map = HashMap::new();
        let mut name_order = Vec::new();
        let mut name_map = HashMap::new();

        let models_map_path = Path::new("RVC_CLI").join("rvc").join("models");

        let model_list = std::fs::File::open(models_map_path.join("uidata.json"))?;
        let mut group_list: Vec<ModelGroup> = serde_json::from_reader(model_list)?;
        for group in &mut group_list {
            group
                .models
                .iter_mut()
                .for_each(|m| m.metadata.set_group(&group.name))
        }
        let model_list: Vec<ModelEntry> = group_list
            .into_iter()
            .flat_map(|g| g.models.into_iter())
            .collect();

        let num_models_map = model_list.len();
        models_map.reserve(num_models_map);
        name_map.reserve(num_models_map * 2);

        for model_entry in model_list {
            let pth_path = models_map_path
                .join(&model_entry.raw_name)
                .join("model.pth");
            let index_path = models_map_path
                .join(&model_entry.raw_name)
                .join("model.index");
            if !pth_path.exists() || !index_path.exists() {
                warn!("model {} file not found", &model_entry.raw_name);
                continue;
            }

            let model_entry = match model_entry.register() {
                Some(entry) => entry,
                None => continue,
            };

            name_order.push(model_entry.raw_name.as_str());

            name_map.insert(model_entry.raw_name.as_str(), model_entry.raw_name.as_str());
            for (_, name) in model_entry.metadata.localizations.iter() {
                name_map.insert(name.as_str(), model_entry.raw_name.as_str());
            }

            models_map.insert(model_entry.raw_name.as_str(), &model_entry.metadata);
        }

        Ok(Self {
            models_map,
            name_order,
            name_map,
        })
    }

    fn load_or_warn() -> ModelLibrary {
        let library = Self::load();
        match library {
            Ok(library) => library,
            Err(e) => {
                warn!("failed to load rvc library, e = {}", e);
                Default::default()
            }
        }
    }

    fn choice_list(&self) -> Vec<poise::CommandParameterChoice> {
        self.name_order
            .iter()
            .filter_map(|n| self.models_map.get(*n))
            .map(std::ops::Deref::deref)
            .map(poise::CommandParameterChoice::from)
            .collect()
    }
}

#[derive(Clone, Copy)]
pub struct Model {
    name: &'static str,
}

impl poise::ChoiceParameter for Model {
    fn from_index(index: usize) -> Option<Self> {
        model_library
            .lock()
            .unwrap()
            .name_order
            .get(index)
            .map(|model_name| {
                Self { name: model_name }
            })
    }

    fn from_name(name: &str) -> Option<Self> {
        model_library
            .lock()
            .unwrap()
            .name_map
            .get(name)
            .map(|model_name| {
                Self { name: model_name }
            })
    }

    fn list() -> Vec<poise::CommandParameterChoice> {
        model_library.lock().unwrap().choice_list()
    }

    fn localized_name(&self, locale: &str) -> Option<&'static str> {
        model_library
            .lock()
            .unwrap()
            .models_map
            .get(self.name)
            .and_then(|m| m.select_localizations.get(locale))
            .map(|s| s.as_str())
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

impl Model {
    fn display_name(&self, locale: Option<&str>) -> Option<&'static str> {
        let library = model_library.lock().unwrap();
            
        let model = *library.models_map.get(self.name)?;
        if let Some(locale) = locale {
            if let Some(name) = model.localizations.get(locale) {
                return Some(name.as_str());
            }
        }

        return Some(model.name.as_str());
    }
}

pub struct RVCSong {
    pub model: Model,
    pub youtube: YoutubeDl,
    pub metadata: AuxMetadata,
    pub working_dir: PathBuf,
    processor: Arc<Mutex<RVCProcessor>>,
}

impl RVCSong {
    pub async fn new(
        model: Model,
        mut youtube: YoutubeDl,
        pitch: Option<i32>,
        export_mp3: bool,
    ) -> Result<Self, Error> {
        let metadata = youtube.aux_metadata().await?;

        let id = chrono::offset::Local::now()
            .format("%y%m%d_%H%M%S_%f")
            .to_string();
        let working_dir = Path::new("temp").join("rvc").join(id);

        let mut command = std::process::Command::new("rvc_processor");
        command
            .arg(&working_dir)
            .arg(metadata.source_url.as_ref().ok_or(Error::from("No Source URL"))?)
            .arg(model.name)
            .arg(pitch.unwrap_or(0).to_string())
            .arg(export_mp3.to_string());

        let processor = Arc::new(Mutex::new(RVCProcessor::new(working_dir.clone(), command)));
        processor_queue.queue(Arc::downgrade(&processor));
        
        Ok(Self {
            model,
            youtube,
            metadata,
            working_dir,
            processor,
        })
    }

    pub fn cancel(&self) {
        self.processor.lock().unwrap().cancel();
    }

    pub async fn wait(&self) -> Result<RVCProcessorResult, Error> {
        let processor = self.processor.clone();
        let result = tokio::task::spawn_blocking(move || {
            loop {
                match processor.lock().unwrap().state {
                    Some(RVCProcessorState::Wait(_)) => {},
                    Some(RVCProcessorState::Running(_)) => {},
                    Some(RVCProcessorState::Completed(result)) => {
                        return result;
                    }
                    _ => {
                        return RVCProcessorResult::Error;
                    }
                }
                std::thread::sleep(std::time::Duration::from_millis(1000))
            }
        }).await?;

        Ok(result)
    }

    pub fn file(&self) -> PathBuf {
        self.working_dir.join("mixdown.wav")
    }

    pub fn mp3(&self) -> PathBuf {
        self.working_dir.join("mixdown.mp3")
    }

    pub fn title(&self, locale: Option<&str>) -> String {
        let model_name = self.model.display_name(locale).unwrap_or(self.model.name);

        if let Some(title) = self.metadata.title.as_ref() {
            format!("{} - {}", model_name, title)
        } else {
            model_name.to_string()
        }
    }
}

impl std::fmt::Display for RVCSong {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.title(None))
    }
}

impl Drop for RVCSong {
    fn drop(&mut self) {
        self.processor.lock().unwrap().cancel();
    }
}