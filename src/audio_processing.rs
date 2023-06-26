use nannou::prelude::*;
use nannou_audio::Buffer;
use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use std::sync::{Arc, Mutex};

pub struct Audio {
    pub sounds: Vec<audrey::read::BufFileReader>,
    pub volume_sender: std::sync::mpsc::Sender<f32>,
    pub volume: Arc<Mutex<f32>>,
    pub fft_output: Arc<Mutex<Vec<Complex<f32>>>>,
}

pub fn render(audio: &mut Audio, buffer: &mut Buffer) {
    let mut have_ended = vec![];
    let len_frames = buffer.len_frames();
    let mut rms_volume = 0.0;

    for (i, sound) in audio.sounds.iter_mut().enumerate() {
        let mut frame_count = 0;
        let file_frames = sound.frames::<[f32; 2]>().filter_map(Result::ok);
        for (frame, file_frame) in buffer.frames_mut().zip(file_frames) {
            let mut frame_rms = 0.0;
            for (sample, file_sample) in frame.iter_mut().zip(&file_frame) {
                *sample += *file_sample;
                frame_rms += *file_sample * *file_sample;
            }
            rms_volume += (frame_rms / 2.0).sqrt();
            frame_count += 1;
        }

        if frame_count < len_frames {
            have_ended.push(i);
        }
    }

    for i in have_ended.into_iter().rev() {
        audio.sounds.remove(i);
    }

    let volume = rms_volume / len_frames as f32 * 100.0;
    *audio.volume.lock().unwrap() = volume;
    audio.volume_sender.send(volume).ok();

    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(buffer.len_frames());

    let n = buffer.len_frames() as f32;
    let mut fft_input: Vec<Complex<f32>> = buffer
        .frames()
        .flat_map(|frame| frame.iter())
        .enumerate()
        .map(|(i, &s)| {
            let window_value = 0.5 * (1.0 - (2.0 * PI * i as f32 / n).cos());
            Complex::new(s * window_value, 0.0)
        })
        .collect();

    fft.process(&mut fft_input);

    *audio.fft_output.lock().unwrap() = fft_input;
}
