# Rust Audio Visualizer

This is a real-time audio visualizer I threw together in Rust using Nannou, rustfft, and Audrey. It does basic beat detection using spectral flux and reacts to both volume and frequency.

You can feed it an audio file or let it run off live input.

I tried to make the strings respond more to string-like tones, and the synthwave sun pulse with kicks and snares.

It’s not rhythmically perfect, but the visual pulse holds up.

Some audio files (like the example song) are sampled at 44100 Hz, but Nannou expects 48000 Hz by default — so playback might be slightly faster unless adjusted.


[![Rust Audio Visualizer Demo](https://img.youtube.com/vi/BI1evQ_j3Bs/0.jpg)](https://youtu.be/BI1evQ_j3Bs)




## Acknowledgments

- Music in the demo video: "On the Run" by Timecop1983
