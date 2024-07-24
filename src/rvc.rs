use std::{
    collections::HashMap, ffi::OsString, path::{Path, PathBuf}
};

use poise::{ChoiceParameter, CommandParameterChoice};
use serde::{Deserialize, Serialize};
use songbird::input::{AuxMetadata, Compose, YoutubeDl};
use tracing::warn;

use crate::prelude::*;

#[derive(Serialize, Deserialize)]
struct ModelMetadata {
    pub name: String,

    #[serde(default)]
    pub localizations: std::collections::HashMap<String, String>,
}

impl From<&ModelMetadata> for CommandParameterChoice {
    fn from(value: &ModelMetadata) -> Self {
        Self {
            name: value.name.clone(),
            localizations: value.localizations.clone(),
            __non_exhaustive: (),
        }
    }
}

pub struct ModelLibrary {
    models: HashMap<String, ModelMetadata>,
    name_index: Vec<String>,
    name_map: HashMap<String, String>,
}

impl ModelLibrary {
    fn load() -> Self {
        let mut models = HashMap::new();
        let mut name_index = Vec::new();
        let mut name_map = HashMap::new();

        let models_path = Path::new("RVC_CLI").join("rvc").join("models");
        match std::fs::read_dir(&models_path) {
            Ok(models_dir) => {
                let (num_models, _) = models_dir.size_hint();
                models.reserve(num_models);
                name_map.reserve(num_models * 2);


                for dir in models_dir {
                    let dir = match dir {
                        Ok(dir) => dir,
                        Err(_) => continue,
                    };

                    let raw_name = match dir.file_name().to_str() {
                        Some(str) => str.to_string(),
                        None => {
                            warn!("model name {:?} failed to convert string", dir.file_name());
                            continue
                        },
                    };

                    let uidata_path = dir.path().join("uidata.json");
                    if !uidata_path.exists() {
                        warn!("model {} doesn't have uidata.json", raw_name);
                        continue
                    }
                    
                    let file = match std::fs::File::open(uidata_path) {
                        Ok(file) => file,
                        Err(e) => {
                            warn!("model {} failed read uidata. error = {}", raw_name, e);
                            continue
                        }
                    };

                    let ui_data: ModelMetadata = match serde_json::from_reader(file) {
                        Ok(ui_data) => ui_data,
                        Err(e) => {
                            warn!("model {} failed to parse uidata. error = {}", raw_name, e);
                            continue
                        }
                    };

                    name_index.push(raw_name.clone());
                    
                    name_map.insert(raw_name.clone(), raw_name.clone());
                    for (_, name) in ui_data.localizations.iter() {
                        name_map.insert(name.clone(), raw_name.clone());
                    }

                    models.insert(raw_name, ui_data);
                }
            },
            Err(e) => {
                warn!("Failed to find model path {:?}, error = {}", models_path, e);
            }
        };

        Self {
            models,
            name_index,
            name_map
        }
    }

    fn choice_list(&self) -> Vec<poise::CommandParameterChoice> {
        self.models.iter().map(|t| t.1.into()).collect()
    }
}

lazy_static::lazy_static! {
    pub static ref model_library: ModelLibrary = ModelLibrary::load();
}

#[derive(Clone, Copy)]
pub struct Model {
    name: &'static str,
}

impl poise::ChoiceParameter for Model {
    
    fn from_index(index: usize) -> Option<Self> {
        model_library.name_index.get(index).map(|name| {
            Self { name: name.as_str() }
        })
    }

    fn from_name(name: &str) -> Option<Self> {
        model_library.name_map.get(name).map(|name| {
            Self { name: name.as_str() }
        })
    }

    fn list() -> Vec<poise::CommandParameterChoice> {
        model_library.choice_list()
    }

    fn localized_name(&self, locale: &str) -> Option<&'static str> {
        model_library.models.get(self.name)
        .and_then(|m| m.localizations.get(locale))
        .map(|s| s.as_str())
    }

    fn name(&self) -> &'static str {
        self.name
    }
}


// #[allow(non_camel_case_types)]
// #[derive(Default, Clone, Copy, poise::ChoiceParameter)]
// pub enum Model {

//     // ~ V-Tuber

