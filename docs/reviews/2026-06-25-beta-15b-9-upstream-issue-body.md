# qwen3-embedding-8b GGUF returns all-zero vectors via llama-cpp-4 0.3.2 on Mac Metal (also CPU, also context=4096)

## Summary

`Qwen/Qwen3-Embedding-8B-GGUF` (q8_0) embedding produces **all-zero vectors** via `llama-cpp-4 = 0.3.2` on macOS. The smaller `Qwen/Qwen3-Embedding-0.6B-GGUF` (q8_0) works correctly via the same code path with same dependency version.

Tested 3 configurations on 8b — all produce all-zero vectors with abnormal short inference time (~1–5 min for 202 sequences vs expected 10–60+ min for healthy 8b inference):

| Config | gpu_layers | context_size | Result |
|---|---|---|---|
| H1 (default Metal) | 99 (full Metal offload, 36 layers MTL0) | 2048 | ❌ all-zero, ~1 min |
| H2 (CPU) | 0 (all 36 layers dev=CPU per kv_cache log) | 2048 | ❌ all-zero, ~5 min |
| H3 (extended context) | 99 (Metal) | 4096 (KV 4096 cells per log) | ❌ all-zero, ~1 min |

All three runs produce identical bit-for-bit output file (SHA256 = `b243e2a9c4d508abe9c0672eebf194b0b17872b27b6870c65dce91d49ad80989`, dim=4096, 124 docs + 78 queries, every vector = `[0.0 × 4096]`, L2 = 0).

Same llama-cpp-4 0.3.2 + same Rust code with `qwen3-embedding-0.6b-q8_0.gguf` produces healthy vectors (cosine similarity ≥ 0.9999 vs the 0.3.0 baseline, dim=1024, L2 ≈ 1.0).

## Interesting log output

llama-cpp-4 0.3.2 **does identify and enable** Qwen3-Embedding-8B's special layer structure:

```
sched_reserve: resolving fused Gated Delta Net support:
sched_reserve: fused Gated Delta Net (autoregressive) enabled
sched_reserve: fused Gated Delta Net (chunked) enabled
```

So `fused Gated Delta Net` paths are wired in, but embedding output is still all-zero. This rules out "binding doesn't know about the architecture" but suggests a possible bug in the fused implementation's interaction with `LlamaPoolingType::Last` for embedding-only inference.

## Environment

- macOS 25.5.0 (Apple M5 Pro)
- `llama-cpp-4 = "0.3.2"` (with `llama-cpp-sys-4 = "0.3.2"`)
- Features: `metal` enabled, `default-features = false` (no `dynamic-link`)
- Upstream llama.cpp commit pinned: `94a220cd6` (per llama-cpp-4 0.3.1 README; 0.3.2 should be near)

## Models compared

| Model | SHA256 | Size | tensors | embedding_length | Works? |
|---|---|---|---|---|---|
| `Qwen/Qwen3-Embedding-0.6B-GGUF` q8_0 | (verified working) | 610 MB | 310 | 1024 | ✅ L2≈1.0, healthy |
| `Qwen/Qwen3-Embedding-8B-GGUF` q8_0 | `a48e50332ee0468f253c9af03d94f7e590906d0c096d13da17818fce0c227445` | 7.5 GB | 398 | 4096 | ❌ all-zero |

## GGUF metadata for the failing 8b model

```
general.architecture     = "qwen3"
general.name             = "Qwen3 Embedding 8B"
general.basename         = "Qwen3-Embedding"
general.size_label       = "8B"
general.file_type        = 7 (Q8_0)
general.quantization_version = 2

qwen3.block_count                       = 36
qwen3.context_length                    = 40960
qwen3.embedding_length                  = 4096
qwen3.feed_forward_length               = 12288
qwen3.attention.head_count              = 32
qwen3.attention.head_count_kv           = 8       (GQA 4:1)
qwen3.attention.key_length              = 128
qwen3.attention.value_length            = 128
qwen3.attention.layer_norm_rms_epsilon  = 1e-06
qwen3.rope.freq_base                    = 1000000.0
qwen3.pooling_type                      = 3       (LLAMA_POOLING_TYPE_LAST)

tokenizer.ggml.model                    = "gpt2"
tokenizer.ggml.pre                      = "qwen2"
(tokens 151665, merges 151387, eos 151643, pad 151643, eot 151645, bos 151643)
```

The 0.6b model that **does** work has the same key fields: `general.architecture=qwen3`, `qwen3.pooling_type=3`, `tokenizer=gpt2`. Differs only in scale: 310 vs 398 tensors, 1024 vs 4096 embedding_length, 28(0.6b) vs 36(8b) blocks.

## Minimal reproduction (Rust)

```rust
use llama_cpp_4::{
    context::params::LlamaContextParams,
    llama_backend::LlamaBackend,
    llama_batch::LlamaBatch,
    model::{params::LlamaModelParams, AddBos, LlamaModel},
};
use std::path::Path;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = LlamaBackend::init()?;
    let model_params = LlamaModelParams::default().with_n_gpu_layers(99);
    let model = LlamaModel::load_from_file(
        &backend,
        Path::new("models/qwen3-embedding-8b-q8_0.gguf"),
        &model_params,
    )?;
    let ctx_params = LlamaContextParams::default()
        .with_n_ctx(Some(std::num::NonZeroU32::new(2048).unwrap()))
        .with_embeddings(true)
        .with_pooling_type(llama_cpp_4::context::params::LlamaPoolingType::Last);
    let mut ctx = model.new_context(&backend, ctx_params)?;
    let tokens = model.str_to_token("hello world", AddBos::Always)?;
    let mut batch = LlamaBatch::new(tokens.len(), 1);
    for (i, &t) in tokens.iter().enumerate() {
        batch.add(t, i as i32, &[0], true)?;
    }
    ctx.decode(&mut batch)?;
    let emb = ctx.embeddings_seq_ith(0)?.to_vec();
    println!("dim={}, nonzero={}/{}, L2={}",
        emb.len(),
        emb.iter().filter(|&&x| x != 0.0).count(),
        emb.len(),
        emb.iter().map(|x| x * x).sum::<f32>().sqrt(),
    );
    Ok(())
}
```

Expected (works for 0.6b, fails for 8b):

- 0.6b: `dim=1024, nonzero=1024/1024, L2=1.000...`
- 8b: `dim=4096, nonzero=0/4096, L2=0` ← **bug**

Identical code, identical dependency version, same machine, same Mac Metal kernel — only model file differs.

## Hypotheses tested (all rejected by these 3 configs)

1. ❌ Upstream patch bump alone (0.3.0 → 0.3.2)
2. ❌ `gpu_layers=99` → `gpu_layers=0` (CPU mode, confirmed via kv_cache log all `dev=CPU`)
3. ❌ `context_size=2048` → `context_size=4096` (confirmed via `KV buffer size = 576 MiB / 4096 cells`)

## Open hypothesis

4. ❓ The 8B fused Gated Delta Net implementation (autoregressive + chunked, enabled per log) has a bug in its interaction with embedding-only inference + Last pooling that 0.6b path doesn't exercise. Possibly the fused path bypasses or zeros the embedding tensor write?

## What we're hoping to learn

- Is there a known upstream llama.cpp issue with Qwen3-Embedding-8B specifically?
- Is there a required `llama-cpp-sys-4` config we missed (e.g., disabling fused Gated Delta Net for embedding mode)?
- Is the upstream llama.cpp commit (`94a220cd6` per 0.3.1 README) confirmed to support 8b embeddings, or is a bump needed?

Happy to provide more logs / try suggested config tweaks. Thank you!
