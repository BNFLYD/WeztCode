use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Mutex, OnceLock};

/// Modo de operación del STT (voz a texto), controlado por la prop stt_mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SttMode {
    Off,
    Embedded,
    Server,
}

impl SttMode {
    fn from_props() -> Self {
        let props = crate::config::props::UserProps::load();
        match props.get("stt_mode").map(str::trim).filter(|s| !s.is_empty()) {
            Some("server") => Self::Server,
            Some("off") | Some("disabled") => Self::Off,
            _ => {
                // Default: embedded si hay modelo configurado/encontrado, si no off
                if model_path().is_some() {
                    Self::Embedded
                } else {
                    Self::Off
                }
            }
        }
    }
}

/// Resuelve la ruta al modelo GGML: prop stt_model_path (con expansión de ~)
/// o fallback a ~/.local/share/weztcode/models/*.bin
fn model_path() -> Option<PathBuf> {
    let props = crate::config::props::UserProps::load();
    if let Some(raw) = props.get("stt_model_path").map(str::trim).filter(|s| !s.is_empty()) {
        let expanded = if let Some(rest) = raw.strip_prefix("~/") {
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(rest)
        } else {
            PathBuf::from(raw)
        };
        return if expanded.exists() { Some(expanded) } else { None };
    }

    let home = std::env::var("HOME").unwrap_or_default();
    let dir = PathBuf::from(home).join(".local/share/weztcode/models");
    let mut candidates: Vec<_> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().map(|e| e == "bin").unwrap_or(false))
        .collect();
    candidates.sort();
    candidates.into_iter().next()
}

struct EmbeddedStt {
    ctx: whisper_rs::WhisperContext,
}

static EMBEDDED_STT: OnceLock<Mutex<Option<EmbeddedStt>>> = OnceLock::new();

fn embedded_slot() -> &'static Mutex<Option<EmbeddedStt>> {
    EMBEDDED_STT.get_or_init(|| Mutex::new(None))
}

/// Carga lazy del modelo: solo ocurre en la primera transcripción.
fn load_embedded() -> Result<&'static Mutex<Option<EmbeddedStt>>, String> {
    let slot = embedded_slot();
    let mut guard = slot.lock().map_err(|e| format!("stt lock: {}", e))?;
    if guard.is_none() {
        let path = model_path()
            .ok_or_else(|| "No hay modelo Whisper: configurá stt_model_path en user_props.lua".to_string())?;
        eprintln!("[stt] cargando modelo: {}", path.display());
        let ctx_params = whisper_rs::WhisperContextParameters::default();
        let ctx = whisper_rs::WhisperContext::new_with_params(&path, ctx_params)
            .map_err(|e| format!("Failed to load whisper model {:?}: {}", path.file_name(), e))?;
        eprintln!("[stt] modelo listo");
        *guard = Some(EmbeddedStt { ctx });
    }
    Ok(slot)
}

fn stt_language() -> Option<String> {
    let props = crate::config::props::UserProps::load();
    let lang = props
        .get("stt_language")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("auto")
        .to_ascii_lowercase();
    if lang == "auto" {
        None
    } else {
        Some(lang)
    }
}

/// Punto de entrada: transcribe un WAV (PCM 16-bit, cualquier rate/canales).
pub fn transcribe(wav_bytes: &[u8]) -> Result<String, String> {
    match SttMode::from_props() {
        SttMode::Off => Err("STT deshabilitado (stt_mode = \"off\" o sin modelo)".to_string()),
        SttMode::Embedded => transcribe_embedded(wav_bytes),
        SttMode::Server => transcribe_server(wav_bytes),
    }
}

fn transcribe_embedded(wav_bytes: &[u8]) -> Result<String, String> {
    let samples = decode_wav_mono_16k(wav_bytes)?;
    transcribe_f32_mono_16k(&samples)
}

/// Transcribe directo desde PCM mono 16kHz f32 (usado por captura nativa cpal).
pub fn transcribe_f32_mono_16k(samples: &[f32]) -> Result<String, String> {
    if samples.len() < 1600 {
        return Err("Audio demasiado corto".to_string());
    }
    // Truncar a 2 min por seguridad (mismo límite que decode_wav)
    let samples = if samples.len() > 2 * 60 * 16_000 {
        &samples[..2 * 60 * 16_000]
    } else {
        samples
    };

    let slot = load_embedded()?;
    let mut guard = slot.lock().map_err(|e| format!("stt lock: {}", e))?;
    let stt = guard.as_mut().ok_or("stt model not loaded")?;

    let mut state = stt.ctx.create_state().map_err(|e| format!("create_state: {}", e))?;

    let mut params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
    let lang = stt_language();
    params.set_language(lang.as_deref());
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);

    state.full(params, samples)
        .map_err(|e| format!("whisper inference: {}", e))?;

    let n = state.full_n_segments().max(0);
    let mut text = String::new();
    for i in 0..n {
        if let Some(seg) = state.get_segment(i) {
            if let Ok(s) = seg.to_str() {
                text.push_str(s);
            }
        }
    }
    Ok(text.trim().to_string())
}