//     #[default]
//     #[name = "Shigure UI (16)"]
//     #[name_localized("ja", "しぐれうい (16歳)")]
//     #[name_localized("ko", "시구레 우이 (16세)")]
//     ui16,


//     // ~ AMM

//     #[name = "Eungoo"]
//     #[name_localized("ja", "ウング")]
//     #[name_localized("ko", "은구")]
//     Eungoo,

//     #[name = "Sanghyeok"]
//     #[name_localized("ja", "サンヒョク")]
//     #[name_localized("ko", "상혁")]
//     Sanghyeok,

//     #[name = "Y00NN0NG"]
//     #[name_localized("ja", "ユンノン")]
//     #[name_localized("ko", "윤농")]
//     Y00NN0NG,


//     // ~ Blue Archive

//     #[name = "Arona"]
//     #[name_localized("ja", "アロナ")]
//     #[name_localized("ko", "아로나")]
//     Arona,

//     #[name = "Hanaoka Yuzu"]
//     #[name_localized("ja", "花岡ユズ")]
//     #[name_localized("ko", "하나오카 유즈")]
//     HanaokaYuzu,

//     #[name = "Saiba Momoi"]
//     #[name_localized("ja", "才羽モモイ")]
//     #[name_localized("ko", "사이바 모모이")]
//     SaibaMomoi,

//     #[name = "Saiba Midori"]
//     #[name_localized("ja", "才羽ミドリ")]
//     #[name_localized("ko", "사이바 미도리")]
//     SaibaMidori,

//     #[name = "Tendou Aris"]
//     #[name_localized("ja", "天童アリス")]
//     #[name_localized("ko", "텐도 아리스")]
//     TendouAlice,

//     #[name = "Hayase Yuuka"]
//     #[name_localized("ja", "早瀬ユウカ")]
//     #[name_localized("ko", "하야세 유우카")]
//     HayaseYuuka,

//     #[name = "Takanashi Hoshino"]
//     #[name_localized("ja", "小鳥遊ホシノ")]
//     #[name_localized("ko", "타카나시 호시노")]
//     TakanashiHoshino,

//     // ~ Mihoyo
    
//     #[name = "Kamisato Ayaka"]
//     #[name_localized("ja", "神里綾華")]
//     #[name_localized("ko", "카미사토 아야카")]
//     KamisatoAyaka,

//     // ~ Minecraft

//     #[name = "Villager"]
//     #[name_localized("ja", "村人")]
//     #[name_localized("ko", "주민")]
//     Villager,

//     #[name = "Chest"]
//     #[name_localized("ja", "チェスト")]
//     #[name_localized("ko", "상자")]
//     Chest,
    
//     #[name = "Lever"]
//     #[name_localized("ja", "レバー")]
//     #[name_localized("ko", "레버")]
//     Lever,

//     // ~ etc

//     #[name = "Yui"]
//     Yui,
    
//     #[name = "Gojo Satoru"]
//     #[name_localized("ja", "五条悟")]
//     #[name_localized("ko", "고죠 사토루")]
//     GojoSatoru,

