use std::{
    ffi::OsString, path::{Path, PathBuf}
};

use songbird::input::{AuxMetadata, Compose, YoutubeDl};

use crate::prelude::*;

#[allow(non_camel_case_types)]
#[derive(Default, Clone, Copy, poise::ChoiceParameter, strum::AsRefStr)]
pub enum Model {
    #[default]
    ui16,
    SaibaMomoi,
    Villager,
    Ayaka,
    // Eunsoo,
    // Sanghyeok,
}

impl Model {
    pub fn friendly_name(&self) -> &'static str {
        match self {
            Self::ui16 => "しぐれうい (16歳)",
            Self::SaibaMomoi => "才羽 モモイ",
            Self::Villager => "주민",
            Self::Ayaka => "神里綾華",
            // Self::Eunsoo => "김은수",
            // Self::Sanghyeok => "한상혁",
        }
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
    output: Option<String>,
}

impl RVCSong {
    pub async fn new(model: Model, mut youtube: YoutubeDl) -> Result<Self, Error> {
        let metadata = youtube.aux_metadata().await?;

        let uuid = uuid::Uuid::new_v4();
        let working_dir = Path::new("temp").join("rvc").join(uuid.to_string());
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
                .arg(downloaded_file)
                .arg("source.wav")
                .output()?;

            let source_path = working_dir_thread.join("source.wav");


            // extract
            let uvr_out = std::process::Command::new(Path::new("RVC_CLI").join("env").join("python.exe"))
                .current_dir("RVC_CLI")
                .arg("uvr.py")
                .arg("--model_filename")
                .arg("Kim_Vocal_2.onnx")
                .arg("--model_file_dir")
                .arg(Path::new("uvr").join("models"))
                .arg("--output_dir")
                .arg(Path::new("..").join(&working_dir_thread))
                .arg(Path::new("..").join(source_path))
                .output()?;
            
            println!("uvr_out = {}", String::from_utf8_lossy(&uvr_out.stdout));
            println!("uvr_err = {}", String::from_utf8_lossy(&uvr_out.stderr));

            let source_vocal = find_file("source_(Vocals)")?;
            let source_inst = find_file("source_(Instrumental)")?;

            let source_vocal_path = working_dir_thread.join(source_vocal);

            // convert
            let rvc_out_path = working_dir_thread.join(Path::new("rvc_out.wav"));
            
            let rvc_out = std::process::Command::new(Path::new("RVC_CLI").join("env").join("python.exe"))
                .current_dir("RVC_CLI")
                .arg("main.py")
                .arg("infer")
                .arg("--input_path")
                .arg(Path::new("..").join(source_vocal_path))
                .arg("--output_path")
                .arg(Path::new("..").join(rvc_out_path))
                .arg("--pth_path")
                .arg(Path::new("rvc").join("models").join(model.as_ref()).join("model.pth"))
                .arg("--index_path")
                .arg(Path::new("rvc").join("models").join(model.as_ref()).join("model.index"))
                .output()?;
            
            println!("rvc_out = {}", String::from_utf8_lossy(&rvc_out.stdout));
            println!("rvc_err = {}", String::from_utf8_lossy(&rvc_out.stderr));

            

            // merge
            let merge_out = std::process::Command::new("ffmpeg")
                .current_dir(&working_dir_thread)
                .arg("-i")
                .arg("rvc_out.wav")
                .arg("-i")
                .arg(&source_inst)
                .arg("-filter_complex")
                .arg("[0:a][1:a]amerge=inputs=2,pan=stereo|c0=c0+c2|c1=c1+c2[a]")
                .arg("-map")
                .arg("[a]")
                .arg("mixdown.wav")
                .output()?;
            
            println!("merge_out = {}", String::from_utf8_lossy(&merge_out.stdout));
            println!("merge_err = {}", String::from_utf8_lossy(&merge_out.stderr));

            let mixdown_path = working_dir_thread.join("mixdown.wav");

            let mut shared = shared_thread.lock().unwrap();
            shared.output = Some(mixdown_path.to_string_lossy().to_string());

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

    pub async fn file(&self) -> Result<String, Error> {
        if let Some(worker) = self.worker.clone() {
            tokio::task::spawn_blocking(move || {
                while !worker.is_finished() {}
            }).await?;
        }

        let shared = self.shared.lock().unwrap();
        shared.output.clone().ok_or(Error::from("failed"))
    }
}

impl std::fmt::Display for RVCSong {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(title) = self.metadata.title.as_ref() {
            write!(f, "{} - {}", self.model.friendly_name(), title)
        }
        else {
            write!(f, "{}", self.model.friendly_name())
        }
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
