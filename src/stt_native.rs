use std::sync::{Arc, Mutex, OnceLock};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

// cpal::Stream es !Send por conservadurismo multi-plataforma, pero en Linux/ALSA
// es seguro moverlo entre hilos bajo Mutex (solo un hilo lo toca a la vez).
struct SendStream(cpal::Stream);
unsafe impl Send for SendStream {}

struct NativeState {
    _stream: SendStream,
    samples: Arc<Mutex<Vec<f32>>>,
    sample_rate: u32,
    channels: u16,
}
unsafe impl Send for NativeState {}

static NATIVE: OnceLock<Mutex<Option<NativeState>>> = OnceLock::new();

fn slot() -> &'static Mutex<Option<NativeState>> {
    NATIVE.get_or_init(|| Mutex::new(None))
}

fn ensure_pulse_server_env() {
    if std::env::var("PULSE_SERVER").is_ok() {
        return;
    }
    // WSLg expone Pulse en /mnt/wslg/PulseServer (visto en pactl info)
    let wslg = "/mnt/wslg/PulseServer";
    if std::path::Path::new(wslg).exists() {
        // cpal ALSA -> pulse plugin respeta PULSE_SERVER
        std::env::set_var("PULSE_SERVER", format!("unix:{wslg}"));
        eprintln!("[stt_native] PULSE_SERVER -> unix:{wslg}");
    }
}

pub fn native_start() -> Result<(), String> {
    ensure_pulse_server_env();

    let mut guard = slot().lock().map_err(|e| format!("native lock: {e}"))?;
    if guard.is_some() {
        return Err("Ya hay una grabación en curso".to_string());
    }

    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "No se encontró dispositivo de entrada (mic no detectado por ALSA/Pulse)".to_string())?;

    let dev_name = device.name().unwrap_or_else(|_| "<unknown>".to_string());
    eprintln!("[stt_native] device: {dev_name}");

    let supported = device
        .default_input_config()
        .map_err(|e| format!("No hay config de entrada: {e} (verificá PipeWire/PulseAudio)"))?;

    let sample_rate = supported.sample_rate().0;
    let channels = supported.channels();
    let sample_format = supported.sample_format();

    eprintln!(
        "[stt_native] config: {:?} {}Hz {}ch",
        sample_format, sample_rate, channels
    );

    let samples: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::with_capacity(sample_rate as usize * 10)));
    let samples_clone = Arc::clone(&samples);

    let err_fn = |err| eprintln!("[stt_native] stream error: {err}");

    let stream: cpal::Stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let config: cpal::StreamConfig = supported.into();
            device
                .build_input_stream(
                    &config,
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        // data interleaved si stereo
                        let mut lock = samples_clone.lock().unwrap();
                        if channels == 1 {
                            lock.extend_from_slice(data);
                        } else {
                            // downmix a mono promediando canales
                            for frame in data.chunks(channels as usize) {
                                let sum: f32 = frame.iter().sum();
                                lock.push(sum / channels as f32);
                            }
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("build_input_stream F32: {e}"))?
        }
        cpal::SampleFormat::I16 => {
            let config: cpal::StreamConfig = supported.into();
            let samples_clone = Arc::clone(&samples);
            device
                .build_input_stream(
                    &config,
                    move |data: &[i16], _: &cpal::InputCallbackInfo| {
                        let mut lock = samples_clone.lock().unwrap();
                        if channels == 1 {
                            lock.extend(data.iter().map(|&s| s as f32 / i16::MAX as f32));
                        } else {
                            for frame in data.chunks(channels as usize) {
                                let sum: i32 = frame.iter().map(|&s| s as i32).sum();
                                lock.push(sum as f32 / (i16::MAX as f32 * channels as f32));
                            }
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("build_input_stream I16: {e}"))?
        }
        cpal::SampleFormat::U16 => {
            let config: cpal::StreamConfig = supported.into();
            let samples_clone = Arc::clone(&samples);
            device
                .build_input_stream(
                    &config,
                    move |data: &[u16], _: &cpal::InputCallbackInfo| {
                        let mut lock = samples_clone.lock().unwrap();
                        if channels == 1 {
                            lock.extend(data.iter().map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0));
                        } else {
                            for frame in data.chunks(channels as usize) {
                                let sum: f32 = frame.iter().map(|&s| (s as f32 / u16::MAX as f32) * 2.0 - 1.0).sum();
                                lock.push(sum / channels as f32);
                            }
                        }
                    },
                    err_fn,
                    None,
                )
                .map_err(|e| format!("build_input_stream U16: {e}"))?
        }
        other => return Err(format!("SampleFormat no soportado: {other:?}")),
    };

    stream.play().map_err(|e| format!("stream play: {e}"))?;
    eprintln!("[stt_native] recording started");

    *guard = Some(NativeState {
        _stream: SendStream(stream),
        samples,
        sample_rate,
        channels,
    });

    Ok(())
}

pub fn native_stop_and_transcribe() -> Result<String, String> {
    let state = slot()
        .lock()
        .map_err(|e| format!("native lock: {e}"))?
        .take()
        .ok_or_else(|| "No hay grabación activa".to_string())?;

    eprintln!("[stt_native] stopping, channels={} rate={}", state.channels, state.sample_rate);

    let raw = state.samples.lock().map_err(|e| format!("samples lock: {e}"))?;
    let len = raw.len();
    eprintln!("[stt_native] captured {} samples", len);

    if len < 1600 {
        return Err("Audio demasiado corto".to_string());
    }

    // Copiar fuera del lock para no bloquear
    let mono = raw.clone();
    drop(raw);
    // state._stream se dropea aquí (stop)

    // Resample a 16k mono si hace falta
    let pcm16k = if state.sample_rate == 16_000 {
        mono
    } else {
        crate::stt::resample_linear(&mono, state.sample_rate, 16_000)
    };

    eprintln!("[stt_native] resampled to {} samples @16k", pcm16k.len());

    crate::stt::transcribe_f32_mono_16k(&pcm16k)
}

pub fn native_cancel() {
    if let Ok(mut guard) = slot().lock() {
        if guard.take().is_some() {
            eprintln!("[stt_native] cancelled");
        }
    }
}

pub fn is_recording() -> bool {
    slot().lock().map(|g| g.is_some()).unwrap_or(false)
}
