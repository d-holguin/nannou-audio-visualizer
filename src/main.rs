use audio_visualizer_core::*;
use nannou::prelude::*;
use nannou_audio as audio;
use nannou_audio::cpal;
use nannou_audio::cpal::traits::{DeviceTrait, HostTrait};
use rustfft::num_complex::Complex;
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};

const THRESHOLD_MULTIPLIER: f32 = 2.0;
const SPECTRAL_FLUX_FRAMES: usize = 30;
const COOLDOWN_TIME: usize = 20;

fn main() {
    nannou::app(Model::new).update(Model::update).run();
}

struct Model {
    stream: audio::Stream<Audio>,
    volume: Arc<Mutex<f32>>,
    fft_output: Arc<Mutex<Vec<Complex<f32>>>>,
    hue: f32,
    string_points: Vec<Vec<Point2>>,
    circle_radius: f32,
    line_color: LinSrgba,
    circle_color: LinSrgba,
    prev_power_spectrum: Vec<f32>,
    past_magnitudes: Vec<Vec<f32>>,
    past_spectral_flux: Vec<f32>,
    cooldown_counter: usize,
    smoothed_flux: f32,
    circle_velocity: f32,
}

impl Model {
    fn new(app: &App) -> Self {
        let config = Args::parse().ok();

        app.new_window()
            .view(Model::view)
            .key_pressed(controls)
            .build()
            .unwrap();

        let audio_host = audio::Host::new();
        let fft_output = Arc::new(Mutex::new(vec![]));
        let (volume_sender, _volume_receiver) = channel();
        let volume = Arc::new(Mutex::new(0.0));

        let mut audio_model = Audio {
            volume_sender,
            fft_output: Arc::clone(&fft_output),
            volume: Arc::clone(&volume),
            sounds: vec![],
            file_sample_rate: None,
        };

        let stream = match config {
            Some(config) => {
                // Play audio file mode
                let reader = audrey::open(&config.input_file).expect("failed to open audio file");
                let sample_rate = reader.description().sample_rate();
                audio_model.file_sample_rate = Some(sample_rate);
                println!("Audio sample rate: {}", sample_rate);
                audio_model.sounds.push(reader);

                audio_host
                    .new_output_stream(audio_model)
                    .render(render)
                    .build()
                    .expect("failed to build output stream")
            }
            None => {
                // Live input fallback
                println!("No input file provided, using live input");
                list_input_devices();

                audio_host
                    .new_input_stream(audio_model)
                    .capture(capture)
                    .build()
                    .expect("failed to build input stream")
            }
        };

        stream.play().unwrap();

        Model {
            stream,
            volume,
            fft_output,
            hue: 0.0,
            string_points: Vec::new(),
            circle_radius: 0.0,
            line_color: hsl(0.0, 0.0, 0.0).into(), // Setting initial color to black
            circle_color: hsl(0.0, 0.0, 0.0).into(),
            prev_power_spectrum: Vec::new(),
            past_magnitudes: vec![vec![0.0; 10]; 6],
            past_spectral_flux: Vec::new(),
            cooldown_counter: 0,
            smoothed_flux: 0.0,
            circle_velocity: 0.0,
        }
    }

    fn update(_app: &App, model: &mut Model, update: Update) {
        
        let dt = update.since_last.as_secs_f32();
        let fft_output_guard = model.fft_output.lock().unwrap();
        let mut fft_magnitudes: Vec<f32> = fft_output_guard.iter().map(|c| c.norm()).collect();

        let spectral_flux = process_fft_output(&fft_magnitudes, &mut model.prev_power_spectrum);

        let neon_hue = 0.6 + 0.3 * (model.hue / 1.0);
        model.line_color = hsl(neon_hue, 1.0, 0.45).into();
        model.circle_color = hsl(neon_hue, 1.0, 0.45).into();

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
            (volume.log(10.0) * 14.5).clamp(1.0, 100.0)
        } else {
            1.0
        };

        let log_spectral_flux = (spectral_flux + 1.0).log(10.0);
        let frequency_multiplier = log_spectral_flux.powf(2.0);
        let window_width = 2300.0;
        let num_points = 2000;
        let frequency = frequency_multiplier * 0.25;

        model.string_points.clear();
        for _ in 0..6 {
            let mut points = Vec::new();
            for i in 0..=num_points {
                let x = map_range(i, 0, num_points, -window_width / 2.0, window_width / 2.0);
                let angle = (i as f32 * frequency * 2.0 * PI / num_points as f32) + (PI / 2.0);
                let y = amplitude * angle.sin();
                points.push(pt2(x, y));
            }
            model.string_points.push(points);
        }

        const FLUX_SMOOTHING: f32 = 0.4;
        model.smoothed_flux =
            model.smoothed_flux * (1.0 - FLUX_SMOOTHING) + spectral_flux * FLUX_SMOOTHING;

        model.past_spectral_flux.push(model.smoothed_flux);
        if model.past_spectral_flux.len() > SPECTRAL_FLUX_FRAMES {
            model.past_spectral_flux.remove(0);
        }

        let flux_history = &model.past_spectral_flux;
        let avg_flux = flux_history.iter().sum::<f32>() / flux_history.len() as f32;
        //let sensitivity = 1.3;
        let mean_flux = avg_flux;
        let std_dev_flux = {
            let variance = flux_history
                .iter()
                .map(|v| (v - mean_flux).powi(2))
                .sum::<f32>()
                / flux_history.len() as f32;
            variance.sqrt()
        };

