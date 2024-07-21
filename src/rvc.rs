use std::{
    pin::Pin,
    task::{Poll, Waker},
};

use futures::Future;
use songbird::input::{Compose, YoutubeDl};

use crate::prelude::*;

#[derive(Default, Clone, Copy, poise::ChoiceParameter)]
pub enum Model {
    #[default]
    UI16,
}

impl Model {
    pub fn friendly_name(&self) -> &'static str {
        match self {
            Self::UI16 => "しぐれうい (16歳)",
        }
    }
}

pub struct RVCSong {
    pub model: Model,
    pub url: String,
    pub working_folder: String,
    pub worker: Option<std::thread::JoinHandle<()>>,
    pub shared: std::sync::Arc<std::sync::Mutex<RVCSharedData>>,
}

pub struct RVCSharedData {
    output: Option<String>,
    track_name: Option<String>,
}

impl RVCSharedData {
    pub fn new() -> Self {
        Self {
            output: None,
            track_name: None,
        }
    }
}

impl Future for RVCSongFuture {
    type Output = Result<String, Error>;

    fn poll(
        self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // let mut shared_state = self.shared.lock().unwrap();
        // if let Some(file) = shared_state.file.take() {
        //     Poll::Ready(file)
        // } else {
        //     shared_state.waker = Some(cx.waker().clone());
        //     Poll::Pending
        // }
        Poll::Pending
    }
}

impl RVCSong {
    pub fn new(model: Model, url: String) -> Self {
        let uuid = uuid::Uuid::new_v4();
        let working_folder = format!("./temp/rvc/{}", uuid);
        let shared = std::sync::Arc::new(std::sync::Mutex::new(RVCSharedData::new()));

        let model_thread = model.clone();
        let url_thread = url.clone();
        let working_folder_thread = working_folder.clone();
        let shared_thread = shared.clone();
        let worker = std::thread::spawn(move || {
            std::process::Command::new("yt-dlp")
                .current_dir(&working_folder_thread)
                .arg("-x")
                .arg("-o source_dl")
                .arg(url_thread);

            std::process::Command::new("ffmpeg")
                .current_dir(&working_folder_thread)
                .arg("-i")
                .arg("source_dl");
        });

        Self {
            model,
            url,
            working_folder,
            worker: Some(worker),
            shared,
        }
    }

    pub async fn get_file(&self) -> Result<String, Error> {
        Ok(String::from("rvc.wav"))
    }
}

impl Drop for RVCSong {
    fn drop(&mut self) {}
}
