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
    pub file_sample_rate: Option<u32>,
}

fn compute_rms(samples: &[f32]) -> f32 {
    let sum_squares: f32 = samples.iter().map(|s| s * s).sum();
    (sum_squares / samples.len() as f32).sqrt() * 100.0
}

fn compute_fft(samples: &[f32]) -> Vec<Complex<f32>> {
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(samples.len());

    let windowed: Vec<Complex<f32>> = samples
        .iter()
        .enumerate()
        .map(|(i, &s)| {
            let window_value = 0.5 * (1.0 - (2.0 * PI * i as f32 / samples.len() as f32).cos());
            Complex::new(s * window_value, 0.0)
        })
        .collect();

    let mut fft_input = windowed.clone();
    fft.process(&mut fft_input);
    fft_input
}

pub fn capture(audio: &mut Audio, buffer: &Buffer) {
    let input_samples: Vec<f32> = buffer.frames().flat_map(|f| f.iter().copied()).collect();
    let volume = compute_rms(&input_samples);

    *audio.volume.lock().unwrap() = volume;
    audio.volume_sender.send(volume).ok();

    let fft_output = compute_fft(&input_samples);
    *audio.fft_output.lock().unwrap() = fft_output;
}

pub fn render(audio: &mut Audio, buffer: &mut Buffer) {
    let mut have_ended = vec![];
    let len_frames = buffer.len_frames();
    let mut all_samples = vec![0.0; len_frames * 2]; // Assuming stereo
    let mut total_rms = 0.0;

    for (i, sound) in audio.sounds.iter_mut().enumerate() {
        let mut frame_count = 0;
        let file_frames = sound.frames::<[f32; 2]>().filter_map(Result::ok);
        for (frame, file_frame) in buffer.frames_mut().zip(file_frames) {
            let mut frame_rms = 0.0;
            for (j, (sample, file_sample)) in frame.iter_mut().zip(&file_frame).enumerate() {
                *sample += *file_sample;
                all_samples[frame_count * 2 + j] += *file_sample;
                frame_rms += *file_sample * *file_sample;
            }
            total_rms += (frame_rms / 2.0).sqrt();
            frame_count += 1;
        }

        if frame_count < len_frames {
            have_ended.push(i);
        }
    }

    for i in have_ended.into_iter().rev() {
        audio.sounds.remove(i);
    }

    let volume = (total_rms / len_frames as f32) * 100.0;
    *audio.volume.lock().unwrap() = volume;
    audio.volume_sender.send(volume).ok();

    let fft_output = compute_fft(&all_samples);
    *audio.fft_output.lock().unwrap() = fft_output;
}
