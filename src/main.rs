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
        previous_hue: Arc::new(Mutex::new(0.6)),
        previous_frequency_multiplier: Arc::new(Mutex::new(0.0)),
        previous_circle_radius: Arc::new(Mutex::new(0.0)),
        hue: 0.0,
        high_freq_sum: 0.0,
        low_freq_sum: 0.0,
    }
}

fn key_pressed(app: &App, model: &mut Model, key: Key) {
    match key {
        Key::Space => {
            let assets = app.assets_path().expect("could not find assets directory");
            let path = assets.join("driver.wav");
            let sound = audrey::open(path).expect("failed to load sound");
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
    previous_hue: Arc<Mutex<f32>>,
    previous_frequency_multiplier: Arc<Mutex<f32>>,
    previous_circle_radius: Arc<Mutex<f32>>,
    hue: f32,
    high_freq_sum: f32,
    low_freq_sum: f32,
}

fn update(app: &App, model: &mut Model, _update: Update) {
    let fft_output = model.fft_output.lock().unwrap();
    let num_bins = fft_output.len() / 2;

    let mut low_freq_sum = 0.0;
    let mut mid_freq_sum = 0.0;
    let mut high_freq_sum = 0.0;

    let low_note_range = 0..(num_bins / 3);
    let mid_note_range = (num_bins / 3)..(2 * num_bins / 3);

    for (i, bin) in fft_output.iter().enumerate().take(num_bins) {
        let magnitude = bin.norm();

        if low_note_range.contains(&i) {
            low_freq_sum += magnitude;
        } else if mid_note_range.contains(&i) {
            mid_freq_sum += magnitude;
        } else {
            high_freq_sum += magnitude;
        }
    }

    let hue_scaling_factor = 0.005;
    let hue_change_threshold = 21.0; 

    let hue = {
        let mut previous_hue_guard = model.previous_hue.lock().unwrap();
        if low_freq_sum > hue_change_threshold {
            let hue_change = low_freq_sum * hue_scaling_factor;
            *previous_hue_guard = (*previous_hue_guard + hue_change) % 1.0;
        }
        *previous_hue_guard
    };

    
    model.hue = hue;
    model.high_freq_sum = high_freq_sum;
    model.low_freq_sum = low_freq_sum;
}

fn view(app: &App, model: &Model, frame: Frame) {
    let draw = app.draw();
    draw.background().color(BLACK);

    let window_rect = app.window_rect();
    let window_width = window_rect.w();

    
    let string_positions = [-90.0, -60.0, -30.0, 0.0, 30.0, 60.0];

   
    let easing = 0.2;

    for (index, &position) in string_positions.iter().enumerate() {
        let mut points = Vec::new();

        let line_color = nannou::color::hsl(model.hue, 1.0, 0.5);

        let target_frequency_multiplier = model.high_freq_sum;

     
        let mut current_frequency_multiplier = model.previous_frequency_multiplier.lock().unwrap();
        let frequency_multiplier = *current_frequency_multiplier
            + (target_frequency_multiplier - *current_frequency_multiplier) * easing;
        *current_frequency_multiplier = frequency_multiplier;

        let volume = *model.volume.lock().unwrap();
        let amplitude = (volume * volume) * 0.0045; 

        let frequency = frequency_multiplier * 0.25; 

        for i in 0..1000 {
            let x = map_range(i, 0, 999, -window_width / 2.0, window_width / 2.0);
            let angle = map_range(i, 0, 999, 0.0, 2.0 * PI * frequency);
            let y = position + amplitude * angle.sin();
            points.push(pt2(x, y));
        }

        draw.polyline().points(points).color(line_color);
    }

    draw_pulsating_circle(&draw, model);

    draw.to_frame(app, &frame).unwrap();
}

fn draw_pulsating_circle(draw: &Draw, model: &Model) {
  
    let circle_scaling_factor = 20.0; 

    
    let circle_easing = 1000.0;

    let target_circle_radius = model.low_freq_sum * circle_scaling_factor;
    let mut previous_circle_radius = model.previous_circle_radius.lock().unwrap();
    let circle_radius =
        *previous_circle_radius + (target_circle_radius - *previous_circle_radius) * circle_easing;
    *previous_circle_radius = circle_radius;

    let circle_color = nannou::color::hsl(model.hue, 1.0, 0.5);

    
    let circle_size = 32.5; 

    
    let min_circle_size = 30.0; 
    let max_circle_size = 35.0; 
    let pulsation_speed = 0.9; 

    
    let target_circle_size =
        min_circle_size + (max_circle_size - min_circle_size) * model.high_freq_sum;
    let pulsating_circle_size = circle_size + (target_circle_size - circle_size) * pulsation_speed;

    draw.ellipse()
        .x_y(0.0, 150.0)
        .radius(pulsating_circle_size)
        .color(circle_color);
}