/// Resample lineal genérico (usado también por captura nativa).
pub fn resample_linear(input: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || input.is_empty() {
        return input.to_vec();
    }
    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = (input.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        let frac = (pos - idx as f64) as f32;
        let a = input[idx];
        let b = input.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    out
}

/// Modo server: proxy multipart vía curl a stt_server_url.
/// Compatible con whisper.cpp server (/inference) y endpoints estilo OpenAI
/// (/v1/audio/transcriptions): ambos responden {"text": "..."}.
fn transcribe_server(wav_bytes: &[u8]) -> Result<String, String> {
    let url = crate::config::props::UserProps::load()
        .get("stt_server_url")
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| "Modo server sin stt_server_url en user_props.lua".to_string())?
        .to_string();

    let tmp_dir = std::env::temp_dir().join("weztcode-stt");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| format!("temp dir: {}", e))?;
    let tmp_wav = tmp_dir.join(format!("req-{}.wav", std::process::id()));
    fs_write(&tmp_wav, wav_bytes)?;

    let output = Command::new("curl")
        .args(["-sS", "-X", "POST", "--max-time", "120"])
        .arg("-F")
        .arg(format!("file=@{}", tmp_wav.display()))
        .arg(&url)
        .output();

    let _ = std::fs::remove_file(&tmp_wav);

    let output = output.map_err(|e| format!("Failed to run curl: {}", e))?;
    if !output.status.success() {
        return Err(format!(
            "STT server error: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let body = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value =
        serde_json::from_str(body.trim()).map_err(|e| format!("Invalid STT response: {} ({})", e, body))?;
    let text = json
        .get("text")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "STT response sin campo 'text'".to_string())?;
    Ok(text.trim().to_string())
}

fn fs_write(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|e| format!("write {}: {}", path.display(), e))
}

/// Decodifica WAV (hound) → mono f32 → resample lineal a 16 kHz.
fn decode_wav_mono_16k(bytes: &[u8]) -> Result<Vec<f32>, String> {
    let reader = hound::WavReader::new(Cursor::new(bytes))
        .map_err(|e| format!("WAV inválido: {}", e))?;

    let spec = reader.spec();
    let channels = spec.channels as usize;
    let sample_rate = spec.sample_rate as usize;

    let mut mono: Vec<f32> = Vec::with_capacity(reader.duration() as usize / channels.max(1) + 1);
    let mut frame_acc = vec![0f32; channels];
    let mut frame_idx = 0usize;

    for sample in reader.into_samples::<i32>() {
        // into_samples::<i32> funciona tanto para int como para float normalizado por hound
        let v = sample.map_err(|e| format!("sample corrupto: {}", e))?;
        let norm = normalize_sample(v, spec.sample_format, spec.bits_per_sample);
        frame_acc[frame_idx] += norm;
        frame_idx += 1;
        if frame_idx == channels {
            for c in frame_acc.iter_mut() {
                *c /= channels as f32;
            }
            mono.extend_from_slice(&frame_acc);
            frame_acc.iter_mut().for_each(|c| *c = 0.0);
            frame_idx = 0;
        }
    }

    // Límite de seguridad: 2 minutos a 16k
    const MAX_SAMPLES: usize = 2 * 60 * 16_000;
    mono.truncate(MAX_SAMPLES);

    if sample_rate == 16_000 {
        return Ok(mono);
    }

    let ratio = sample_rate as f64 / 16_000.0;
    let out_len = (mono.len() as f64 / ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let pos = i as f64 * ratio;
        let idx = pos as usize;
        let frac = (pos - idx as f64) as f32;
        let a = mono[idx];
        let b = mono.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }
    Ok(out)
}

fn normalize_sample(value: i32, format: hound::SampleFormat, bits: u16) -> f32 {
    match format {
        hound::SampleFormat::Float => f32::from_bits(value as u32),
        hound::SampleFormat::Int => {
            let max = (1i64 << (bits - 1)) as f32;
            value as f32 / max
        }
    }
}