//     #[name = "Ryomen Sukuna"]
//     #[name_localized("ja", "両面宿儺")]
//     #[name_localized("ko", "료멘스쿠나")]
//     RyomenSukuna,
// }

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
    pub async fn new(model: Model, mut youtube: YoutubeDl, pitch: Option<i32>, export_mp3: bool) -> Result<Self, Error> {
        let metadata = youtube.aux_metadata().await?;

        let id = chrono::offset::Local::now().format("%y%m%d_%H%M%S_%f").to_string();
        let working_dir = Path::new("temp").join("rvc").join(id);
        let shared = std::sync::Arc::new(std::sync::Mutex::new(RVCSharedData::default()));

        let metadata_thread = metadata.clone();
        let working_dir_thread = working_dir.clone();
        let shared_thread = shared.clone();

        let worker = tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&working_dir_thread)?;

            let find_file = |prefix: &str| -> Result<OsString, Error> {
                let mut working_dir = std::fs::read_dir(&working_dir_thread)?;
                let file = working_dir.find(
                    |f| f.as_ref().is_ok_and(
                        |f| f.file_name().to_str().is_some_and(
                            |s| s.starts_with(prefix)
                )));
                Ok(file.ok_or(Error::from("cannot find file "))??.file_name())
            };
            
            // download
            std::process::Command::new("yt-dlp")
                .current_dir(&working_dir_thread)
                .arg("-x")
                .arg("-o")
                .arg("source_dl")
                .arg(metadata_thread.source_url.ok_or(Error::from("no source url"))?)
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
                let uvr_out = std::process::Command::new(Path::new("audio-separator").join(".venv").join("Scripts").join("audio-separator.exe"))
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
                
                std::fs::rename(working_dir_thread.join(&source_vocals), working_dir_thread.join("kim_vocals.wav"))?;
                std::fs::rename(working_dir_thread.join(&source_inst), working_dir_thread.join("kim_inst.wav"))?;
            }

            // extract with karaoke
            {
                let uvr_out = std::process::Command::new(Path::new("audio-separator").join(".venv").join("Scripts").join("audio-separator.exe"))
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

                std::fs::rename(working_dir_thread.join(&source_vocals), working_dir_thread.join("karaoke_vocal.wav"))?;
                std::fs::rename(working_dir_thread.join(&source_inst), working_dir_thread.join("karaoke_harmony.wav"))?;
            }

            // convert
            {                
                let mut rvc = std::process::Command::new(Path::new("RVC_CLI").join("env").join("python.exe"));

                rvc
                    .current_dir("RVC_CLI")
                    .arg("main.py")
                    .arg("infer");
                
                if let Some(pitch) = pitch {
                    rvc
                        .arg("--f0up_key")
                        .arg(pitch.to_string());
                }

                rvc
                    .arg("--input_path")
                    .arg(Path::new("..").join(working_dir_thread.join("karaoke_vocal.wav")))
                    .arg("--output_path")
                    .arg(Path::new("..").join(working_dir_thread.join("rvc.wav")))
                    .arg("--pth_path")
                    .arg(Path::new("rvc").join("models").join(model.name()).join("model.pth"))
                    .arg("--index_path")
                    .arg(Path::new("rvc").join("models").join(model.name()).join("model.index"));

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
                            .arg(format!("asetrate=44100*{freq_ratio},aresample=44100,atempo=1/{freq_ratio}"))
                            .arg("mix_inst.wav")
                            .output()?;

                        println!("inst_shift_out = {}", String::from_utf8_lossy(&inst_shift_out.stdout));
                        println!("inst_shift_err = {}", String::from_utf8_lossy(&inst_shift_out.stderr));

                        let harmony_shift_out = std::process::Command::new("ffmpeg")
                            .current_dir(&working_dir_thread)
                            .arg("-i")
                            .arg("karaoke_harmony.wav")
                            .arg("-af")
                            .arg(format!("asetrate=44100*{freq_ratio},aresample=44100,atempo=1/{freq_ratio}"))
                            .arg("mix_harmony.wav")
                            .output()?;

                        println!("harmony_shift_out = {}", String::from_utf8_lossy(&harmony_shift_out.stdout));
                        println!("harmony_shift_err = {}", String::from_utf8_lossy(&harmony_shift_out.stderr));
                    }
                    _ => {
                        std::fs::rename(working_dir_thread.join("kim_inst.wav"), working_dir_thread.join("mix_inst.wav"))?;
                        std::fs::rename(working_dir_thread.join("karaoke_harmony.wav"), working_dir_thread.join("mix_harmony.wav"))?;
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

                println!("merge_inst_out = {}", String::from_utf8_lossy(&merge_inst_out.stdout));
                println!("merge_inst_err = {}", String::from_utf8_lossy(&merge_inst_out.stderr));

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

                println!("merge_vocal_out = {}", String::from_utf8_lossy(&merge_vocal_out.stdout));
                println!("merge_vocal_err = {}", String::from_utf8_lossy(&merge_vocal_out.stderr));
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
            tokio::task::spawn_blocking(move || {
                while !worker.is_finished() {}
            }).await?;
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
            Some(locale) => self.model.localized_name(locale).unwrap_or(self.model.name()),
            None => self.model.name(),
        };
        
        if let Some(title) = self.metadata.title.as_ref() {
            format!("{} - {}", model_name, title)
        }
        else {
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
