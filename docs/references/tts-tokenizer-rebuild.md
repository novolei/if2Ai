# TTS Tokenizer Rebuild — Generating bit-exact `tokenizer.json` from SentencePiece

## Why this exists

`src-tauri/src/modules/tts/text/tokenizer.rs` (Phase TTS-B.6+) loads
`tokenizer.json` via the pure-Rust HuggingFace `tokenizers` crate, eliminating the
former Python `sentencepiece` sidecar dependency.

For the TTS pipeline to produce identical audio to the reference Python
implementation, the loaded `tokenizer.json` must encode every input string to the
**identical** token id sequence as `sentencepiece.SentencePieceProcessor.Encode()`
on the source `tokenizer.model`.

This document describes how to regenerate `tokenizer.json` from `tokenizer.model`
when a model is updated.

## When to run

- A new TTS model is added (different `tokenizer.model`).
- An upstream `tokenizer.model` is replaced (vocab change, retrain, etc.).
- The bit-exact verification suite (below) reports any mismatch.

## Prerequisites

```bash
python3 -m venv /tmp/spvenv
/tmp/spvenv/bin/pip install --upgrade pip
/tmp/spvenv/bin/pip install "transformers<5" sentencepiece protobuf
```

`transformers >= 5` has a regression in `convert_slow_tokenizer.SpmExtractor`
(`AttributeError: 'list' object has no attribute '__name__'`); pin to 4.x.

## Generate `tokenizer.json`

```python
import os, json, base64
from transformers.models.llama.tokenization_llama import LlamaTokenizer
from transformers.convert_slow_tokenizer import LlamaConverter
import sentencepiece.sentencepiece_model_pb2 as pb

MODEL_DIR = os.path.expanduser('~/.if2ai/models/tts/MOSS-TTS-Nano-100M-ONNX')
MODEL_PATH = f'{MODEL_DIR}/tokenizer.model'
OUT_PATH = f'{MODEL_DIR}/tokenizer.json'

# Step 1: HF official converter (BPE + byte_fallback, like LLaMA)
slow = LlamaTokenizer(
    vocab_file=MODEL_PATH,
    legacy=False,
    add_bos_token=False,
    add_eos_token=False,
)
fast = LlamaConverter(slow).converted()
fast.save(OUT_PATH)

# Step 2: Inject SentencePiece nmt_nfkc precompiled_charsmap as Precompiled
# normalizer (LlamaConverter does not preserve it). Required for CJK
# punctuation normalization (e.g. '…' -> '...', '，' -> ',').
proto = pb.ModelProto()
proto.ParseFromString(open(MODEL_PATH, 'rb').read())
charsmap = proto.normalizer_spec.precompiled_charsmap
assert len(charsmap) > 0, 'tokenizer.model has no precompiled_charsmap; not nmt_nfkc?'

with open(OUT_PATH) as f:
    d = json.load(f)

d['normalizer'] = {
    'type': 'Sequence',
    'normalizers': [
        {
            'type': 'Precompiled',
            'precompiled_charsmap': base64.b64encode(charsmap).decode('ascii'),
        },
        # Match SP `remove_extra_whitespaces=True`
        {'type': 'Replace', 'pattern': {'Regex': ' {2,}'}, 'content': ' '},
        # Match SP behavior: trim trailing whitespace, but preserve leading
        # space so `add_dummy_prefix=True` semantics survive
        {'type': 'Strip', 'strip_left': False, 'strip_right': True},
    ],
}

with open(OUT_PATH, 'w') as f:
    json.dump(d, f, ensure_ascii=False)
print(f'Wrote {OUT_PATH} ({os.path.getsize(OUT_PATH)} bytes)')
```

## Verify bit-exactness

After regeneration, run a 1000+ sentence diff between SP and the new HF tokenizer.

```python
# /tmp/verify_tokenizer.py
import os, sys, subprocess, sentencepiece as sp

MODEL_DIR = os.path.expanduser('~/.if2ai/models/tts/MOSS-TTS-Nano-100M-ONNX')
sp_proc = sp.SentencePieceProcessor()
sp_proc.Load(f'{MODEL_DIR}/tokenizer.model')

# Build test corpus: real markdown + generated stress tests.
# (See docs/references/tts-tokenizer-corpus.txt for the canonical set.)
texts = open('docs/references/tts-tokenizer-corpus.txt').read().splitlines()

# Run HF side via the in-tree Rust test binary or a small ad-hoc rust util.
# Easiest: write a tiny rust binary (see verify-rs.md) that prints `[ids]` per line.

mismatches = 0
for i, t in enumerate(texts):
    sp_ids = sp_proc.encode(t, out_type=int)
    # ... compare with HF output for the same line ...
    pass  # placeholder

print(f'mismatches: {mismatches}/{len(texts)}')
sys.exit(0 if mismatches == 0 else 1)
```

The verified MOSS-TTS-Nano-100M tokenizer.json (Apr 2026) achieves 1000/1000
bit-exact on the canonical corpus.

## Required normalizer chain (summary)

The order matters. From input to vocabulary lookup:

1. `Precompiled(nmt_nfkc charsmap)` — full-width → half-width punctuation, etc.
2. `Replace(/  +/, " ")` — collapse runs of spaces.
3. `Strip(right only)` — trim trailing whitespace, **keep leading**.
4. `Metaspace(▁, prepend_scheme="first", split=false)` (set by LlamaConverter,
   already in `pre_tokenizer`) — convert spaces to `▁` and prepend a `▁` to the
   first piece.

Removing or reordering these will break bit-exactness on at least one of:

- CJK punctuation (`…`/`，`/`！`)
- multi-space inputs (`"a   b   c"`)
- inputs with leading or trailing whitespace

## Alternative model families

The script above uses `LlamaConverter` because MOSS-TTS-Nano shares LLaMA's
SentencePiece flavor (BPE + `byte_fallback=True` + `nmt_nfkc`). For other model
families:

| SP `model_type`     | HF converter to use                                     |
| ------------------- | ------------------------------------------------------- |
| BPE + byte_fallback | `LlamaConverter` (this doc)                             |
| Unigram             | `T5Converter` / load via `Unigram::load_spm()` directly |
| Pure BPE            | `GPT2Converter`                                         |

Inspect the `.model` first:

```python
import sentencepiece.sentencepiece_model_pb2 as pb
mp = pb.ModelProto()
mp.ParseFromString(open('tokenizer.model', 'rb').read())
print('model_type:', mp.trainer_spec.model_type)  # 1=Unigram 2=BPE 3=Word 4=Char
print('byte_fallback:', mp.trainer_spec.byte_fallback)
print('normalizer:', mp.normalizer_spec.name)
```
