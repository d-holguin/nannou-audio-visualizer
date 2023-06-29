use audio_visualizer::*;
use nannou::prelude::*;
use nannou_audio as audio;
use rustfft::num_complex::Complex;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

fn main() {
    nannou::app(model).update(update).run();
}

fn model(app: &App) -> Model {
    app.new_window()
        .key_pressed(key_pressed)
        .view(view)
        .build()
        .unwrap();

    let audio_host = audio::Host::new();
    let fft_output = Arc::new(Mutex::new(vec![]));
    let (volume_sender, volume_receiver) = channel();
    let volume = Arc::new(Mutex::new(0.0));

    let audio_model = Audio {
        sounds: vec![],
        volume_sender,
        fft_output: Arc::clone(&fft_output),
        volume: Arc::clone(&volume),
    };

    let stream = audio_host
        .new_output_stream(audio_model)
        .render(audio_visualizer::render)
        .build()
        .unwrap();

    stream.play().unwrap();

    Model {
        stream,
        volume,
        fft_output,
        previous_hue: 0.6,
        previous_frequency_multiplier: 0.0,
        previous_circle_radius: 50.0,
        hue: 0.0,
        high_freq_sum: 0.0,
        low_freq_sum: 0.0,
        string_points: Vec::new(),
        circle_radius: 0.0,
        line_color: nannou::color::hsl(0.0, 0.0, 0.0).into(), // Setting initial color to black
        circle_color: nannou::color::hsl(0.0, 0.0, 0.0).into(),
        prev_power_spectrum: Vec::new(),
        past_magnitudes: vec![vec![0.0; 10]; 6],
        past_spectral_flux: Vec::new(),
        cooldown_counter: 0,
    }
}

fn key_pressed(app: &App, model: &mut Model, key: Key) {
    match key {
        Key::Space => {
            let assets = app.assets_path().expect("could not find assets directory");
            let path = assets.join("time.wav");
            let sound = audrey::open(path).expect("failed to load sound");

            // Update the sample rate in the audio model
            model
                .stream
                .send(move |audio| {
                    audio.sounds.push(sound);
                })
                .ok();
        }
        _ => {}
    }
}
struct Model {
    stream: audio::Stream<Audio>,
    volume: Arc<Mutex<f32>>,
    fft_output: Arc<Mutex<Vec<Complex<f32>>>>,
    previous_hue: f32,
    previous_frequency_multiplier: f32,
    previous_circle_radius: f32,
    hue: f32,
    high_freq_sum: f32,
    low_freq_sum: f32,
    string_points: Vec<Vec<Point2>>,
    circle_radius: f32,
    line_color: LinSrgba,
    circle_color: LinSrgba,
    prev_power_spectrum: Vec<f32>,
    past_magnitudes: Vec<Vec<f32>>,
    past_spectral_flux: Vec<f32>,
    cooldown_counter: usize,
}
fn process_fft_output(fft_output: &[f32], prev_power_spectrum: &mut Vec<f32>) -> f32 {
    let mut power_spectrum = Vec::new();
    let mut spectral_flux = 0.0;

    for &value in fft_output {
        let power = value * value;
        power_spectrum.push(power);
    }

    if !prev_power_spectrum.is_empty() {
        for i in 0..power_spectrum.len() {
            let flux = power_spectrum[i] - prev_power_spectrum[i];
            if flux > 0.0 {
                spectral_flux += flux;
            }
        }
    }

    *prev_power_spectrum = power_spectrum;

    spectral_flux
}

fn update(app: &App, model: &mut Model, _update: Update) {
    let fft_output_guard = model.fft_output.lock().unwrap();

    let mut fft_magnitudes: Vec<f32> = fft_output_guard.iter().map(|c| c.norm()).collect();

    let spectral_flux = process_fft_output(&fft_magnitudes, &mut model.prev_power_spectrum);

    let neon_hue = 0.6 + 0.3 * (model.hue / 1.0);

    model.line_color = nannou::color::hsl(neon_hue, 1.0, 0.45).into();
    model.circle_color = nannou::color::hsl(neon_hue, 1.0, 0.45).into();

    let string_positions = [-90.0, -60.0, -30.0, 0.0, 30.0, 60.0];
    model.string_points.clear();

    const N: usize = 20;
    for (index, mag) in fft_magnitudes.iter_mut().enumerate() {
        let past_mags = &mut model.past_magnitudes[index % 6];
        past_mags.push(*mag);
        if past_mags.len() > N {
            past_mags.remove(0);
        }
        *mag = past_mags.iter().sum::<f32>() / past_mags.len() as f32;
    }

    let volume = *model.volume.lock().unwrap();
    let amplitude = if volume > 0.0 {
        (volume.log(10.0) * 8.5).max(1.0).min(100.0)
    } else {
        1.0
    };

    let log_spectral_flux = (spectral_flux + 1.0).log(10.0);

    let frequency_multiplier = log_spectral_flux * 0.75;

    let mut target_circle_radius = 50.0;

    if model.cooldown_counter > 0 {
        model.cooldown_counter -= 1;
    }

    let spectral_flux_frames: usize = 15; // Number of past frames to average
    model.past_spectral_flux.push(spectral_flux);
    if model.past_spectral_flux.len() > spectral_flux_frames {
        model.past_spectral_flux.remove(0);
    }

    let avg_spectral_flux =
        model.past_spectral_flux.iter().sum::<f32>() / model.past_spectral_flux.len() as f32;

    let beat_detection_threshold = 175.0;
    const COOLDOWN_TIME: usize = 30;
    if model.cooldown_counter == 0 {
        if avg_spectral_flux > beat_detection_threshold {
            model.hue = (model.hue + 0.3) % 1.0;
            target_circle_radius = 100.0;

            model.cooldown_counter = COOLDOWN_TIME;
        }
    } else {
        model.cooldown_counter -= 1;
    }

    const DECAY_FACTOR: f32 = 0.50;

    // Decay the target circle radius
    target_circle_radius *= DECAY_FACTOR;

    const SMOOTHING_FACTOR: f32 = 1.0;

    model.circle_radius = model.previous_circle_radius
        + (target_circle_radius - model.previous_circle_radius) * SMOOTHING_FACTOR;

    model.previous_circle_radius = model.circle_radius;

    for &position in &string_positions {
        let mut points = Vec::new();
        for i in 0..1000 {
            let x = map_range(
                i,
                0,
                999,
                -app.window_rect().w() / 2.0,
                app.window_rect().w() / 2.0,
            );
            let angle = map_range(i, 0, 999, 0.0, 2.0 * PI * frequency_multiplier);
            let y = position + amplitude * angle.sin();
            points.push(pt2(x, y));
        }
        model.string_points.push(points);
    }
}

fn view(app: &App, model: &Model, frame: Frame) {
    let draw = app.draw();
    draw.background().color(BLACK);

    // ocolating strings
    for points in &model.string_points {
        draw.polyline()
            .points(points.clone())
            .color(model.line_color);
    }

    //circle
    draw.ellipse()
        .x_y(0.0, 150.0)
        .radius(model.circle_radius)
        .color(model.circle_color);

    draw.to_frame(app, &frame).unwrap();
}
