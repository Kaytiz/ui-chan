use std::{
    collections::HashMap,
    ffi::OsString,
    path::{Path, PathBuf},
};

use poise::ChoiceParameter;
use serde::Deserialize;
use songbird::input::{AuxMetadata, Compose, YoutubeDl};
use tracing::warn;

use crate::prelude::*;

#[allow(non_upper_case_globals)]
static mut model_list_static: Option<[Option<ModelEntry>; 512]> = None;

lazy_static::lazy_static! {

    pub static ref model_library: std::sync::Arc<std::sync::Mutex<ModelLibrary>> = std::sync::Arc::new(std::sync::Mutex::new(ModelLibrary::load_or_warn()));
}

pub fn reload() {
    *model_library.lock().unwrap() = ModelLibrary::load_or_warn();
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
            name: value.name.clone(),
            localizations: value.localizations.clone(),
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
                println!("from_index {} -> {}", index, model_name);
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
                println!("from_name {} -> {}", name, model_name);
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
            .and_then(|m| m.localizations.get(locale))
            .map(|s| s.as_str())
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

pub struct RVCSong {
    pub model: Model,
    pub youtube: YoutubeDl,
    pub metadata: AuxMetadata,
    pub working_dir: PathBuf,
    pub worker: Option<std::sync::Arc<tokio::task::JoinHandle<Result<(), Error>>>>,
    pub shared: std::sync::Arc<std::sync::Mutex<RVCSharedData>>,
}

#[derive(Default)]
pub struct RVCSharedData {
    output: Option<PathBuf>,
    mp3: Option<PathBuf>,
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
        let shared = std::sync::Arc::new(std::sync::Mutex::new(RVCSharedData::default()));

        let metadata_thread = metadata.clone();
        let working_dir_thread = working_dir.clone();
        let shared_thread = shared.clone();

        let worker = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&working_dir_thread)?;

            let find_file = |prefix: &str| -> Result<OsString, Error> {
                let mut working_dir = std::fs::read_dir(&working_dir_thread)?;
                let file = working_dir.find(|f| {
                    f.as_ref().is_ok_and(|f| {
                        f.file_name()
                            .to_str()
                            .is_some_and(|s| s.starts_with(prefix))
                    })
                });
                Ok(file.ok_or(Error::from("cannot find file "))??.file_name())
            };

            // download
            std::process::Command::new("yt-dlp")
                .current_dir(&working_dir_thread)
                .arg("-x")
                .arg("-o")
                .arg("source_dl")
                .arg(
                    metadata_thread
                        .source_url
                        .ok_or(Error::from("no source url"))?,
                )
                .output()?;

            let downloaded_file = find_file("source_dl")?;

            std::process::Command::new("ffmpeg")
                .current_dir(&working_dir_thread)
                .arg("-i")
                .arg(&downloaded_file)
                .arg("source.wav")
                .output()?;

            // std::fs::remove_file(working_dir_thread.join(&downloaded_file))?;

            // extract with kim_vocal
            {
                let uvr_out = std::process::Command::new(
                    Path::new("audio-separator")
                        .join(".venv")
                        .join("Scripts")
                        .join("audio-separator.exe"),
                )
                .current_dir(&working_dir_thread)
                .arg("source.wav")
                .arg("--model_filename")
                .arg("Kim_Vocal_2.onnx")
                .arg("--output_format")
                .arg("wav")
                .output()?;

                println!("uvr_out = {}", String::from_utf8_lossy(&uvr_out.stdout));
                println!("uvr_err = {}", String::from_utf8_lossy(&uvr_out.stderr));

                let source_vocals = find_file("source_(Vocals)")?;
                let source_inst = find_file("source_(Instrumental)")?;

                std::fs::rename(
                    working_dir_thread.join(&source_vocals),
                    working_dir_thread.join("kim_vocals.wav"),
                )?;
                std::fs::rename(
                    working_dir_thread.join(&source_inst),
                    working_dir_thread.join("kim_inst.wav"),
                )?;
            }

            // extract with karaoke
            {
                let uvr_out = std::process::Command::new(
                    Path::new("audio-separator")
                        .join(".venv")
                        .join("Scripts")
                        .join("audio-separator.exe"),
                )
                .current_dir(&working_dir_thread)
                .arg("kim_vocals.wav")
                .arg("--model_filename")
                .arg("5_HP-Karaoke-UVR.pth")
                .arg("--output_format")
                .arg("wav")
                .output()?;

                println!("uvr_out = {}", String::from_utf8_lossy(&uvr_out.stdout));
                println!("uvr_err = {}", String::from_utf8_lossy(&uvr_out.stderr));

                let source_vocals = find_file("kim_vocals_(Vocals)")?;
                let source_inst = find_file("kim_vocals_(Instrumental)")?;

                std::fs::rename(
                    working_dir_thread.join(&source_vocals),
                    working_dir_thread.join("karaoke_vocal.wav"),
                )?;
                std::fs::rename(
                    working_dir_thread.join(&source_inst),
                    working_dir_thread.join("karaoke_harmony.wav"),
                )?;
            }

            // convert
            {
                let mut rvc =
                    std::process::Command::new(Path::new("RVC_CLI").join("env").join("python.exe"));

                rvc.current_dir("RVC_CLI").arg("main.py").arg("infer");

                if let Some(pitch) = pitch {
                    rvc.arg("--f0up_key").arg(pitch.to_string());
                }

                rvc.arg("--input_path")
                    .arg(Path::new("..").join(working_dir_thread.join("karaoke_vocal.wav")))
                    .arg("--output_path")
                    .arg(Path::new("..").join(working_dir_thread.join("rvc.wav")))
                    .arg("--pth_path")
                    .arg(
                        Path::new("rvc")
                            .join("models_map")
                            .join(model.name())
                            .join("model.pth"),
                    )
                    .arg("--index_path")
                    .arg(
                        Path::new("rvc")
                            .join("models_map")
                            .join(model.name())
                            .join("model.index"),
                    );

                let rvc_out = rvc.output()?;

                println!("rvc_out = {}", String::from_utf8_lossy(&rvc_out.stdout));
                println!("rvc_err = {}", String::from_utf8_lossy(&rvc_out.stderr));
            }

            // Inst pitchshift
            {
                match pitch.as_ref() {
                    Some(pitch) if *pitch != 0 => {
                        let normalized = (pitch + 6).rem_euclid(12) - 6;
                        let freq_ratio = 2.0f64.powf(normalized as f64 / 12.0);

                        let inst_shift_out = std::process::Command::new("ffmpeg")
                            .current_dir(&working_dir_thread)
                            .arg("-i")
                            .arg("kim_inst.wav")
                            .arg("-af")
                            .arg(format!(
                                "asetrate=44100*{freq_ratio},aresample=44100,atempo=1/{freq_ratio}"
                            ))
                            .arg("mix_inst.wav")
                            .output()?;

                        println!(
                            "inst_shift_out = {}",
                            String::from_utf8_lossy(&inst_shift_out.stdout)
                        );
                        println!(
                            "inst_shift_err = {}",
                            String::from_utf8_lossy(&inst_shift_out.stderr)
                        );

                        let harmony_shift_out = std::process::Command::new("ffmpeg")
                            .current_dir(&working_dir_thread)
                            .arg("-i")
                            .arg("karaoke_harmony.wav")
                            .arg("-af")
                            .arg(format!(
                                "asetrate=44100*{freq_ratio},aresample=44100,atempo=1/{freq_ratio}"
                            ))
                            .arg("mix_harmony.wav")
                            .output()?;

                        println!(
                            "harmony_shift_out = {}",
                            String::from_utf8_lossy(&harmony_shift_out.stdout)
                        );
                        println!(
                            "harmony_shift_err = {}",
                            String::from_utf8_lossy(&harmony_shift_out.stderr)
                        );
                    }
                    _ => {
                        std::fs::rename(
                            working_dir_thread.join("kim_inst.wav"),
                            working_dir_thread.join("mix_inst.wav"),
                        )?;
                        std::fs::rename(
                            working_dir_thread.join("karaoke_harmony.wav"),
                            working_dir_thread.join("mix_harmony.wav"),
                        )?;
                    }
                }
            }

            // merge
            {
                let merge_inst_out = std::process::Command::new("ffmpeg")
                    .current_dir(&working_dir_thread)
                    .arg("-i")
                    .arg("mix_inst.wav")
                    .arg("-i")
                    .arg("mix_harmony.wav")
                    .arg("-filter_complex")
                    .arg("[0:a][1:a]amerge=inputs=2,pan=stereo|c0=c0+c2|c1=c1+c3[a]")
                    .arg("-map")
                    .arg("[a]")
                    .arg("merge_inst.wav")
                    .output()?;

                println!(
                    "merge_inst_out = {}",
                    String::from_utf8_lossy(&merge_inst_out.stdout)
                );
                println!(
                    "merge_inst_err = {}",
                    String::from_utf8_lossy(&merge_inst_out.stderr)
                );

                let merge_vocal_out = std::process::Command::new("ffmpeg")
                    .current_dir(&working_dir_thread)
                    .arg("-i")
                    .arg("merge_inst.wav")
                    .arg("-i")
                    .arg("rvc.wav")
                    .arg("-filter_complex")
                    .arg("[0:a][1:a]amerge=inputs=2,pan=stereo|c0=c0+1.5*c2|c1=c1+1.5*c2[a]")
                    .arg("-map")
                    .arg("[a]")
                    .arg("mixdown.wav")
                    .output()?;

                println!(
                    "merge_vocal_out = {}",
                    String::from_utf8_lossy(&merge_vocal_out.stdout)
                );
                println!(
                    "merge_vocal_err = {}",
                    String::from_utf8_lossy(&merge_vocal_out.stderr)
                );
            }

            // mp3
            if export_mp3 {
                let mp3_out = std::process::Command::new("ffmpeg")
                    .current_dir(&working_dir_thread)
                    .arg("-i")
                    .arg("mixdown.wav")
                    .arg("-b:a")
                    .arg("320k")
                    .arg("mixdown.mp3")
                    .output()?;

                println!("mp3_out = {}", String::from_utf8_lossy(&mp3_out.stdout));
                println!("mp3_err = {}", String::from_utf8_lossy(&mp3_out.stderr));
            }

            let mixdown_path = working_dir_thread.join("mixdown.wav");
            let mp3_path = working_dir_thread.join("mixdown.mp3");

            let mut shared = shared_thread.lock().unwrap();
            shared.output = Some(mixdown_path);
            shared.mp3 = Some(mp3_path);

            Ok(())
        });

        Ok(Self {
            model,
            youtube,
            metadata,
            working_dir,
            worker: Some(std::sync::Arc::new(worker)),
            shared,
        })
    }

    pub async fn wait(&self) -> Result<(), Error> {
        if let Some(worker) = self.worker.clone() {
            tokio::task::spawn_blocking(move || while !worker.is_finished() {}).await?;
        }

        Ok(())
    }

    pub async fn file(&self) -> Result<PathBuf, Error> {
        self.wait().await?;
        let shared = self.shared.lock().unwrap();
        shared.output.clone().ok_or(Error::from("failed"))
    }

    pub async fn mp3(&self) -> Result<PathBuf, Error> {
        self.wait().await?;
        let shared = self.shared.lock().unwrap();
        shared.mp3.clone().ok_or(Error::from("failed"))
    }

    pub fn title(&self, locale: Option<&str>) -> String {
        let model_name = match locale {
            Some(locale) => self
                .model
                .localized_name(locale)
                .unwrap_or(self.model.name()),
            None => self.model.name(),
        };

        if let Some(title) = self.metadata.title.as_ref() {
            format!("{} - {}", model_name, title)
        } else {
            format!("{}", model_name)
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
        if let Some(worker) = self.worker.take() {
            let working_dir = self.working_dir.clone();
            tokio::spawn(async move {
                while !worker.is_finished() {}
                std::fs::remove_dir_all(working_dir).ok();
            });
        } else {
            std::fs::remove_dir_all(&self.working_dir).ok();
        }
    }
}
