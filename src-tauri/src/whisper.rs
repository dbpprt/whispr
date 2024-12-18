use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
use crate::config::WhisprConfig;
use std::sync::Arc;
use std::result::Result;

pub struct WhisperProcessor {
    ctx: Arc<WhisperContext>,
    config: WhisprConfig,
}

impl WhisperProcessor {
    pub fn new(model_path: &std::path::Path, config: WhisprConfig) -> Result<Self, String> {
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().ok_or_else(|| "Invalid model path".to_string())?,
            WhisperContextParameters::default()
        ).map_err(|e| e.to_string())?;

        Ok(Self {
            ctx: Arc::new(ctx),
            config,
        })
    }

    pub fn process_audio(&self, captured_audio: Vec<f32>) -> Result<Vec<(f32, f32, String)>, String> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(self.config.whisper.language.as_deref());
        params.set_translate(self.config.whisper.translate);

        let mut state = self.ctx.create_state()
            .map_err(|e| e.to_string())?;

        state.full(params, &captured_audio[..])
            .map_err(|e| e.to_string())?;

        let num_segments = state.full_n_segments()
            .map_err(|e| e.to_string())?;

        let mut segments = Vec::new();
        for i in 0..num_segments {
            let segment = state.full_get_segment_text(i)
                .map_err(|e| e.to_string())?;
            let start = state.full_get_segment_t0(i)
                .map_err(|e| e.to_string())? as f32;
            let end = state.full_get_segment_t1(i)
                .map_err(|e| e.to_string())? as f32;

            println!("[{} - {}]: {}", start, end, segment);
            segments.push((start, end, segment));
        }
        Ok(segments)
    }
}