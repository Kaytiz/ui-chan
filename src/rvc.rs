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

    Eungoo,
    Sanghyeok,
    Y00NN0NG,

    Arona,
    HanaokaYuzu,
    SaibaMomoi,
    SaibaMidori,
    TendouAlice,
    HayaseYuuka,
    TakanashiHoshino,

    Ayaka,

    Villager,
    Lever,
    Chest,

    Yui,
    GojoSatoru,
    RyomenSukuna,
}

impl Model {
    pub fn friendly_name(&self) -> &'static str {
        match self {

            Self::ui16 => "しぐれうい (16歳)",

            Self::Eungoo => "은구",
            Self::Sanghyeok => "상혁",
            Self::Y00NN0NG => "윤농",

            Self::Arona => "アロナ",
            Self::HanaokaYuzu => "花岡ユズ",
            Self::TendouAlice => "天童アリス",
            Self::SaibaMomoi => "才羽モモイ",
            Self::SaibaMidori => "才羽ミドリ",
            Self::HayaseYuuka => "早瀬ユウカ",
            Self::TakanashiHoshino => "小鳥遊ホシノ",
            
            Self::Ayaka => "아야카",

            Self::Villager => "주민",
            Self::Lever => "레버",
            Self::Chest => "상자",

            Self::Yui => "Yui",
            Self::GojoSatoru => "五条悟",
            Self::RyomenSukuna => "両面宿儺",
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
    output: Option<PathBuf>,
    mp3: Option<PathBuf>,
}

impl RVCSong {
    pub async fn new(model: Model, mut youtube: YoutubeDl, pitch: Option<i32>, mp3: bool) -> Result<Self, Error> {
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
                    .arg(Path::new("rvc").join("models").join(model.as_ref()).join("model.pth"))
                    .arg("--index_path")
                    .arg(Path::new("rvc").join("models").join(model.as_ref()).join("model.index"));

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
            {
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