        let adaptive_threshold = mean_flux + std_dev_flux * THRESHOLD_MULTIPLIER;

        const BASE_RADIUS: f32 = 50.0;

        if model.cooldown_counter == 0 {
            let last_flux = if flux_history.len() >= 2 {
                flux_history[flux_history.len() - 2]
            } else {
                0.0
            };

            if model.smoothed_flux > adaptive_threshold && model.smoothed_flux > last_flux {
                model.hue = (model.hue + 0.3) % 1.0;
                let beat_strength = (model.smoothed_flux - adaptive_threshold).clamp(0.0, 1.0);
                model.circle_velocity += 20.0 * beat_strength;
                model.cooldown_counter = COOLDOWN_TIME;
            }
        } else {
            model.cooldown_counter -= 1;
        }

        // Spring dynamics
        let target = BASE_RADIUS;
        let stiffness = 0.08; // how fast it returns to target
        let damping = 0.4; // how much velocity is absorbed

        let displacement = model.circle_radius - target;
        model.circle_velocity -= displacement * stiffness;
        model.circle_velocity *= 1.0 - damping;

        model.circle_radius += model.circle_velocity;
        model.circle_radius = model.circle_radius.clamp(50.0, 180.0);
    }

    fn view(app: &App, model: &Model, frame: Frame) {
        let draw = app.draw();
        draw.background().color(BLACK);

        let line_color = model.circle_color;

        let base_y = 210.0; // controls vertical center
        let spacing = 30.0;
        let string_positions: Vec<f32> = (-3..=2).map(|i| base_y + i as f32 * spacing).collect();

        for (index, &position) in string_positions.iter().enumerate() {
            if index < model.string_points.len() {
                let points = &model.string_points[index];
                let osc_points: Vec<_> = points.iter().map(|&p| pt2(p.x, p.y + position)).collect();
                draw.polyline().points(osc_points).color(line_color);
            }
        }

        let horizon_y = 100.0;
        let grid_depth = 400.0;
        let grid_width = 800.0;
        let vertical_lines = 20;
        let horizontal_lines = 30;

        let audio_amplitude = model.circle_radius / 300.0;
        let wave_freq = 4.0;
        let wave_amp = 40.0 * audio_amplitude;

        // Vertical lines (static perspective lines)
        for i in -vertical_lines..=vertical_lines {
            let x = i as f32 * 40.0;
            let t = (i + vertical_lines) as f32 / (2.0 * vertical_lines as f32);
            let color = hsl(model.hue + t * 0.1, 1.0, 0.6 + 0.2 * t);
            draw.line()
                .start(pt2(x, -grid_depth))
                .end(pt2(0.0, horizon_y))
                .color(color);
        }

        // Horizontal lines with wave distortion
        for i in 0..=horizontal_lines {
            let z = i as f32 / horizontal_lines as f32;
            let y = -grid_depth + z * (grid_depth + horizon_y);
            let half_w = grid_width * (1.0 - z);

            let mut points = Vec::new();
            let segments = 100;

            for j in 0..=segments {
                let t = j as f32 / segments as f32;
                let x = lerp(-half_w, half_w, t);
                let offset = (x * wave_freq * 0.01 + model.hue * TAU).sin() * wave_amp * (1.0 - z);
                points.push(pt2(x, y + offset));
            }
            let color = hsl(model.hue + z * 0.1, 1.0, 0.55 + 0.25 * z);
            draw.polyline().points(points).color(color);
        }

        let circle_color = model.circle_color;
        draw.ellipse()
            .x_y(0.0, 150.0)
            .radius(model.circle_radius)
            .color(circle_color);

        draw_synthwave_sun(&draw, pt2(0.0, 150.0), model.circle_radius, model.hue);

        draw.to_frame(app, &frame).unwrap();
    }
}

fn draw_synthwave_sun(draw: &Draw, center: Point2, radius: f32, hue: f32) {
    let layers = 20;
    for i in 0..layers {
        let t = i as f32 / (layers - 1) as f32;
        let layer_radius = radius * (1.0 - t * 0.05);
        let color = hsl(hue + t * 0.1, 1.0, 0.55 + 0.25 * t);
        draw.ellipse().xy(center).radius(layer_radius).color(color);
    }
}

fn list_input_devices() {
    let host = cpal::default_host();
    let devices = host.input_devices().unwrap();
    println!("Available input devices:");
    for device in devices {
        println!(" - {:?}", device.name());
    }
}

fn process_fft_output(fft_output: &[f32], prev_power_spectrum: &mut Vec<f32>) -> f32 {
    let num_bins = fft_output.len();
    let low_bin_cutoff = (num_bins as f32 * 0.1) as usize; // Lower 10% of spectrum

    let mut spectral_flux = 0.0;
    let mut power_spectrum = vec![0.0; num_bins];

    for i in 0..num_bins {
        let power = fft_output[i] * fft_output[i];
        power_spectrum[i] = power;
    }

    if !prev_power_spectrum.is_empty() {
        for i in 0..low_bin_cutoff {
            let flux = power_spectrum[i] - prev_power_spectrum[i];
            if flux > 0.0 {
                spectral_flux += flux;
            }
        }
    }

    *prev_power_spectrum = power_spectrum;

    spectral_flux
}

/// Press space to pauce or play the stream
fn controls(_app: &App, model: &mut Model, key: Key) {
    if key == Key::Space {
        if model.stream.is_playing() {
            model.stream.pause().expect("Failed to pause audio stream");
        } else {
            model.stream.play().expect("Failed to play audio stream");
        }
    }
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
