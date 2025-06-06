use clap::builder::styling::AnsiColor;
use clap::builder::Styles;
use clap::{CommandFactory, FromArgMatches, Parser};
use std::path::PathBuf;

mod audio_processing;

pub use audio_processing::*;

#[derive(Parser, Debug)]
#[command(name = "Audio Visualizer")]
#[command(version = "1.0")]
#[command(author = "D.holguin@protonmail.com")]
#[command(
    about = "Audio Visualizer",
    long_about = "A simple real-time audio visualizer.\n\
                  \n\
                  By default, it listens to live audio input (e.g. from your microphone or a virtual cable).\
                  \n\
                  Alternatively, you can supply a path to an audio file to visualize that instead.\n\
                  \n\
                  Example usage:\n\
                  \n\
                      audio_visualizer                       # Live input mode\n\
                      audio_visualizer \"assets/track.mp3\"   # File playback mode\n\
                  \n\
                  NOTE: To visualize audio from apps like Spotify or YouTube, route your system output through a virtual audio device (e.g. VB-Cable on Windows, BlackHole on macOS) and run in live mode."
)]
#[command(
    after_long_help = "TIP: Try playing any MP3 file or route system audio to a virtual device for a cool effect."
)]
#[command(styles = get_custom_styles())]
pub struct Args {
    #[arg(value_name = "Audio File", help = "Path to an audio file to visualize. If omitted, the app uses live input.", num_args = 0..=1)]
    pub input_file: String,
}

impl Args {
    pub fn parse() -> Result<Config, String> {
        let args = Args::command_for_update().get_matches();
        let input_file_arg = Args::from_arg_matches(&args).map_err(|e| e.to_string())?;

        if input_file_arg.input_file.is_empty() {
            return Err("Input file is required".to_string());
        }

        let input_file: PathBuf = PathBuf::from(input_file_arg.input_file);

        if !input_file.exists() {
            return Err("Input file is required".to_string());
        }

        Ok(Config { input_file })
    }
}

#[derive(Debug)]
pub struct Config {
    pub input_file: PathBuf,
}

fn get_custom_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Yellow.on_default().bold())
        .usage(AnsiColor::Green.on_default())
        .error(AnsiColor::Blue.on_default())
        .placeholder(AnsiColor::Blue.on_default())
}
