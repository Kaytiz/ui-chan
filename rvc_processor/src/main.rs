use std::{ffi::OsString, path::{Path, PathBuf}};

type Error = Box<dyn std::error::Error + Send + Sync>;

#[derive(Clone)]
struct Params {
    working_dir: PathBuf,
    source: String,
    model: String,
    pitch: i32,
    export_mp3: bool,
}
impl Params {
    fn new() -> Self {
        let mut args = std::env::args().skip(1);

        let working_dir = PathBuf::from(args.next().unwrap());
        let source = args.next().unwrap();
        let model = args.next().unwrap();
        let pitch = args.next().unwrap().parse().unwrap();
        let export_mp3 = args.next().unwrap().parse().unwrap();

        Params {
            working_dir,
            source,
            model,
            pitch,
            export_mp3
        }
    }
}

fn main() -> Result<(), Error> {
    let params = Params::new();
    
    let find_file = |prefix: &str| -> Result<OsString, Error> {
        let mut working_dir = std::fs::read_dir(&params.working_dir)?;
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
        .current_dir(&params.working_dir)
        .arg("-x")
        .arg("-o")
        .arg("source_dl")
        .arg(&params.source)
        .output()?;

    let downloaded_file = find_file("source_dl")?;

    std::process::Command::new("ffmpeg")
        .current_dir(&params.working_dir)
        .arg("-i")
        .arg(&downloaded_file)
        .arg("source.wav")
        .output()?;

    // extract with kim_vocal
    {
        std::process::Command::new(
            Path::new("audio-separator")
                .join(".venv")
                .join("Scripts")
                .join("audio-separator.exe"),
        )
            .current_dir(&params.working_dir)
            .arg("source.wav")
            .arg("--model_filename")
            .arg("Kim_Vocal_2.onnx")
            .arg("--output_format")
            .arg("wav")
            .output()?;

        let source_vocals = find_file("source_(Vocals)")?;
        let source_inst = find_file("source_(Instrumental)")?;

        std::fs::rename(
            params.working_dir.join(&source_vocals),
            params.working_dir.join("kim_vocals.wav"),
        )?;
        std::fs::rename(
            params.working_dir.join(&source_inst),
            params.working_dir.join("kim_inst.wav"),
        )?;
    }

    // extract with karaoke
    {
        std::process::Command::new(
            Path::new("audio-separator")
                .join(".venv")
                .join("Scripts")
                .join("audio-separator.exe"),
        )
            .current_dir(&params.working_dir)
            .arg("kim_vocals.wav")
            .arg("--model_filename")
            .arg("5_HP-Karaoke-UVR.pth")
            .arg("--output_format")
            .arg("wav")
            .output()?;

        let source_vocals = find_file("kim_vocals_(Vocals)")?;
        let source_inst = find_file("kim_vocals_(Instrumental)")?;

        std::fs::rename(
            params.working_dir.join(&source_vocals),
            params.working_dir.join("karaoke_vocal.wav"),
        )?;
        std::fs::rename(
            params.working_dir.join(&source_inst),
            params.working_dir.join("karaoke_harmony.wav"),
        )?;
    };

    // convert
    {
        let mut rvc =
            std::process::Command::new(Path::new("RVC_CLI").join("env").join("python.exe"));

        rvc
            .current_dir("RVC_CLI")
            .arg("main.py")
            .arg("infer");

        if params.pitch != 0 {
            rvc
                .arg("--f0up_key")
                .arg(params.pitch.to_string());
        }

        rvc.arg("--input_path")
            .arg(Path::new("..").join(&params.working_dir).join("karaoke_vocal.wav"))
            .arg("--output_path")
            .arg(Path::new("..").join(&params.working_dir).join("rvc.wav"))
            .arg("--pth_path")
            .arg(
                Path::new("rvc")
                    .join("models")
                    .join(&params.model)
                    .join("model.pth"),
            )
            .arg("--index_path")
            .arg(
                Path::new("rvc")
                    .join("models")
                    .join(&params.model)
                    .join("model.index"),
            );

        rvc.output()?;
    }

    // Inst pitchshift
    {
        if params.pitch != 0 {
            let normalized = (params.pitch + 6).rem_euclid(12) - 6;
            let freq_ratio = 2.0f64.powf(normalized as f64 / 12.0);

            std::process::Command::new("ffmpeg")
                .current_dir(&params.working_dir)
                .arg("-i")
                .arg("kim_inst.wav")
                .arg("-af")
                .arg(format!(
                    "asetrate=44100*{freq_ratio},aresample=44100,atempo=1/{freq_ratio}"
                ))
                .arg("mix_inst.wav")
                .output()?;

            std::process::Command::new("ffmpeg")
                .current_dir(&params.working_dir)
                .arg("-i")
                .arg("karaoke_harmony.wav")
                .arg("-af")
                .arg(format!(
                    "asetrate=44100*{freq_ratio},aresample=44100,atempo=1/{freq_ratio}"
                ))
                .arg("mix_harmony.wav")
                .output()?;
        } else {
            std::fs::rename(
                params.working_dir.join("kim_inst.wav"),
                params.working_dir.join("mix_inst.wav"),
            )?;
            std::fs::rename(
                params.working_dir.join("karaoke_harmony.wav"),
                params.working_dir.join("mix_harmony.wav"),
            )?;
        }
    }

    // merge
    {
        std::process::Command::new("ffmpeg")
            .current_dir(&params.working_dir)
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

        std::process::Command::new("ffmpeg")
            .current_dir(&params.working_dir)
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
    }

    // mp3
    if params.export_mp3 {
        std::process::Command::new("ffmpeg")
            .current_dir(&params.working_dir)
            .arg("-i")
            .arg("mixdown.wav")
            .arg("-b:a")
            .arg("320k")
            .arg("mixdown.mp3")
            .output()?;
    }

    Ok(())
}
