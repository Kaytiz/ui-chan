use std::{path::Path, pin::Pin, task::{Poll, Waker}};

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
            Self::UI16 => "しぐれうい (16歳)"
        }
    }
}

pub struct RVCSong {
    pub model: Model,
    pub url: String,
    pub song_data: std::sync::Arc<std::sync::Mutex<Result<RVCSongData, Error>>>,
}

struct RVCSongData {
    working_folder: String,
    file: Option<String>,
    track_name: Option<String>,
}

impl RVCSongData {
    fn new(working_folder: String) -> Self {
        Self {
            working_folder,
            file: None,
            track_name: None,
        }
    }
}

pub struct RVCSongFuture {
    pub shared: std::sync::Arc<std::sync::Mutex<RVCSongFutureShared>>,
}

pub struct RVCSongFutureShared {
    pub file: std::sync::Arc<std::sync::Mutex<Option<String>>>,
    pub waker: Option<Waker>,
}

impl Future for RVCSongFuture {
    type Output = Result<String, Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        let mut shared_state = self.shared.lock().unwrap();
        if let Some(file) = shared_state.file.take() {
            Poll::Ready(file)
        } else {
            shared_state.waker = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl RVCSong {
    pub fn new(model: Model, url: String) -> Self {
        
        let file = std::sync::Arc::new(std::sync::Mutex::new(None));

        let model_thread = model.clone();
        let url_thread = url.clone();
        let file_thread = file.clone();
        std::thread::spawn(move || {
            let 
            let name =
            //yt-dlp
            YoutubeDl::new(url_thread).create()
            std::process::Command::new(ytdl)
        });

        Self {
            model,
            url,
            file,
        }
    }

    pub async fn get_file(&self) -> Result<String, Error> {
        Ok(String::from("rvc.wav"))
    }
}

impl Drop for RVCSong {

}